use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use loong_contracts::CapabilityToken;
use loong_kernel::{
    AuditError, AuditSink, Capability, Clock, ExecutionRoute, FanoutAuditSink, HarnessKind,
    InMemoryAuditSink, JsonlAuditSink, LoongKernel, StaticPolicyEngine, SystemClock,
    VerticalPackManifest,
};

use crate::config::{AuditMode, HttpAuditConfig, LoongConfig, SyslogAuditConfig};

/// Default pack identifier used by MVP entry points.
const MVP_PACK_ID: &str = "dev-automation";

/// Default token TTL (24 hours) for long-running MVP entry points.
pub const DEFAULT_TOKEN_TTL_S: u64 = 86400;

/// Kernel execution context for policy-gated MVP operations.
///
/// When present, memory and tool operations route through the kernel's
/// capability/policy/audit system instead of direct adapter calls.
///
/// `pack_id` and `agent_id` are accessed via the embedded `CapabilityToken`
/// to avoid data divergence.
#[derive(Clone)]
pub struct KernelContext {
    pub kernel: Arc<LoongKernel<StaticPolicyEngine>>,
    pub token: CapabilityToken,
}

impl KernelContext {
    pub fn pack_id(&self) -> &str {
        &self.token.pack_id
    }

    pub fn agent_id(&self) -> &str {
        &self.token.agent_id
    }
}

/// Bootstrap a minimal in-memory kernel suitable for tests.
///
/// Registers a default pack manifest with the MVP tool, memory, filesystem,
/// and public-web capabilities, then issues a long-lived token for the given
/// `agent_id`.
///
/// Production-facing runtime entrypoints should prefer
/// `bootstrap_kernel_context_with_config` so audit retention follows config.
#[cfg(test)]
pub(crate) fn bootstrap_test_kernel_context(
    agent_id: &str,
    ttl_s: u64,
) -> Result<KernelContext, String> {
    bootstrap_kernel_context_with_audit_sink(
        agent_id,
        ttl_s,
        Arc::new(InMemoryAuditSink::default()) as Arc<dyn AuditSink>,
        &LoongConfig::default(),
    )
}

/// Bootstrap a governed kernel context for production-facing runtime entrypoints.
///
/// This installs the audit sink selected by `config.audit`, registers the MVP
/// pack plus the core tool/memory adapters and policy extensions, and issues a
/// long-lived capability token for `agent_id`.
///
/// The helper intentionally stays below higher-level runtime initialization: it
/// does not export `LOONG_*` environment variables, resolve chat session
/// ids, or prepare channel/conversation state. Callers that need those side
/// effects should compose it with `runtime_env::initialize_runtime_environment`
/// or a surface-specific bootstrap such as `chat::initialize_cli_turn_runtime`.
pub fn bootstrap_kernel_context_with_config(
    agent_id: &str,
    ttl_s: u64,
    config: &LoongConfig,
) -> Result<KernelContext, String> {
    bootstrap_kernel_context_with_audit_sink(agent_id, ttl_s, build_audit_sink(config)?, config)
}

fn build_audit_sink(config: &LoongConfig) -> Result<Arc<dyn AuditSink>, String> {
    match config.audit.mode {
        AuditMode::InMemory => Ok(Arc::new(InMemoryAuditSink::default()) as Arc<dyn AuditSink>),
        AuditMode::Jsonl => build_jsonl_audit_sink(config),
        AuditMode::Fanout => {
            let durable = build_jsonl_audit_sink(config)?;
            if !config.audit.retain_in_memory {
                return Ok(durable);
            }

            Ok(Arc::new(FanoutAuditSink::new(vec![
                durable,
                Arc::new(InMemoryAuditSink::default()) as Arc<dyn AuditSink>,
            ])) as Arc<dyn AuditSink>)
        }
        AuditMode::Http => {
            let http_cfg =
                config.audit.http.as_ref().ok_or_else(|| {
                    "audit.mode=http requires audit.http configuration".to_owned()
                })?;
            build_http_audit_sink(http_cfg)
        }
        AuditMode::Syslog => {
            let syslog_cfg =
                config.audit.syslog.as_ref().ok_or_else(|| {
                    "audit.mode=syslog requires audit.syslog configuration".to_owned()
                })?;
            build_syslog_audit_sink(syslog_cfg)
        }
    }
}

