# Tool-Call Testing Framework Implementation Plan (v2)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add integration tests covering the full TurnEngine tool-call path (provider response → gate → kernel dispatch → real tool execution → audit) with deterministic, non-flaky assertions.

**Architecture:** Fix the `ToolRuntimeConfig` singleton problem first by making it injectable through `MvpToolAdapter` and `execute_tool_core`. Then build `FakeProviderBuilder` + `TurnTestHarness` that composes a real kernel with injected config, real `InMemoryAuditSink`, and real tool executors pointed at a unique temp dir. Tests assert exact success outcomes — no permissive "accept any error" branches. Scenarios that duplicate existing unit tests are excluded.

**Tech Stack:** Rust, tokio, serde_json, loongclaw-kernel (`InMemoryAuditSink`, `FixedClock`, `StaticPolicyEngine`), loongclaw-app (`TurnEngine`, `MvpToolAdapter`, `ToolRuntimeConfig`).

**Execution Notes:** Use `@superpowers:test-driven-development` and `@superpowers:verification-before-completion` while implementing.

**Key design decisions from code review (v1 issues fixed):**
1. `ToolRuntimeConfig` is now injectable via parameter — no OnceLock race in tests
2. Each harness gets a unique temp dir via `std::env::temp_dir().join(uuid)` — no PID collision
3. Scenarios 1, 3, 4, 5 removed — already covered by unit tests in `tests.rs`
4. Real-execution tests assert exact `FinalText` with content — no permissive `ToolError` branches
5. Audit test asserts `PlaneInvoked { plane: Tool }` — not just `TokenIssued`
6. Persistence scenarios deferred — `execute_turn` doesn't call `persist_tool_*` yet

---

### Task 1: Make ToolRuntimeConfig Injectable Through Tool Executors

The `OnceLock`-based global `ToolRuntimeConfig` causes test interference when multiple tests need different configs. Fix: add a `config` parameter to the tool execution chain. Keep the global for production code (backward compat).

**Files:**
- Modify: `crates/app/src/tools/shell.rs`
- Modify: `crates/app/src/tools/file.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/kernel_adapter.rs`
- Modify: `crates/app/src/tools/runtime_config.rs`

**Step 1: Write the failing test**

Add to `crates/app/src/tools/runtime_config.rs` tests section:

```rust
#[test]
fn injected_config_overrides_global() {
    let config = ToolRuntimeConfig {
        shell_allowlist: BTreeSet::from(["echo".to_owned()]),
        file_root: Some(PathBuf::from("/tmp/injected-root")),
    };
    // execute_tool_core_with_config should use the injected config,
    // not the global OnceLock.
    let result = crate::tools::execute_tool_core_with_config(
        loongclaw_contracts::ToolCoreRequest {
            tool_name: "shell.exec".to_owned(),
            payload: serde_json::json!({"command": "echo", "args": ["injected"]}),
        },
        &config,
    );
    // echo should succeed because we injected it into the allowlist
    let outcome = result.expect("echo should be allowed with injected config");
    assert_eq!(outcome.status, "ok");
    assert!(outcome.payload["stdout"].as_str().unwrap().contains("injected"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app --all-features injected_config_overrides_global -- --nocapture`
Expected: FAIL (`execute_tool_core_with_config` not found).

**Step 3: Write minimal implementation**

In `crates/app/src/tools/shell.rs`, add a config-accepting variant:

```rust
pub(super) fn execute_shell_tool_with_config(
    request: ToolCoreRequest,
    config: &super::runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "tool-shell"))]
    {
        let _ = (request, config);
        return Err("shell tool is disabled in this build (enable feature `tool-shell`)".to_owned());
    }

    #[cfg(feature = "tool-shell")]
    {
        let payload = request
            .payload
            .as_object()
            .ok_or_else(|| "shell.exec payload must be an object".to_owned())?;
        let command = payload
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "shell.exec requires payload.command".to_owned())?;
        let args = payload
            .get("args")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cwd = payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let normalized_command = command.to_ascii_lowercase();
        if !config.shell_allowlist.contains(&normalized_command) {
            return Err(format!(
                "shell command `{command}` is not allowed (allowlist={})",
                config.shell_allowlist.iter().cloned().collect::<Vec<_>>().join(",")
            ));
        }

        let output = Command::new(command)
            .args(&args)
            .current_dir(&cwd)
            .output()
            .map_err(|error| format!("shell command spawn failed: {error}"))?;

        Ok(ToolCoreOutcome {
            status: if output.status.success() {
                "ok".to_owned()
            } else {
                "failed".to_owned()
            },
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "command": command,
                "args": args,
                "cwd": cwd.display().to_string(),
                "exit_code": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout).trim().to_owned(),
                "stderr": String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        })
    }
}
```

