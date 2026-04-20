use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::shared::{default_loong_home, expand_path};

/// Syslog facility used in RFC 5424 structured-data headers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyslogFacility {
    /// Local use 0 (default for custom applications).
    #[default]
    Local0,
    /// Local use 1.
    Local1,
    /// Local use 2.
    Local2,
    /// Local use 3.
    Local3,
    /// Local use 4.
    Local4,
    /// Local use 5.
    Local5,
    /// Local use 6.
    Local6,
    /// Local use 7.
    Local7,
}

impl SyslogFacility {
    pub const fn code(self) -> u8 {
        match self {
            Self::Local0 => 16,
            Self::Local1 => 17,
            Self::Local2 => 18,
            Self::Local3 => 19,
            Self::Local4 => 20,
            Self::Local5 => 21,
            Self::Local6 => 22,
            Self::Local7 => 23,
        }
    }
}

/// HTTP transport configuration for remote SIEM audit sinks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpAuditConfig {
    /// Destination URL (e.g. "https://siem.example.com/ingest").
    pub url: String,
    /// Optional static Authorization header value (e.g. "Bearer <token>").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    /// Maximum events to batch in a single POST (default: 100).
    #[serde(default = "default_http_batch_size")]
    pub batch_size: usize,
    /// Seconds between automatic flushes (default: 5).
    #[serde(default = "default_http_flush_interval_s")]
    pub flush_interval_s: u64,
}

fn default_http_batch_size() -> usize {
    100
}

fn default_http_flush_interval_s() -> u64 {
    5
}

/// Syslog transport configuration for remote SIEM audit sinks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyslogAuditConfig {
    /// Hostname or IP address of the syslog receiver.
    pub host: String,
    /// UDP port number (traditional syslog default is 514).
    #[serde(default = "default_syslog_port")]
    pub port: u16,
    /// Syslog facility encoded in each message.
    #[serde(default)]
    pub facility: SyslogFacility,
    /// Optional app name prefix in syslog HEADER (defaults to "loongclaw").
    #[serde(default = "default_syslog_app_name")]
    pub app_name: String,
}

fn default_syslog_port() -> u16 {
    514
}

fn default_syslog_app_name() -> String {
    "loongclaw".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditConfig {
    #[serde(default)]
    pub mode: AuditMode,
    #[serde(default = "default_audit_path")]
    pub path: String,
    #[serde(default = "default_true")]
    pub retain_in_memory: bool,
    /// HTTP transport configuration. Required when mode is `http`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpAuditConfig>,
    /// Syslog transport configuration. Required when mode is `syslog`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syslog: Option<SyslogAuditConfig>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            mode: AuditMode::default(),
            path: default_audit_path(),
            retain_in_memory: default_true(),
            http: None,
            syslog: None,
        }
    }
}

impl AuditConfig {
    pub fn resolved_path(&self) -> PathBuf {
        let trimmed = self.path.trim();
        if trimmed.is_empty() {
            return expand_path(&default_audit_path());
        }
        expand_path(trimmed)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    InMemory,
    Jsonl,
    /// Fan out to both Jsonl and in-memory (default).
    #[default]
    Fanout,
    /// Emit audit events to a remote HTTP endpoint.
    Http,
    /// Emit audit events to a remote syslog receiver via UDP.
    Syslog,
}

impl AuditMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InMemory => "in_memory",
            Self::Jsonl => "jsonl",
            Self::Fanout => "fanout",
            Self::Http => "http",
            Self::Syslog => "syslog",
        }
    }
}

fn default_audit_path() -> String {
    default_loong_home()
        .join("audit")
        .join("events.jsonl")
        .display()
        .to_string()
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audit_config_defaults_to_fanout_under_loong_home() {
        let config = AuditConfig::default();

        assert_eq!(config.mode, AuditMode::Fanout);
        assert!(config.retain_in_memory);
        assert_eq!(
            PathBuf::from(&config.path),
            default_loong_home().join("audit").join("events.jsonl")
        );
    }

    #[test]
    fn audit_mode_ids_are_stable() {
        assert_eq!(AuditMode::InMemory.as_str(), "in_memory");
        assert_eq!(AuditMode::Jsonl.as_str(), "jsonl");
        assert_eq!(AuditMode::Fanout.as_str(), "fanout");
        assert_eq!(AuditMode::Http.as_str(), "http");
        assert_eq!(AuditMode::Syslog.as_str(), "syslog");
    }

    #[test]
    fn audit_config_empty_path_falls_back_to_default_location() {
        let config = AuditConfig {
            path: "   ".to_owned(),
            ..AuditConfig::default()
        };

        assert_eq!(
            config.resolved_path(),
            default_loong_home().join("audit").join("events.jsonl")
        );
    }
}