fn build_jsonl_audit_sink(config: &LoongConfig) -> Result<Arc<dyn AuditSink>, String> {
    let path = config.audit.resolved_path();
    JsonlAuditSink::new(path.clone())
        .map(|sink| Arc::new(sink) as Arc<dyn AuditSink>)
        .map_err(|error| {
            format!(
                "failed to initialize durable audit journal {}: {error}",
                path.display()
            )
        })
}

fn build_http_audit_sink(config: &HttpAuditConfig) -> Result<Arc<dyn AuditSink>, String> {
    HttpAuditSink::new(config.clone()).map(|sink| Arc::new(sink) as Arc<dyn AuditSink>)
}

fn build_syslog_audit_sink(config: &SyslogAuditConfig) -> Result<Arc<dyn AuditSink>, String> {
    SyslogAuditSink::new(config.clone()).map(|sink| Arc::new(sink) as Arc<dyn AuditSink>)
}

/// HTTP remote audit sink.
///
/// Sends events in batches via POST to a configured URL. Events are buffered
/// in a synchronous mpsc channel and flushed by a background thread either
/// when the batch size is reached or the flush interval expires.
struct HttpAuditSink {
    queue: std::sync::mpsc::SyncSender<loong_contracts::AuditEvent>,
    worker_handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl HttpAuditSink {
    fn new(config: HttpAuditConfig) -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::sync_channel(config.batch_size.max(1) * 2);
        let worker_url = config.url.clone();
        let worker_auth = config.auth_header.clone();
        let worker_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("failed to build HTTP audit client: {e}"))?;
        let worker_client_clone = worker_client.clone();
        let worker_batch_size = config.batch_size;
        let worker_flush_interval = std::time::Duration::from_secs(config.flush_interval_s);

        let handle = std::thread::Builder::new()
            .name("http-audit-worker".to_owned())
            .spawn(move || {
                let mut batch: Vec<loong_contracts::AuditEvent> =
                    Vec::with_capacity(worker_batch_size);
                let mut deadline = std::time::Instant::now() + worker_flush_interval;

                loop {
                    let timeout = deadline.saturating_duration_since(std::time::Instant::now());
                    let msg = rx.recv_timeout(timeout);
                    match msg {
                        Ok(event) => {
                            batch.push(event);
                            if batch.len() >= worker_batch_size {
                                let _ = Self::flush_batch(
                                    &worker_client_clone,
                                    &worker_url,
                                    worker_auth.as_deref(),
                                    &mut batch,
                                );
                                deadline = std::time::Instant::now() + worker_flush_interval;
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                let _ = Self::flush_batch(
                                    &worker_client_clone,
                                    &worker_url,
                                    worker_auth.as_deref(),
                                    &mut batch,
                                );
                                deadline = std::time::Instant::now() + worker_flush_interval;
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            while let Ok(event) = rx.try_recv() {
                                batch.push(event);
                            }
                            if !batch.is_empty() {
                                let _ = Self::flush_batch(
                                    &worker_client_clone,
                                    &worker_url,
                                    worker_auth.as_deref(),
                                    &mut batch,
                                );
                            }
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("failed to spawn HTTP audit worker thread: {e}"))?;

        Ok(Self {
            queue: tx,
            worker_handle: std::sync::Mutex::new(Some(handle)),
        })
    }

    fn flush_batch(
        client: &reqwest::blocking::Client,
        url: &str,
        auth_header: Option<&str>,
        batch: &mut Vec<loong_contracts::AuditEvent>,
    ) {
        if batch.is_empty() {
            return;
        }
        let body = match serde_json::to_string(batch) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("HTTP audit sink failed to serialize batch: {e}");
                batch.clear();
                return;
            }
        };
        let mut req = client.post(url);
        if let Some(header) = auth_header {
            req = req.header("Authorization", header);
        }
        req = req
            .header("Content-Type", "application/json")
            .body(body)
            .timeout(std::time::Duration::from_secs(10));
        match req.send() {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                tracing::warn!(
                    "HTTP audit sink received non-success status {}: {}",
                    resp.status(),
                    resp.text().unwrap_or_default()
                );
            }
            Err(e) => {
                tracing::warn!("HTTP audit sink batch send failed: {e}");
            }
        }
        batch.clear();
    }
}

impl AuditSink for HttpAuditSink {
    fn record(&self, event: loong_contracts::AuditEvent) -> Result<(), AuditError> {
        self.queue
            .send(event)
            .map_err(|_| AuditError::Sink("HTTP audit worker channel closed".to_owned()))
    }
}

impl Drop for HttpAuditSink {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.worker_handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Syslog remote audit sink.
///
/// Sends events to a remote syslog receiver via UDP using RFC 5424 format.
/// Each event is transmitted as a single UDP datagram.
struct SyslogAuditSink {
    sock: std::net::UdpSocket,
    facility: u8,
    app_name: String,
    hostname: String,
}

impl SyslogAuditSink {
    fn new(config: SyslogAuditConfig) -> Result<Self, String> {
        let sock = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("failed to bind UDP socket for syslog audit: {e}"))?;
        sock.connect(format!("{}:{}", config.host, config.port))
            .map_err(|e| format!("failed to connect UDP socket for syslog audit: {e}"))?;
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_owned());
        Ok(Self {
            sock,
            facility: config.facility.code(),
            app_name: config.app_name,
            hostname,
        })
    }