Then refactor the existing `execute_shell_tool` to delegate:

```rust
pub(super) fn execute_shell_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    execute_shell_tool_with_config(request, super::runtime_config::get_tool_runtime_config())
}
```

In `crates/app/src/tools/file.rs`, add config-accepting variants:

```rust
pub(super) fn execute_file_read_tool_with_config(
    request: ToolCoreRequest,
    config: &super::runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    // Same body as execute_file_read_tool but calls resolve_safe_file_path_with_config(target, config)
}

pub(super) fn execute_file_write_tool_with_config(
    request: ToolCoreRequest,
    config: &super::runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    // Same body as execute_file_write_tool but calls resolve_safe_file_path_with_config(target, config)
}

#[cfg(feature = "tool-file")]
fn resolve_safe_file_path_with_config(
    raw: &str,
    config: &super::runtime_config::ToolRuntimeConfig,
) -> Result<PathBuf, String> {
    let root = config
        .file_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = canonicalize_or_fallback(root)?;
    // ... rest identical to resolve_safe_file_path
}
```

Then refactor existing functions to delegate:

```rust
pub(super) fn execute_file_read_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    execute_file_read_tool_with_config(request, super::runtime_config::get_tool_runtime_config())
}

pub(super) fn execute_file_write_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    execute_file_write_tool_with_config(request, super::runtime_config::get_tool_runtime_config())
}
```

In `crates/app/src/tools/mod.rs`, add the config-accepting dispatcher:

```rust
pub fn execute_tool_core_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    match request.tool_name.as_str() {
        "shell.exec" | "shell_exec" | "shell" => shell::execute_shell_tool_with_config(request, config),
        "file.read" | "file_read" => file::execute_file_read_tool_with_config(request, config),
        "file.write" | "file_write" => file::execute_file_write_tool_with_config(request, config),
        _ => Err(format!("tool_not_found: unknown tool `{}`", request.tool_name)),
    }
}
```

In `crates/app/src/tools/kernel_adapter.rs`, make `MvpToolAdapter` configurable:

```rust
use super::runtime_config::ToolRuntimeConfig;

pub struct MvpToolAdapter {
    config: Option<ToolRuntimeConfig>,
}

impl MvpToolAdapter {
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: ToolRuntimeConfig) -> Self {
        Self { config: Some(config) }
    }
}

#[async_trait]
impl CoreToolAdapter for MvpToolAdapter {
    fn name(&self) -> &str {
        "mvp-tools"
    }

    async fn execute_core_tool(
        &self,
        request: ToolCoreRequest,
    ) -> Result<ToolCoreOutcome, ToolPlaneError> {
        match &self.config {
            Some(config) => super::execute_tool_core_with_config(request, config),
            None => super::execute_tool_core(request),
        }
        .map_err(ToolPlaneError::Execution)
    }
}
```

