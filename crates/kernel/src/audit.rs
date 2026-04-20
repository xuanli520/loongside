use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// Re-export data types from contracts
pub use loong_contracts::{AuditEvent, AuditEventKind, ExecutionPlane, PlaneTier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::AuditError;

pub trait AuditSink: Send + Sync {
    fn record(&self, event: AuditEvent) -> Result<(), AuditError>;
}

#[derive(Debug, Default)]
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _event: AuditEvent) -> Result<(), AuditError> {
        Ok(())
    }
}

/// In-memory audit event buffer with a configurable capacity.
///
/// Uses a ring-buffer strategy: once `capacity` events are stored, each new
/// record evicts the oldest one. This prevents unbounded memory growth while
/// keeping the most recent events available for in-process queries.
///
/// The `snapshot()` method returns all events currently in the buffer (up to
/// `capacity`). The `snapshot_filtered()` method applies an `AuditSnapshotFilter`
/// to that same view.
#[derive(Debug)]
pub struct InMemoryAuditSink {
    events: Arc<Mutex<Vec<AuditEvent>>>,
    /// Maximum number of events to retain in memory.
    capacity: usize,
}

/// Default in-memory capacity used when `InMemoryAuditSink::default()` is used.
const DEFAULT_IN_MEMORY_AUDIT_CAPACITY: usize = 10_000;

impl Default for InMemoryAuditSink {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_IN_MEMORY_AUDIT_CAPACITY)
    }
}

impl InMemoryAuditSink {
    /// Construct a new sink that retains at most `capacity` of the most recent events.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
            capacity,
        }
    }

    /// Return all events currently in the buffer.
    ///
    /// Callers that need only a subset should use `snapshot_filtered` instead.
    #[must_use]
    pub fn snapshot(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map_or_else(|_| Vec::new(), |guard| guard.clone())
    }

    /// Return events that match the given filter.
    ///
    /// The filter is applied to the in-memory buffer only; it has no access to
    /// events that have already been evicted due to the capacity limit.
    #[must_use]
    pub fn snapshot_filtered(&self, filter: &AuditSnapshotFilter) -> Vec<AuditEvent> {
        let Ok(guard) = self.events.lock() else {
            return Vec::new();
        };
        guard.iter().filter(|e| filter.matches(e)).cloned().collect()
    }

    /// Current number of events held in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.lock().map_or(0, |g| g.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditSink for InMemoryAuditSink {
    fn record(&self, event: AuditEvent) -> Result<(), AuditError> {
        let mut guard = self
            .events
            .lock()
            .map_err(|_| AuditError::Sink("audit mutex poisoned".to_owned()))?;

        if guard.len() >= self.capacity {
            // Ring-buffer eviction: discard the oldest record.
            guard.remove(0);
        }
        guard.push(event);
        Ok(())
    }
}

#[derive(Debug)]
struct JsonlAuditJournalState {
    file: File,
    last_entry_hash: Option<String>,
}