    fn format_syslog(&self, event: &loong_contracts::AuditEvent) -> String {
        let pri = self.facility * 8 + 6; // severity = INFO (6)
        let ts = format_rfc5424_timestamp(event.timestamp_epoch_s);
        let msg = serde_json::to_string(&event).unwrap_or_default();
        // RFC 5424 HEADER: <PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID SD
        // VERSION is always 1.
        // PROCID and MSGID are "-" (not used).
        // SD (structured-data) is "-" (no SD-ELEMENTs).
        // MSG follows as unstructured text after a single space.
        format!(
            "<{}>1 {} {} {} - - - {}",
            pri, ts, self.hostname, self.app_name, msg
        )
    }
}

impl AuditSink for SyslogAuditSink {
    fn record(&self, event: loong_contracts::AuditEvent) -> Result<(), AuditError> {
        let msg = self.format_syslog(&event);
        self.sock
            .send(msg.as_bytes())
            .map_err(|e| AuditError::Sink(format!("syslog send failed: {e}")))
            .map(|_| ())
    }
}

/// Format a Unix timestamp as an RFC 5424 timestamp (YYYY-MM-DDThh:mm:ssZ).
fn format_rfc5424_timestamp(epoch_s: u64) -> String {
    let secs = epoch_s as i64;
    let total_days = secs / 86400;
    let rem_secs = secs % 86400;
    let mut year = 1970;
    let mut remaining_days = total_days;
    while remaining_days >= 365 {
        let leap = if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
            366
        } else {
            365
        };
        if remaining_days < leap {
            break;
        }
        remaining_days -= leap;
        year += 1;
    }
    let is_leap = year % 400 == 0 || (year % 4 == 0 && year % 100 != 0);
    let mut month = 1;
    let mut day = remaining_days + 1;
    const DAYS_PER_MONTH: &[i64] = &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    const DAYS_PER_MONTH_LEAP: &[i64] = &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let days_in_months: &[i64] = if is_leap { DAYS_PER_MONTH_LEAP } else { DAYS_PER_MONTH };
    for &dim in days_in_months {
        if day <= dim {
            break;
        }
        day -= dim;
        month += 1;
    }
    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