**Important:** Update all existing call sites that construct `MvpToolAdapter` as a unit struct:
- `crates/app/src/context.rs:82` — change `MvpToolAdapter` to `MvpToolAdapter::new()`
- Any test in `crates/app/src/tools/mod.rs` that uses `MvpToolAdapter` — change to `MvpToolAdapter::new()`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p loongclaw-app --all-features injected_config_overrides_global -- --nocapture
cargo test -p loongclaw-app --all-features tool -- --nocapture
cargo test --workspace --all-features
```

Expected: PASS (all existing tests still pass, new test passes).

**Step 5: Commit**

```bash
git add crates/app/src/tools/shell.rs crates/app/src/tools/file.rs crates/app/src/tools/mod.rs crates/app/src/tools/kernel_adapter.rs crates/app/src/tools/runtime_config.rs crates/app/src/context.rs
git commit -m "refactor(app): make ToolRuntimeConfig injectable through tool executors"
```

---

### Task 2: Add FakeProviderBuilder and TurnTestHarness

**Files:**
- Create: `crates/app/src/conversation/integration_tests.rs`
- Modify: `crates/app/src/conversation/mod.rs`

**Step 1: Write the failing test**

Create `crates/app/src/conversation/integration_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn fake_provider_builder_text_only() {
        let turn = FakeProviderBuilder::new().with_text("hello world").build();
        assert_eq!(turn.assistant_text, "hello world");
        assert!(turn.tool_intents.is_empty());
    }

    #[test]
    fn fake_provider_builder_with_tool_call() {
        let turn = FakeProviderBuilder::new()
            .with_text("checking file")
            .with_tool_call("file.read", json!({"path": "test.txt"}))
            .build();
        assert_eq!(turn.assistant_text, "checking file");
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(turn.tool_intents[0].tool_name, "file.read");
        assert_eq!(turn.tool_intents[0].args_json, json!({"path": "test.txt"}));
        assert!(!turn.tool_intents[0].tool_call_id.is_empty());
    }

    #[test]
    fn fake_provider_builder_unique_tool_call_ids() {
        let turn = FakeProviderBuilder::new()
            .with_tool_call("file.read", json!({"path": "a.txt"}))
            .with_tool_call("file.read", json!({"path": "b.txt"}))
            .build();
        assert_eq!(turn.tool_intents.len(), 2);
        assert_ne!(
            turn.tool_intents[0].tool_call_id,
            turn.tool_intents[1].tool_call_id
        );
    }

    #[test]
    fn harness_builds_with_invoke_tool_capability() {
        let harness = TurnTestHarness::new();
        assert!(harness
            .kernel_ctx
            .token
            .allowed_capabilities
            .contains(&Capability::InvokeTool));
        // Temp dir should exist and be unique
        assert!(harness.temp_dir.exists());
    }

    #[test]
    fn harness_temp_dirs_are_unique() {
        let h1 = TurnTestHarness::new();
        let h2 = TurnTestHarness::new();
        assert_ne!(h1.temp_dir, h2.temp_dir);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app fake_provider_builder -- --nocapture`
Expected: FAIL (module/type not found).

**Step 3: Write minimal implementation**

In `crates/app/src/conversation/integration_tests.rs`, add above the `#[cfg(test)]`:

```rust
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use loongclaw_contracts::{Capability, ExecutionRoute, HarnessKind};
use loongclaw_kernel::{
    FixedClock, InMemoryAuditSink, LoongClawKernel, StaticPolicyEngine, VerticalPackManifest,
};

use super::turn_engine::{ProviderTurn, ToolIntent, TurnEngine, TurnResult};
use crate::context::KernelContext;
use crate::tools::runtime_config::ToolRuntimeConfig;
use crate::tools::MvpToolAdapter;

/// Monotonic counter for unique harness IDs (avoids temp dir collisions).
static HARNESS_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ergonomic builder for constructing fake `ProviderTurn` responses in tests.
pub(crate) struct FakeProviderBuilder {
    text: String,
    tool_calls: Vec<(String, serde_json::Value)>,
}

impl FakeProviderBuilder {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            tool_calls: Vec::new(),
        }
    }

    pub fn with_text(mut self, text: &str) -> Self {
        self.text = text.to_owned();
        self
    }

    pub fn with_tool_call(mut self, tool_name: &str, args: serde_json::Value) -> Self {
        self.tool_calls.push((tool_name.to_owned(), args));
        self
    }

    pub fn build(self) -> ProviderTurn {
        let tool_intents = self
            .tool_calls
            .into_iter()
            .enumerate()
            .map(|(i, (name, args))| ToolIntent {
                tool_name: name,
                args_json: args,
                source: "fake_provider".to_owned(),
                session_id: "test-session".to_owned(),
                turn_id: "test-turn".to_owned(),
                tool_call_id: format!("call-{i}"),
            })
            .collect();

        ProviderTurn {
            assistant_text: self.text,
            tool_intents,
            raw_meta: serde_json::Value::Null,
        }
    }
}

/// Integration test harness composing real kernel + real tools + fake provider.
///
/// Each harness gets:
/// - A unique temp dir (no collision between parallel tests)
/// - An `MvpToolAdapter` with injected `ToolRuntimeConfig` (no OnceLock race)
/// - A real `InMemoryAuditSink` for audit assertions
/// - `max_tool_steps = 1`
pub(crate) struct TurnTestHarness {
    pub engine: TurnEngine,
    pub kernel_ctx: KernelContext,
    pub audit: Arc<InMemoryAuditSink>,
    pub temp_dir: PathBuf,
}

impl TurnTestHarness {
    pub fn new() -> Self {
        Self::with_capabilities(BTreeSet::from([Capability::InvokeTool]))
    }

    pub fn with_capabilities(capabilities: BTreeSet<Capability>) -> Self {
        let id = HARNESS_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "loongclaw-integ-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        // Inject config so tests don't race on the global OnceLock
        let tool_config = ToolRuntimeConfig {
            shell_allowlist: BTreeSet::from([
                "echo".to_owned(),
                "cat".to_owned(),
                "ls".to_owned(),
            ]),
            file_root: Some(temp_dir.clone()),
        };

        let audit = Arc::new(InMemoryAuditSink::default());
        let clock = Arc::new(FixedClock::new(1_700_000_000));
        let mut kernel =
            LoongClawKernel::with_runtime(StaticPolicyEngine::default(), clock, audit.clone());

        let pack = VerticalPackManifest {
            pack_id: "test-pack".to_owned(),
            domain: "testing".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: None,
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: capabilities,
            metadata: BTreeMap::new(),
        };
        kernel.register_pack(pack).expect("register pack");
        kernel.register_core_tool_adapter(MvpToolAdapter::with_config(tool_config));
        kernel
            .set_default_core_tool_adapter("mvp-tools")
            .expect("set default adapter");

        let token = kernel
            .issue_token("test-pack", "test-agent", 3600)
            .expect("issue token");

        let ctx = KernelContext {
            kernel: Arc::new(kernel),
            token,
        };

        Self {
            engine: TurnEngine::new(1),
            kernel_ctx: ctx,
            audit,
            temp_dir,
        }
    }

    /// Execute a provider turn through the full TurnEngine path.
    pub async fn execute(&self, turn: &ProviderTurn) -> TurnResult {
        self.engine
            .execute_turn(turn, Some(&self.kernel_ctx))
            .await
    }
}

impl Drop for TurnTestHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}
```

In `crates/app/src/conversation/mod.rs`, add the module declaration near other mod declarations:

```rust
#[cfg(test)]
mod integration_tests;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p loongclaw-app --all-features fake_provider_builder -- --nocapture && cargo test -p loongclaw-app --all-features harness_ -- --nocapture`
Expected: PASS (all 5 tests).

**Step 5: Commit**

```bash
git add crates/app/src/conversation/integration_tests.rs crates/app/src/conversation/mod.rs
git commit -m "feat(app): add FakeProviderBuilder and TurnTestHarness with injected config"
```

---

### Task 3: Add Real-Execution Integration Tests (file.read, shell.exec, audit)

These are the scenarios that existing unit tests do NOT cover: full-stack execution through kernel → MvpToolAdapter → real tool executors with injected config and deterministic assertions.

**Files:**
- Modify: `crates/app/src/conversation/integration_tests.rs`

**Step 1: Write the tests**

Add to the `tests` module in `integration_tests.rs`:

```rust
// --- Real execution tests ---
// These test the full path: TurnEngine → kernel dispatch → MvpToolAdapter →
// execute_tool_core_with_config → real file/shell executor.
// Unlike unit tests in tests.rs, these use MvpToolAdapter (not EchoToolAdapter)
// with injected ToolRuntimeConfig, so the real executors run against a temp dir.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_file_read_returns_real_content() {
    let harness = TurnTestHarness::new();

    // Seed a file inside the harness temp dir (which is also file_root)
    let test_file = harness.temp_dir.join("greeting.txt");
    std::fs::write(&test_file, "hello from integration test").expect("seed file");

    // Use relative path — file_root is set to temp_dir, so "greeting.txt" resolves inside it
    let turn = FakeProviderBuilder::new()
        .with_tool_call("file.read", json!({"path": "greeting.txt"}))
        .build();
    let result = harness.execute(&turn).await;
    match result {
        TurnResult::FinalText(text) => {
            assert!(
                text.contains("hello from integration test"),
                "expected file content in output, got: {text}"
            );
        }
        other => panic!("expected FinalText with file content, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_file_write_then_read_round_trip() {
    let harness = TurnTestHarness::new();

    // Write a file via tool
    let write_turn = FakeProviderBuilder::new()
        .with_tool_call(
            "file.write",
            json!({"path": "round-trip.txt", "content": "written by tool"}),
        )
        .build();
    let write_result = harness.execute(&write_turn).await;
    match &write_result {
        TurnResult::FinalText(text) => {
            assert!(text.contains("[ok]"), "write should succeed, got: {text}");
        }
        other => panic!("expected FinalText for write, got {:?}", other),
    }

    // Read it back
    let read_turn = FakeProviderBuilder::new()
        .with_tool_call("file.read", json!({"path": "round-trip.txt"}))
        .build();
    let read_result = harness.execute(&read_turn).await;
    match read_result {
        TurnResult::FinalText(text) => {
            assert!(
                text.contains("written by tool"),
                "read should return written content, got: {text}"
            );
        }
        other => panic!("expected FinalText for read, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_shell_exec_echo() {
    let harness = TurnTestHarness::new();
    let turn = FakeProviderBuilder::new()
        .with_tool_call("shell.exec", json!({"command": "echo", "args": ["hello"]}))
        .build();
    let result = harness.execute(&turn).await;
    match result {
        TurnResult::FinalText(text) => {
            assert!(
                text.contains("hello"),
                "expected 'hello' in shell output, got: {text}"
            );
            assert!(
                text.contains("[ok]"),
                "expected ok status, got: {text}"
            );
        }
        other => panic!("expected FinalText with echo output, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_shell_exec_blocked_command() {
    let harness = TurnTestHarness::new();
    // "rm" is not in the injected allowlist (echo, cat, ls)
    let turn = FakeProviderBuilder::new()
        .with_tool_call("shell.exec", json!({"command": "rm", "args": ["-rf", "/"]}))
        .build();
    let result = harness.execute(&turn).await;
    // Should go through kernel but executor should reject via allowlist
    match result {
        TurnResult::FinalText(text) => {
            assert!(
                text.contains("not allowed"),
                "expected allowlist rejection in output, got: {text}"
            );
        }
        TurnResult::ToolError(err) => {
            assert!(
                err.contains("not allowed"),
                "expected allowlist rejection, got: {err}"
            );
        }
        other => panic!("expected rejection for blocked command, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_file_read_sandbox_rejects_path_escape() {
    let harness = TurnTestHarness::new();
    // Try to read a file outside the sandbox via path traversal
    let turn = FakeProviderBuilder::new()
        .with_tool_call("file.read", json!({"path": "../../../etc/passwd"}))
        .build();
    let result = harness.execute(&turn).await;
    match result {
        TurnResult::FinalText(text) => {
            assert!(
                text.contains("escapes"),
                "expected sandbox rejection, got: {text}"
            );
        }
        TurnResult::ToolError(err) => {
            assert!(
                err.contains("escapes"),
                "expected sandbox rejection, got: {err}"
            );
        }
        other => panic!("expected sandbox rejection, got {:?}", other),
    }
}

// --- Capability denial test (proves kernel policy gate works end-to-end) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_missing_capability_denies_tool() {
    // Build harness WITHOUT InvokeTool capability
    let harness = TurnTestHarness::with_capabilities(BTreeSet::from([Capability::MemoryRead]));
    let turn = FakeProviderBuilder::new()
        .with_tool_call("file.read", json!({"path": "test.txt"}))
        .build();
    let result = harness.execute(&turn).await;
    match result {
        TurnResult::ToolDenied(reason) => {
            assert!(
                reason.contains("denied") || reason.contains("apability"),
                "expected policy denial, got: {reason}"
            );
        }
        other => panic!("expected ToolDenied, got {:?}", other),
    }
}

// --- Audit event tests ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_audit_captures_tool_plane_invocation() {
    let harness = TurnTestHarness::new();

    let turn = FakeProviderBuilder::new()
        .with_tool_call("shell.exec", json!({"command": "echo", "args": ["audit-test"]}))
        .build();
    let _result = harness.execute(&turn).await;

    let events = harness.audit.snapshot();

    // Must have PlaneInvoked with plane: Tool (from tool execution, not just setup)
    let has_tool_plane = events.iter().any(|e| {
        matches!(
            &e.kind,
            loongclaw_contracts::AuditEventKind::PlaneInvoked {
                plane: loongclaw_contracts::ExecutionPlane::Tool,
                ..
            }
        )
    });
    assert!(
        has_tool_plane,
        "audit should contain PlaneInvoked{{Tool}} event, got: {:?}",
        events.iter().map(|e| format!("{:?}", e.kind)).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integ_malformed_tool_args_returns_error() {
    let harness = TurnTestHarness::new();
    // Send a non-object payload to file.read (expects an object with "path")
    let turn = FakeProviderBuilder::new()
        .with_tool_call("file.read", json!("not an object"))
        .build();
    let result = harness.execute(&turn).await;
    match result {
        TurnResult::FinalText(text) => {
            // MvpToolAdapter wraps errors — check the output contains error info
            assert!(
                text.contains("payload must be an object") || text.contains("error"),
                "expected payload error, got: {text}"
            );
        }
        TurnResult::ToolError(err) => {
            assert!(
                err.contains("payload must be an object"),
                "expected payload error, got: {err}"
            );
        }
        other => panic!("expected error for malformed args, got {:?}", other),
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test -p loongclaw-app --all-features integ_ -- --nocapture`
Expected: PASS (all 8 tests).

**Step 3: Commit**

```bash
git add crates/app/src/conversation/integration_tests.rs
git commit -m "test(app): add real-execution integration tests with injected config"
```

---

### Task 4: Final Verification Gate

**Step 1: Run full verification**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: all commands succeed with 0 errors, 0 warnings.

**Step 2: Fix any issues found**

If clippy or test failures arise, fix them and re-run the verification gate.

**Step 3: Commit any fixes**

```bash
git add -u
git commit -m "fix(app): address clippy/fmt issues in integration tests"
```

---

## Final Verification Gate

Run all required verification before claiming completion:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: all commands succeed.

---

## Appendix: Issues Fixed From v1 Review

| v1 Issue | Fix in v2 |
|----------|-----------|
| OnceLock process-wide singleton race | `MvpToolAdapter::with_config()` injects config per-harness |
| Permissive assertions that can never fail | Exact `FinalText` assertions with content checks; no `ToolError` fallback branches for happy-path tests |
| Scenarios 1,3,4,5 duplicate unit tests | Removed; unit tests in `tests.rs` already cover these |
| PID-based temp dir shared across tests | `AtomicU64` counter gives each harness a unique dir |
| Scenario 8 only asserts TokenIssued | Now asserts `PlaneInvoked { plane: Tool }` |
| Scenario 9 persistence claim is false | Removed; persistence not wired into `execute_turn` yet |
| Missing coverage: malformed args | Added `integ_malformed_tool_args_returns_error` |
| Missing coverage: sandbox escape | Added `integ_file_read_sandbox_rejects_path_escape` |
| Missing coverage: blocked commands | Added `integ_shell_exec_blocked_command` |
| Missing coverage: write+read round-trip | Added `integ_file_write_then_read_round_trip` |
