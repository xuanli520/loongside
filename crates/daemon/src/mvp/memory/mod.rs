#[cfg(feature = "memory-sqlite")]
use std::path::PathBuf;

use kernel::{MemoryCoreOutcome, MemoryCoreRequest};
use serde_json::json;

#[cfg(feature = "memory-sqlite")]
mod sqlite;

#[cfg(feature = "memory-sqlite")]
pub use sqlite::ConversationTurn;

pub fn execute_memory_core(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    match request.operation.as_str() {
        "append_turn" => append_turn(request),
        "window" => load_window(request),
        "clear_session" => clear_session(request),
        _ => Ok(MemoryCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "kv-core",
                "operation": request.operation,
                "payload": request.payload,
            }),
        }),
    }
}

fn append_turn(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = request;
        return Err(
            "sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned(),
        );
    }

    #[cfg(feature = "memory-sqlite")]
    {
        sqlite::append_turn(request)
    }
}

fn load_window(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = request;
        return Err(
            "sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned(),
        );
    }

    #[cfg(feature = "memory-sqlite")]
    {
        sqlite::load_window(request)
    }
}

fn clear_session(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = request;
        return Err(
            "sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned(),
        );
    }

    #[cfg(feature = "memory-sqlite")]
    {
        sqlite::clear_session(request)
    }
}

#[cfg(feature = "memory-sqlite")]
pub fn append_turn_direct(session_id: &str, role: &str, content: &str) -> Result<(), String> {
    sqlite::append_turn_direct(session_id, role, content)
}

#[cfg(feature = "memory-sqlite")]
pub fn window_direct(session_id: &str, limit: usize) -> Result<Vec<ConversationTurn>, String> {
    sqlite::window_direct(session_id, limit)
}

#[cfg(feature = "memory-sqlite")]
pub fn ensure_memory_db_ready(path: Option<PathBuf>) -> Result<PathBuf, String> {
    sqlite::ensure_memory_db_ready(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_memory_operation_stays_compatible() {
        let outcome = execute_memory_core(MemoryCoreRequest {
            operation: "noop".to_owned(),
            payload: json!({"a":1}),
        })
        .expect("fallback operation should succeed");
        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["adapter"], "kv-core");
    }
}