fn bootstrap_kernel_context_with_audit_sink(
    agent_id: &str,
    ttl_s: u64,
    audit_sink: Arc<dyn AuditSink>,
    config: &LoongConfig,
) -> Result<KernelContext, String> {
    let mut kernel = LoongKernel::with_runtime(
        StaticPolicyEngine::default(),
        Arc::new(SystemClock) as Arc<dyn Clock>,
        audit_sink,
    );

    let pack = VerticalPackManifest {
        pack_id: MVP_PACK_ID.to_owned(),
        domain: "mvp".to_owned(),
        version: "0.1.0".to_owned(),
        default_route: ExecutionRoute {
            harness_kind: HarnessKind::EmbeddedPi,
            adapter: None,
        },
        allowed_connectors: BTreeSet::new(),
        granted_capabilities: BTreeSet::from([
            Capability::InvokeTool,
            Capability::NetworkEgress,
            Capability::MemoryRead,
            Capability::MemoryWrite,
            Capability::FilesystemRead,
            Capability::FilesystemWrite,
        ]),
        metadata: BTreeMap::new(),
    };

    kernel
        .register_pack(pack)
        .map_err(|e| format!("kernel pack registration failed: {e}"))?;

    #[cfg(feature = "memory-sqlite")]
    {
        let mem_config =
            crate::memory::runtime_config::MemoryRuntimeConfig::from_memory_config_without_env_overrides(
                &config.memory,
            );
        kernel
            .register_core_memory_adapter(crate::memory::MvpMemoryAdapter::with_config(mem_config));
        kernel
            .set_default_core_memory_adapter("mvp-memory")
            .map_err(|e| format!("set default memory adapter failed: {e}"))?;
    }

    let tool_rt = crate::tools::runtime_config::ToolRuntimeConfig::from_loong_config(config, None);
    let file_root = tool_rt.file_root.clone();
    kernel.register_core_tool_adapter(crate::tools::MvpToolAdapter::with_config(tool_rt));
    kernel
        .set_default_core_tool_adapter("mvp-tools")
        .map_err(|e| format!("set default tool adapter failed: {e}"))?;

    // Register policy extensions for unified security enforcement.
    let tool_policy_rt =
        crate::tools::runtime_config::ToolRuntimeConfig::from_loong_config(config, None);
    kernel.register_policy_extension(
        crate::tools::shell_policy_ext::ToolPolicyExtension::from_config(&tool_policy_rt),
    );
    kernel.register_policy_extension(crate::tools::file_policy_ext::FilePolicyExtension::new(
        file_root,
    ));

    let token = kernel
        .issue_token(MVP_PACK_ID, agent_id, ttl_s)
        .map_err(|e| format!("kernel token issue failed: {e}"))?;

    Ok(KernelContext {
        kernel: Arc::new(kernel),
        token,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn bootstrap_kernel_context_with_config_writes_jsonl_audit_events() {
        let tempdir = tempdir().expect("tempdir");
        let audit_path = tempdir.path().join("audit").join("events.jsonl");
        let mut config = LoongConfig::default();
        config.audit.mode = AuditMode::Jsonl;
        config.audit.path = audit_path.display().to_string();
        config.audit.retain_in_memory = false;

        let context = bootstrap_kernel_context_with_config("test-agent", 60, &config)
            .expect("bootstrap with jsonl audit should succeed");

        assert_eq!(context.agent_id(), "test-agent");

        let journal = fs::read_to_string(&audit_path).expect("audit journal should exist");
        assert_eq!(
            journal.lines().count(),
            1,
            "token bootstrap should emit one audit event"
        );
        assert!(
            journal.contains("\"TokenIssued\"") || journal.contains("\"token_id\""),
            "bootstrap journal should capture token issuance"
        );
    }

    #[test]
    fn bootstrap_kernel_context_with_config_writes_fanout_audit_events() {
        let tempdir = tempdir().expect("tempdir");
        let audit_path = tempdir.path().join("audit").join("events.jsonl");
        let mut config = LoongConfig::default();
        config.audit.mode = AuditMode::Fanout;
        config.audit.path = audit_path.display().to_string();
        config.audit.retain_in_memory = true;

        let context = bootstrap_kernel_context_with_config("test-agent", 60, &config)
            .expect("bootstrap with fanout audit should succeed");

        assert_eq!(context.agent_id(), "test-agent");

        let journal = fs::read_to_string(&audit_path).expect("audit journal should exist");
        assert_eq!(
            journal.lines().count(),
            1,
            "token bootstrap should emit one audit event"
        );
        assert!(
            journal.contains("\"TokenIssued\"") || journal.contains("\"token_id\""),
            "fanout journal should capture token issuance"
        );
    }

    #[test]
    fn bootstrap_kernel_context_with_config_grants_network_egress() {
        let mut config = LoongConfig::default();
        config.audit.mode = AuditMode::InMemory;

        let context = bootstrap_kernel_context_with_config("test-agent", 60, &config)
            .expect("bootstrap with default config should succeed");

        let allowed_capabilities = &context.token.allowed_capabilities;

        assert!(
            allowed_capabilities.contains(&Capability::InvokeTool),
            "bootstrap token should retain invoke tool capability"
        );
        assert!(
            allowed_capabilities.contains(&Capability::NetworkEgress),
            "bootstrap token should grant network egress for kernel-bound web tools"
        );
    }
}
