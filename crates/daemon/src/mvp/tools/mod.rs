use std::collections::BTreeMap;

use kernel::{ToolCoreOutcome, ToolCoreRequest};
use serde_json::{json, Value};

mod file;
mod shell;

pub fn execute_tool_core(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    match request.tool_name.as_str() {
        "shell.exec" | "shell_exec" | "shell" => shell::execute_shell_tool(request),
        "file.read" | "file_read" => file::execute_file_read_tool(request),
        "file.write" | "file_write" => file::execute_file_write_tool(request),
        _ => Ok(ToolCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "payload": request.payload,
            }),
        }),
    }
}

#[allow(dead_code)]
fn _shape_examples() -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        (
            "shell.exec",
            json!({
                "command": "echo",
                "args": ["hello"]
            }),
        ),
        (
            "file.read",
            json!({
                "path": "README.md",
                "max_bytes": 4096
            }),
        ),
        (
            "file.write",
            json!({
                "path": "notes.txt",
                "content": "hello",
                "create_dirs": true
            }),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tool_keeps_backward_compatible_payload_shape() {
        let outcome = execute_tool_core(ToolCoreRequest {
            tool_name: "unknown".to_owned(),
            payload: json!({"hello":"world"}),
        })
        .expect("unknown tool should fallback to echo behavior");
        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["adapter"], "core-tools");
    }
}