#[derive(Debug)]
pub struct JsonlAuditSink {
    path: PathBuf,
    journal: Mutex<JsonlAuditJournalState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditVerificationReport {
    pub total_events: usize,
    pub verified_events: usize,
    pub valid: bool,
    pub last_entry_hash: Option<String>,
    pub first_invalid_line: Option<usize>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedAuditIntegrity {
    algorithm: String,
    prev_hash: Option<String>,
    entry_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedAuditEvent {
    #[serde(flatten)]
    event: AuditEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    integrity: Option<PersistedAuditIntegrity>,
}

fn prepare_audit_journal_parent(path: &Path) -> Result<(), AuditError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            AuditError::Sink(format!(
                "failed to prepare audit journal parent directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }

    Ok(())
}

fn open_jsonl_audit_journal(path: &Path) -> Result<File, AuditError> {
    prepare_audit_journal_parent(path)?;

    OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            AuditError::Sink(format!(
                "failed to open audit journal `{}`: {error}",
                path.display()
            ))
        })
}

fn lock_audit_journal(journal: &File, path: &Path) -> Result<(), AuditError> {
    journal.lock().map_err(|error| {
        AuditError::Sink(format!(
            "failed to lock audit journal `{}`: {error}",
            path.display()
        ))
    })
}

fn unlock_audit_journal(journal: &File, path: &Path) -> Result<(), AuditError> {
    journal.unlock().map_err(|error| {
        AuditError::Sink(format!(
            "failed to unlock audit journal `{}`: {error}",
            path.display()
        ))
    })
}

/// Exercise the same open + lock + unlock path that production audit writes use.
pub fn probe_jsonl_audit_journal_runtime_ready(path: &Path) -> Result<(), AuditError> {
    let journal = open_jsonl_audit_journal(path)?;
    lock_audit_journal(&journal, path)?;
    unlock_audit_journal(&journal, path)
}

impl JsonlAuditSink {
    pub fn new(path: PathBuf) -> Result<Self, AuditError> {
        let journal = open_jsonl_audit_journal(&path)?;
        let last_entry_hash = load_last_audit_entry_hash(&path)?;

        Ok(Self {
            path,
            journal: Mutex::new(JsonlAuditJournalState {
                file: journal,
                last_entry_hash,
            }),
        })
    }
}

fn serialize_audit_event_chain_material(
    event: &AuditEvent,
    prev_hash: Option<&str>,
    journal_path: &Path,
) -> Result<Vec<u8>, AuditError> {
    serde_json::to_vec(&serde_json::json!({
        "event_id": event.event_id,
        "timestamp_epoch_s": event.timestamp_epoch_s,
        "agent_id": event.agent_id,
        "kind": event.kind,
        "prev_hash": prev_hash,
    }))
    .map_err(|error| {
        AuditError::Sink(format!(
            "failed to serialize audit chain material for `{}`: {error}",
            journal_path.display()
        ))
    })
}

fn compute_audit_event_entry_hash(
    event: &AuditEvent,
    prev_hash: Option<&str>,
    journal_path: &Path,
) -> Result<String, AuditError> {
    let material = serialize_audit_event_chain_material(event, prev_hash, journal_path)?;
    let digest = Sha256::digest(material);
    let encoded = hex::encode(digest);
    Ok(encoded)
}

fn event_with_integrity(
    event: AuditEvent,
    prev_hash: Option<String>,
    entry_hash: String,
) -> PersistedAuditEvent {
    let integrity = PersistedAuditIntegrity {
        algorithm: "sha256".to_owned(),
        prev_hash,
        entry_hash,
    };

    PersistedAuditEvent {
        event,
        integrity: Some(integrity),
    }
}

fn decode_persisted_audit_event_line(
    line: &str,
    journal_path: &Path,
    line_number: &str,
) -> Result<PersistedAuditEvent, AuditError> {
    serde_json::from_str::<PersistedAuditEvent>(line).map_err(|error| {
        AuditError::Sink(format!(
            "failed to decode audit journal `{}` at {}: {error}",
            journal_path.display(),
            line_number
        ))
    })
}

fn load_last_audit_entry_hash(path: &Path) -> Result<Option<String>, AuditError> {
    if !path.exists() {
        return Ok(None);
    }

    let file = File::open(path).map_err(|error| {
        AuditError::Sink(format!(
            "failed to inspect audit journal `{}`: {error}",
            path.display()
        ))
    })?;
    let reader = BufReader::new(file);
    let mut last_non_empty_line = None;

    for line_result in reader.lines() {
        let line = line_result.map_err(|error| {
            AuditError::Sink(format!(
                "failed to read audit journal `{}` while loading tail hash: {error}",
                path.display()
            ))
        })?;
        if !line.trim().is_empty() {
            last_non_empty_line = Some(line);
        }
    }

    let Some(line) = last_non_empty_line else {
        return Ok(None);
    };

    let persisted_event = decode_persisted_audit_event_line(&line, path, "tail line")?;
    let last_hash = persisted_event.integrity.and_then(|value| {
        let hash = value.entry_hash;
        let trimmed_hash = hash.trim();
        if trimmed_hash.is_empty() {
            return None;
        }
        Some(hash)
    });

    Ok(last_hash)
}

pub fn verify_jsonl_audit_journal(path: &Path) -> Result<AuditVerificationReport, AuditError> {
    if !path.exists() {
        return Ok(AuditVerificationReport {
            total_events: 0,
            verified_events: 0,
            valid: true,
            last_entry_hash: None,
            first_invalid_line: None,
            reason: None,
        });
    }

    let file = File::open(path).map_err(|error| {
        AuditError::Sink(format!(
            "failed to open audit journal `{}` for verification: {error}",
            path.display()
        ))
    })?;
    let reader = BufReader::new(file);
    let mut total_events = 0usize;
    let mut verified_events = 0usize;
    let mut previous_hash: Option<String> = None;
    let mut protected_chain_started = false;

    for (index, line_result) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line_result.map_err(|error| {
            AuditError::Sink(format!(
                "failed to read audit journal `{}` at line {}: {error}",
                path.display(),
                line_number
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }

        total_events += 1;
        let line_label = format!("line {line_number}");
        let persisted_event = decode_persisted_audit_event_line(&line, path, &line_label)?;
        let event = persisted_event.event;
        let Some(integrity) = persisted_event.integrity.as_ref() else {
            if protected_chain_started {
                return Ok(AuditVerificationReport {
                    total_events,
                    verified_events,
                    valid: false,
                    last_entry_hash: previous_hash,
                    first_invalid_line: Some(line_number),
                    reason: Some("missing integrity envelope".to_owned()),
                });
            }

            continue;
        };

        if integrity.algorithm.trim() != "sha256" {
            return Ok(AuditVerificationReport {
                total_events,
                verified_events,
                valid: false,
                last_entry_hash: previous_hash,
                first_invalid_line: Some(line_number),
                reason: Some(format!(
                    "unsupported integrity algorithm `{}`",
                    integrity.algorithm
                )),
            });
        }

        protected_chain_started = true;

        if integrity.prev_hash != previous_hash {
            return Ok(AuditVerificationReport {
                total_events,
                verified_events,
                valid: false,
                last_entry_hash: previous_hash,
                first_invalid_line: Some(line_number),
                reason: Some("prev_hash mismatch".to_owned()),
            });
        }

        let expected_hash =
            compute_audit_event_entry_hash(&event, integrity.prev_hash.as_deref(), path)?;

        if integrity.entry_hash != expected_hash {
            return Ok(AuditVerificationReport {
                total_events,
                verified_events,
                valid: false,
                last_entry_hash: previous_hash,
                first_invalid_line: Some(line_number),
                reason: Some("entry_hash mismatch".to_owned()),
            });
        }

        previous_hash = Some(integrity.entry_hash.clone());
        verified_events += 1;
    }

    Ok(AuditVerificationReport {
        total_events,
        verified_events,
        valid: true,
        last_entry_hash: previous_hash,
        first_invalid_line: None,
        reason: None,
    })
}

/// Filter parameters for querying an audit snapshot.
/// All fields are optional — None means "don't filter on this field".
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AuditSnapshotFilter {
    /// Inclusive lower bound on event timestamp (epoch seconds).
    pub since_epoch_s: Option<u64>,
    /// Inclusive upper bound on event timestamp (epoch seconds).
    pub until_epoch_s: Option<u64>,
    /// Match only events for this agent_id.
    pub agent_id: Option<String>,
    /// Match only events whose discriminant matches one of these kind names.
    /// For example: "TokenIssued", "AuthorizationDenied".
    pub kinds: Option<Vec<&'static str>>,
}

impl AuditSnapshotFilter {
    /// Returns true if the given event matches every non-None field in the filter.
    pub fn matches(&self, event: &AuditEvent) -> bool {
        if let Some(since) = self.since_epoch_s {
            if event.timestamp_epoch_s < since {
                return false;
            }
        }
        if let Some(until) = self.until_epoch_s {
            if event.timestamp_epoch_s > until {
                return false;
            }
        }
        if let Some(ref agent_id) = self.agent_id {
            if event.agent_id.as_deref() != Some(agent_id.as_str()) {
                return false;
            }
        }
        if let Some(ref kinds) = self.kinds {
            if kinds.is_empty() {
                return true;
            }
            let kind_name = event.kind.name();
            if !kinds.contains(&kind_name) {
                return false;
            }
        }
        true
    }
}

/// Minimal interface for accessing the name of an AuditEventKind variant.
trait AuditEventKindExt {
    fn name(&self) -> &'static str;
}

impl AuditEventKindExt for AuditEventKind {
    fn name(&self) -> &'static str {
        match self {
            AuditEventKind::TokenIssued { .. } => "TokenIssued",
            AuditEventKind::TokenRevoked { .. } => "TokenRevoked",
            AuditEventKind::TaskDispatched { .. } => "TaskDispatched",
            AuditEventKind::ConnectorInvoked { .. } => "ConnectorInvoked",
            AuditEventKind::PlaneInvoked { .. } => "PlaneInvoked",
            AuditEventKind::SecurityScanEvaluated { .. } => "SecurityScanEvaluated",
            AuditEventKind::PluginTrustEvaluated { .. } => "PluginTrustEvaluated",
            AuditEventKind::ToolSearchEvaluated { .. } => "ToolSearchEvaluated",
            AuditEventKind::ProviderFailover { .. } => "ProviderFailover",
            AuditEventKind::AuthorizationDenied { .. } => "AuthorizationDenied",
            _ => "Unknown",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum AuditRepairOutcome {
    #[default]
    Healthy,
    Repaired,
    Refused { line: usize, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRepairReport {
    pub total_events: usize,
    pub repaired_events: usize,
    pub already_valid_events: usize,
    pub outcome: AuditRepairOutcome,
}

/// Repair legacy journal entries that are missing integrity envelopes.
///
/// **Must be run while the daemon is stopped.** A running `JsonlAuditSink` holds
/// an open file handle and cached tail hash that would be invalidated by the
/// atomic rename.
pub fn repair_jsonl_audit_journal(path: &Path) -> Result<AuditRepairReport, AuditError> {
    if !path.exists() {
        return Ok(AuditRepairReport {
            total_events: 0,
            repaired_events: 0,
            already_valid_events: 0,
            outcome: AuditRepairOutcome::Healthy,
        });
    }

    let file = File::open(path).map_err(|error| {
        AuditError::Sink(format!(
            "failed to open audit journal `{}` for repair: {error}",
            path.display()
        ))
    })?;
    let original_metadata = fs::metadata(path).map_err(|error| {
        AuditError::Sink(format!(
            "failed to read audit journal metadata `{}` before repair: {error}",
            path.display()
        ))
    })?;
    let original_permissions = original_metadata.permissions();

    let reader = BufReader::new(file);
    let mut repaired_lines: Vec<Vec<u8>> = Vec::new();
    let mut rebuilt_previous_hash: Option<String> = None;
    let mut source_previous_hash: Option<String> = None;
    let mut protected_chain_started = false;
    let mut total_events = 0usize;
    let mut repaired_events = 0usize;
    let mut already_valid_events = 0usize;

    for (index, line_result) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line_result.map_err(|error| {
            AuditError::Sink(format!(
                "failed to read audit journal `{}` at line {line_number}: {error}",
                path.display()
            ))
        })?;
        if line.trim().is_empty() {
            repaired_lines.push(b"\n".to_vec());
            continue;
        }

        total_events += 1;
        let line_label = format!("line {line_number}");
        let persisted = decode_persisted_audit_event_line(&line, path, &line_label)?;
        let event = persisted.event;

        if let Some(integrity) = persisted.integrity.as_ref() {
            if integrity.algorithm.trim() != "sha256" {
                return Ok(AuditRepairReport {
                    total_events,
                    repaired_events,
                    already_valid_events,
                    outcome: AuditRepairOutcome::Refused {
                        line: line_number,
                        reason: format!(
                            "unsupported integrity algorithm `{}`",
                            integrity.algorithm
                        ),
                    },
                });
            }

            // Validate source chain: prev_hash must match the previous
            // source entry_hash (mirrors verify_jsonl_audit_journal).
            if integrity.prev_hash != source_previous_hash {
                return Ok(AuditRepairReport {
                    total_events,
                    repaired_events,
                    already_valid_events,
                    outcome: AuditRepairOutcome::Refused {
                        line: line_number,
                        reason: "prev_hash mismatch in source chain".to_owned(),
                    },
                });
            }

            // Check self-consistency: does entry_hash match the event data?
            let self_consistent_hash =
                compute_audit_event_entry_hash(&event, integrity.prev_hash.as_deref(), path)?;
            if integrity.entry_hash != self_consistent_hash {
                return Ok(AuditRepairReport {
                    total_events,
                    repaired_events,
                    already_valid_events,
                    outcome: AuditRepairOutcome::Refused {
                        line: line_number,
                        reason: "entry_hash mismatch — event data may be tampered".to_owned(),
                    },
                });
            }

            protected_chain_started = true;
            source_previous_hash = Some(integrity.entry_hash.clone());

            if repaired_events == 0 && integrity.prev_hash == rebuilt_previous_hash {
                // No prior repairs and chain is continuous — keep as-is.
                rebuilt_previous_hash = Some(integrity.entry_hash.clone());
                already_valid_events += 1;
                let mut encoded = line.into_bytes();
                encoded.push(b'\n');
                repaired_lines.push(encoded);
            } else {
                // Prior legacy entries were repaired, so the chain position
                // changed. Re-seal this entry with the rebuilt prev_hash.
                let entry_hash =
                    compute_audit_event_entry_hash(&event, rebuilt_previous_hash.as_deref(), path)?;
                let resealed =
                    event_with_integrity(event, rebuilt_previous_hash.clone(), entry_hash.clone());
                let encoded = serialize_audit_event_line(&resealed, path)?;
                repaired_lines.push(encoded);
                rebuilt_previous_hash = Some(entry_hash);
                repaired_events += 1;
            }
        } else {
            if protected_chain_started {
                return Ok(AuditRepairReport {
                    total_events,
                    repaired_events,
                    already_valid_events,
                    outcome: AuditRepairOutcome::Refused {
                        line: line_number,
                        reason: "missing integrity envelope after protected chain started"
                            .to_owned(),
                    },
                });
            }
            let entry_hash =
                compute_audit_event_entry_hash(&event, rebuilt_previous_hash.as_deref(), path)?;
            let repaired =
                event_with_integrity(event, rebuilt_previous_hash.clone(), entry_hash.clone());
            let encoded = serialize_audit_event_line(&repaired, path)?;
            repaired_lines.push(encoded);
            rebuilt_previous_hash = Some(entry_hash);
            repaired_events += 1;
        }
    }

    if repaired_events == 0 {
        return Ok(AuditRepairReport {
            total_events,
            repaired_events: 0,
            already_valid_events,
            outcome: AuditRepairOutcome::Healthy,
        });
    }

    let temp_path = path.with_extension("jsonl.repair-tmp");
    let write_result = (|| {
        let mut temp_file = File::create(&temp_path).map_err(|error| {
            AuditError::Sink(format!(
                "failed to create repair temp file `{}`: {error}",
                temp_path.display()
            ))
        })?;
        fs::set_permissions(&temp_path, original_permissions.clone()).map_err(|error| {
            AuditError::Sink(format!(
                "failed to apply original permissions to repair temp file `{}`: {error}",
                temp_path.display()
            ))
        })?;
        for line_bytes in &repaired_lines {
            temp_file.write_all(line_bytes).map_err(|error| {
                AuditError::Sink(format!(
                    "failed to write repair temp file `{}`: {error}",
                    temp_path.display()
                ))
            })?;
        }
        temp_file.flush().map_err(|error| {
            AuditError::Sink(format!(
                "failed to flush repair temp file `{}`: {error}",
                temp_path.display()
            ))
        })?;
        temp_file.sync_all().map_err(|error| {
            AuditError::Sink(format!(
                "failed to sync repair temp file `{}`: {error}",
                temp_path.display()
            ))
        })?;
        drop(temp_file);
        fs::rename(&temp_path, path).map_err(|error| {
            AuditError::Sink(format!(
                "failed to replace journal with repaired file `{}`: {error}",
                path.display()
            ))
        })
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result?;

    Ok(AuditRepairReport {
        total_events,
        repaired_events,
        already_valid_events,
        outcome: AuditRepairOutcome::Repaired,
    })
}

fn serialize_audit_event_line(
    event: &PersistedAuditEvent,
    journal_path: &Path,
) -> Result<Vec<u8>, AuditError> {
    let mut encoded = serde_json::to_vec(event).map_err(|error| {
        AuditError::Sink(format!(
            "failed to serialize audit event for `{}`: {error}",
            journal_path.display()
        ))
    })?;
    encoded.push(b'\n');
    Ok(encoded)
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, event: AuditEvent) -> Result<(), AuditError> {
        let mut guard = self
            .journal
            .lock()
            .map_err(|_error| AuditError::Sink("audit journal mutex poisoned".to_owned()))?;
        let previous_hash = guard.last_entry_hash.clone();
        let entry_hash =
            compute_audit_event_entry_hash(&event, previous_hash.as_deref(), &self.path)?;
        let persisted_event = event_with_integrity(event, previous_hash, entry_hash.clone());
        let encoded = serialize_audit_event_line(&persisted_event, &self.path)?;

        lock_audit_journal(&guard.file, &self.path)?;

        let write_result = guard
            .file
            .write_all(&encoded)
            .map_err(|error| {
                AuditError::Sink(format!(
                    "failed to append audit event to `{}`: {error}",
                    self.path.display()
                ))
            })
            .and_then(|()| {
                guard.file.flush().map_err(|error| {
                    AuditError::Sink(format!(
                        "failed to flush audit journal `{}`: {error}",
                        self.path.display()
                    ))
                })
            });

        let unlock_result = unlock_audit_journal(&guard.file, &self.path);

        match (write_result, unlock_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => {
                guard.last_entry_hash = Some(entry_hash);
                Ok(())
            }
        }
    }
}

pub struct FanoutAuditSink {
    children: Vec<Arc<dyn AuditSink>>,
}

impl FanoutAuditSink {
    #[must_use]
    pub fn new(children: Vec<Arc<dyn AuditSink>>) -> Self {
        assert!(
            !children.is_empty(),
            "fanout audit sink requires at least one child"
        );
        Self { children }
    }
}

impl AuditSink for FanoutAuditSink {
    fn record(&self, event: AuditEvent) -> Result<(), AuditError> {
        if let Some((last, rest)) = self.children.split_last() {
            for sink in rest {
                sink.record(event.clone())?;
            }
            last.record(event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loong_contracts::CapabilityToken;

    fn make_event(timestamp_epoch_s: u64, agent_id: Option<&str>, kind: AuditEventKind) -> AuditEvent {
        AuditEvent {
            event_id: format!("evt-{}", timestamp_epoch_s),
            timestamp_epoch_s,
            agent_id: agent_id.map(String::from),
            kind,
        }
    }

    fn token_issued(timestamp_epoch_s: u64, agent_id: Option<&str>) -> AuditEvent {
        make_event(
            timestamp_epoch_s,
            agent_id,
            AuditEventKind::TokenIssued {
                token: CapabilityToken {
                    token_id: format!("tok-{}", timestamp_epoch_s),
                    pack_id: "test-pack".to_owned(),
                    agent_id: agent_id.map(String::from).unwrap_or_default(),
                    allowed_capabilities: Default::default(),
                    issued_at_epoch_s: timestamp_epoch_s,
                    expires_at_epoch_s: timestamp_epoch_s.saturating_add(3600),
                    generation: 0,
                },
            },
        )
    }

    fn authorization_denied(timestamp_epoch_s: u64, agent_id: Option<&str>) -> AuditEvent {
        make_event(
            timestamp_epoch_s,
            agent_id,
            AuditEventKind::AuthorizationDenied {
                pack_id: "test-pack".to_owned(),
                token_id: format!("tok-{}", timestamp_epoch_s),
                reason: "denied".to_owned(),
            },
        )
    }

    // -------------------------------------------------------------------------
    // AuditSnapshotFilter tests
    // -------------------------------------------------------------------------

    #[test]
    fn filter_empty_matches_everything() {
        let filter = AuditSnapshotFilter::default();
        let evt = token_issued(1000, None);
        assert!(filter.matches(&evt));
    }

    #[test]
    fn filter_since_excludes_older_events() {
        let filter = AuditSnapshotFilter {
            since_epoch_s: Some(1000),
            ..Default::default()
        };
        assert!(!filter.matches(&token_issued(999, None)));
        assert!(filter.matches(&token_issued(1000, None)));
        assert!(filter.matches(&token_issued(2000, None)));
    }

    #[test]
    fn filter_until_excludes_newer_events() {
        let filter = AuditSnapshotFilter {
            until_epoch_s: Some(2000),
            ..Default::default()
        };
        assert!(filter.matches(&token_issued(1000, None)));
        assert!(filter.matches(&token_issued(2000, None)));
        assert!(!filter.matches(&token_issued(2001, None)));
    }

    #[test]
    fn filter_agent_id_matches() {
        let filter = AuditSnapshotFilter {
            agent_id: Some("agent-a".to_owned()),
            ..Default::default()
        };
        let evt_a = token_issued(1000, Some("agent-a"));
        let evt_b = token_issued(1000, Some("agent-b"));
        let evt_none = token_issued(1000, None);
        assert!(filter.matches(&evt_a));
        assert!(!filter.matches(&evt_b));
        assert!(!filter.matches(&evt_none));
    }

    #[test]
    fn filter_kinds_matches() {
        let filter = AuditSnapshotFilter {
            kinds: Some(vec!["TokenIssued"]),
            ..Default::default()
        };
        assert!(filter.matches(&token_issued(1000, None)));
        assert!(!filter.matches(&authorization_denied(1000, None)));
    }

    #[test]
    fn filter_kinds_empty_is_no_op() {
        let filter = AuditSnapshotFilter {
            kinds: Some(vec![]),
            ..Default::default()
        };
        assert!(filter.matches(&token_issued(1000, None)));
        assert!(filter.matches(&authorization_denied(1000, None)));
    }

    #[test]
    fn filter_combined() {
        let filter = AuditSnapshotFilter {
            since_epoch_s: Some(1000),
            until_epoch_s: Some(2000),
            agent_id: Some("agent-a".to_owned()),
            kinds: Some(vec!["TokenIssued"]),
        };
        // within range, correct agent, correct kind
        assert!(filter.matches(&token_issued(1500, Some("agent-a"))));
        // out of range
        assert!(!filter.matches(&token_issued(999, Some("agent-a"))));
        assert!(!filter.matches(&token_issued(2001, Some("agent-a"))));
        // wrong agent
        assert!(!filter.matches(&token_issued(1500, Some("agent-b"))));
        // wrong kind
        assert!(!filter.matches(&authorization_denied(1500, Some("agent-a"))));
    }

    // -------------------------------------------------------------------------
    // InMemoryAuditSink ring-buffer tests
    // -------------------------------------------------------------------------

    #[test]
    fn in_memory_sink_records_and_snapshots() {
        let sink = InMemoryAuditSink::with_capacity(100);
        sink.record(token_issued(1, Some("a"))).unwrap();
        sink.record(token_issued(2, Some("b"))).unwrap();
        let snap = sink.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn in_memory_sink_evicts_oldest_when_full() {
        let sink = InMemoryAuditSink::with_capacity(3);
        sink.record(token_issued(1, Some("a"))).unwrap();
        sink.record(token_issued(2, Some("b"))).unwrap();
        sink.record(token_issued(3, Some("c"))).unwrap();
        // Fill to capacity
        sink.record(token_issued(4, Some("d"))).unwrap();

        let snap = sink.snapshot();
        assert_eq!(snap.len(), 3);
        // Events 1 should be evicted, 2,3,4 remain
        let ids: Vec<_> = snap.iter().map(|e| e.event_id.clone()).collect();
        assert!(ids.contains(&"evt-2".to_owned()));
        assert!(ids.contains(&"evt-3".to_owned()));
        assert!(ids.contains(&"evt-4".to_owned()));
        assert!(!ids.contains(&"evt-1".to_owned()));
    }

    #[test]
    fn in_memory_sink_len_and_is_empty() {
        let sink = InMemoryAuditSink::with_capacity(10);
        assert!(sink.is_empty());
        assert_eq!(sink.len(), 0);
        sink.record(token_issued(1, None)).unwrap();
        assert!(!sink.is_empty());
        assert_eq!(sink.len(), 1);
    }

    #[test]
    fn snapshot_filtered_returns_matching_events() {
        let sink = InMemoryAuditSink::with_capacity(100);
        sink.record(token_issued(1, Some("a"))).unwrap();
        sink.record(authorization_denied(2, Some("b"))).unwrap();
        sink.record(token_issued(3, Some("c"))).unwrap();

        let filter = AuditSnapshotFilter {
            kinds: Some(vec!["TokenIssued"]),
            ..Default::default()
        };
        let snap = sink.snapshot_filtered(&filter);
        assert_eq!(snap.len(), 2);
        assert!(snap.iter().all(|e| matches!(e.kind, AuditEventKind::TokenIssued { .. })));
    }

    #[test]
    fn snapshot_filtered_time_window() {
        let sink = InMemoryAuditSink::with_capacity(100);
        sink.record(token_issued(100, None)).unwrap();
        sink.record(token_issued(200, None)).unwrap();
        sink.record(token_issued(300, None)).unwrap();

        let filter = AuditSnapshotFilter {
            since_epoch_s: Some(150),
            until_epoch_s: Some(250),
            ..Default::default()
        };
        let snap = sink.snapshot_filtered(&filter);
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].timestamp_epoch_s, 200);
    }

    #[test]
    fn snapshot_filtered_agent_id() {
        let sink = InMemoryAuditSink::with_capacity(100);
        sink.record(token_issued(1, Some("alice"))).unwrap();
        sink.record(token_issued(2, Some("bob"))).unwrap();

        let filter = AuditSnapshotFilter {
            agent_id: Some("alice".to_owned()),
            ..Default::default()
        };
        let snap = sink.snapshot_filtered(&filter);
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].agent_id.as_deref(), Some("alice"));
    }

    #[test]
    fn snapshot_filtered_combined() {
        let sink = InMemoryAuditSink::with_capacity(100);
        sink.record(token_issued(100, Some("alice"))).unwrap();
        sink.record(authorization_denied(200, Some("alice"))).unwrap();
        sink.record(token_issued(200, Some("bob"))).unwrap();
        // Alice's TokenIssued at ts=250 passes all three filter conditions:
        // ts >= 150, agent_id == alice, kind == TokenIssued
        sink.record(token_issued(250, Some("alice"))).unwrap();

        let filter = AuditSnapshotFilter {
            since_epoch_s: Some(150),
            until_epoch_s: None,
            agent_id: Some("alice".to_owned()),
            kinds: Some(vec!["TokenIssued"]),
        };

        let snap = sink.snapshot_filtered(&filter);
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].agent_id.as_deref(), Some("alice"));
        assert_eq!(snap[0].timestamp_epoch_s, 250);
    }
}
