use std::path::PathBuf;
use std::sync::OnceLock;

/// Typed runtime configuration for the memory (SQLite) subsystem.
///
/// Mirrors [`crate::tools::runtime_config::ToolRuntimeConfig`] — a
/// process-wide singleton populated once at startup so that per-call
/// `std::env::var` lookups are avoided on the hot path.
#[derive(Debug, Clone, Default)]
pub struct MemoryRuntimeConfig {
    pub sqlite_path: Option<PathBuf>,
    pub sliding_window: Option<usize>,
}

impl MemoryRuntimeConfig {
    /// Build a config by reading the legacy environment variable.
    ///
    /// Keeps full backward compatibility for callers that still rely on
    /// `LOONGCLAW_SQLITE_PATH`.
    pub fn from_env() -> Self {
        let sqlite_path = std::env::var("LOONGCLAW_SQLITE_PATH")
            .ok()
            .map(PathBuf::from);
        let sliding_window_raw = std::env::var("LOONGCLAW_SLIDING_WINDOW").ok();
        let sliding_window = parse_sliding_window(sliding_window_raw.as_deref());
        Self {
            sqlite_path,
            sliding_window,
        }
    }
}

fn parse_sliding_window(raw: Option<&str>) -> Option<usize> {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

static MEMORY_RUNTIME_CONFIG: OnceLock<MemoryRuntimeConfig> = OnceLock::new();

/// Initialise the process-wide memory runtime config.
///
/// Returns `Ok(())` on the first call.  Subsequent calls return
/// `Err` because the `OnceLock` rejects duplicate initialisation.
pub fn init_memory_runtime_config(config: MemoryRuntimeConfig) -> Result<(), String> {
    MEMORY_RUNTIME_CONFIG.set(config).map_err(|_err| {
        "memory runtime config already initialised (duplicate init_memory_runtime_config call)"
            .to_owned()
    })
}

/// Return the process-wide memory runtime config.
///
/// If `init_memory_runtime_config` was never called the config is lazily
/// populated from environment variables (backward-compat path).
pub fn get_memory_runtime_config() -> &'static MemoryRuntimeConfig {
    MEMORY_RUNTIME_CONFIG.get_or_init(MemoryRuntimeConfig::from_env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sliding_window_accepts_positive_integer() {
        assert_eq!(parse_sliding_window(Some("24")), Some(24));
    }

    #[test]
    fn parse_sliding_window_rejects_zero_negative_and_invalid_values() {
        assert_eq!(parse_sliding_window(Some("0")), None);
        assert_eq!(parse_sliding_window(Some("-1")), None);
        assert_eq!(parse_sliding_window(Some("invalid")), None);
    }

    #[test]
    fn parse_sliding_window_returns_none_when_absent() {
        assert_eq!(parse_sliding_window(None), None);
    }

    #[test]
    fn memory_runtime_config_default_has_no_path() {
        let config = MemoryRuntimeConfig::default();
        assert!(config.sqlite_path.is_none());
        assert!(config.sliding_window.is_none());
    }

    #[test]
    fn explicit_config_overrides_default() {
        let config = MemoryRuntimeConfig {
            sqlite_path: Some(PathBuf::from("/tmp/test-memory.sqlite3")),
            sliding_window: Some(24),
        };
        assert_eq!(
            config.sqlite_path,
            Some(PathBuf::from("/tmp/test-memory.sqlite3"))
        );
        assert_eq!(config.sliding_window, Some(24));
    }
}
