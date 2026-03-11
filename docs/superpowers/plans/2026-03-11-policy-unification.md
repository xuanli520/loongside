# Policy System Unification Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify all permission/security decision paths onto the token-based `PolicyExtensionChain` system, eliminating shadow security paths in tool adapters and the parallel `check_tool_call` mechanism.

**Architecture:** Extend `PolicyExtensionContext` with `request_parameters`, implement `ToolPolicyExtension` and `FilePolicyExtension` in the app crate (registered at bootstrap), deprecate `check_tool_call`, wire `FilesystemRead`/`FilesystemWrite` capabilities, remove `shell_allowlist`.

**Tech Stack:** Rust, serde_json, toml (existing dependency)

**Spec:** `docs/design-docs/rfc-policy-unification.md`

**Phasing:** This plan is split into two phases:
- **Phase A (blocking):** Minimum changes required to unblock the Shell AST Sandbox RFC. Tasks 1-9.
- **Phase B (non-blocking cleanup):** Terminology cleanup and config externalization. Tasks 10-11. Can be done after Shell AST RFC.

After completing Phase A, pause to run full verification and reflect before proceeding to Phase B.

---

## File Structure

### Kernel crate (`crates/kernel/src/`)

| File | Change | Phase |
|------|--------|-------|
| `policy_ext.rs` | Add `request_parameters` field to `PolicyExtensionContext` | A |
| `kernel.rs` | Extend `authorize_pack_operation` signature; remove `enforce_tool_policy`; update 10 call sites | A |
| `policy.rs` | Deprecate `check_tool_call`; `StaticPolicyEngine::check_tool_call` returns `Allow` unconditionally | A |

### App crate (`crates/app/src/`)

| File | Change | Phase |
|------|--------|-------|
| `tools/policy_ext.rs` | **NEW** - `ToolPolicyExtension` implementing `PolicyExtension` trait | A |
| `tools/file_policy_ext.rs` | **NEW** - `FilePolicyExtension` implementing `PolicyExtension` trait | A |
| `tools/shell.rs` | Remove allowlist check (lines 48-55) | A |
| `tools/runtime_config.rs` | Remove `shell_allowlist` field | A |
| `tools/mod.rs` | Add new modules; remove `shell_allowlist` from test configs | A |
| `context.rs` | Grant `FilesystemRead`/`FilesystemWrite`; register policy extensions | A |
| `conversation/turn_engine.rs` | Add `ApprovalRequired` to `KernelFailureClass`; refine `classify_kernel_error` | A |
| `conversation/turn_coordinator.rs` | Handle new `ApprovalRequired` variant | A |
| `tools/file.rs` | Remove `shell_allowlist` from test configs | A |
| `chat.rs` | Remove `LOONGCLAW_SHELL_ALLOWLIST` env export and `shell_allowlist` config wiring | A |
| `channel/mod.rs` | Same as `chat.rs` | A |
| `config/tools_memory.rs` | Add deprecation warning for `shell_allowlist` | A |
| `conversation/lane_arbiter.rs` | Rename `risk_score` to `routing_score`; remove security keywords | B |
| `config/conversation.rs` | Rename config fields with serde aliases | B |

### Contracts crate (`crates/contracts/src/`)

No changes. `Capability` enum already has `FilesystemRead`/`FilesystemWrite`. `PolicyError` already has `ToolCallDenied` and `ToolCallApprovalRequired`.

---

## Phase A: Core Security Unification (Tasks 1-9)

### Task 1: Extend PolicyExtensionContext with request_parameters

**Files:**
- Modify: `crates/kernel/src/policy_ext.rs`
- Modify: `crates/kernel/src/kernel.rs:864`

- [ ] **Step 1: Write failing test**

In `crates/kernel/src/tests.rs` (or inline in `policy_ext.rs`), add a test that constructs `PolicyExtensionContext` with `request_parameters: Some(&json!(...))` and asserts the value is accessible.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-kernel`
Expected: FAIL — `PolicyExtensionContext` has no field `request_parameters`

- [ ] **Step 3: Add field to PolicyExtensionContext**

In `crates/kernel/src/policy_ext.rs`:

```rust
pub struct PolicyExtensionContext<'a> {
    pub pack: &'a VerticalPackManifest,
    pub token: &'a CapabilityToken,
    pub now_epoch_s: u64,
    pub required_capabilities: &'a BTreeSet<Capability>,
    pub request_parameters: Option<&'a serde_json::Value>,
}
```

- [ ] **Step 4: Fix compilation — add `request_parameters: None` at kernel.rs:864**

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p loongclaw-kernel`
Expected: All pass including new test.

- [ ] **Step 6: Commit**

```
git commit -am "feat(kernel): add request_parameters to PolicyExtensionContext"
```

---

### Task 2: Extend authorize_pack_operation to accept request_parameters

**Files:**
- Modify: `crates/kernel/src/kernel.rs`

- [ ] **Step 1: Change `authorize_pack_operation` signature (kernel.rs:724)**

Add `request_parameters: Option<&serde_json::Value>` parameter. Flow it through to `authorize_or_audit_denial` (kernel.rs:841) and into `PolicyExtensionContext`.

- [ ] **Step 2: Update 8 non-tool call sites to pass `None`**

Lines: 274, 313, 358, 412, 468, 509, 646, 687.

- [ ] **Step 3: Update 2 tool call sites to construct and pass parameters**

At kernel.rs:549 (`execute_tool_core`):
```rust
let tool_policy_params = serde_json::json!({
    "tool_name": &request.tool_name,
    "payload": &request.payload,
});
let now = self.authorize_pack_operation(
    pack, token, required_capabilities, Some(&tool_policy_params)
)?;
```

At kernel.rs:598 (`execute_tool_extension`):
```rust
let tool_policy_params = serde_json::json!({
    "tool_name": &request.extension_action,
    "payload": &request.payload,
});
let now = self.authorize_pack_operation(
    pack, token, required_capabilities, Some(&tool_policy_params)
)?;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p loongclaw-kernel`
Expected: All pass.

- [ ] **Step 5: Commit**

```
git commit -am "refactor(kernel): flow request_parameters through authorize_pack_operation"
```

---

### Task 3: Deprecate check_tool_call and remove enforce_tool_policy

**Files:**
- Modify: `crates/kernel/src/policy.rs`
- Modify: `crates/kernel/src/kernel.rs`

- [ ] **Step 1: Write test — deprecated check_tool_call always returns Allow**

```rust
#[test]
fn deprecated_check_tool_call_always_allows() {
    let engine = StaticPolicyEngine::default();
    let request = PolicyRequest { /* shell.exec with rm */ };
    assert_eq!(engine.check_tool_call(&request), PolicyDecision::Allow);
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — current `check_tool_call` returns `Deny` for `rm`.

- [ ] **Step 3: Deprecate check_tool_call**

In `policy.rs`:
- Add `#[deprecated(note = "Use PolicyExtensionChain instead")]` to `check_tool_call` on `PolicyEngine` trait (line 65)
- Make `StaticPolicyEngine::check_tool_call` (line 190) return `PolicyDecision::Allow` unconditionally
- Keep `SHELL_HARD_DENY_COMMANDS`, `SHELL_APPROVAL_REQUIRED_COMMANDS`, `default_tool_policy`, `default_shell_policy` as dead code for now (will be removed when `check_tool_call` is removed from trait)

- [ ] **Step 4: Remove `enforce_tool_policy` from kernel.rs**

Remove the method (kernel.rs:786-838) and its two call sites:
- kernel.rs:559 (`execute_tool_core`)
- kernel.rs:608 (`execute_tool_extension`)

Policy enforcement now happens entirely through `authorize_pack_operation` -> `PolicyExtensionChain`.

- [ ] **Step 5: Run tests — allow `#[deprecated]` warnings**

Run: `cargo test -p loongclaw-kernel`
Expected: All pass. Some existing tests that assert `Deny`/`RequireApproval` from `check_tool_call` will now fail — update them to assert `Allow`.

- [ ] **Step 6: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All pass.

- [ ] **Step 7: Commit**

```
git commit -am "refactor(kernel): deprecate check_tool_call, remove enforce_tool_policy"
```

---

### Task 4: Implement ToolPolicyExtension (TDD)

**Files:**
- Create: `crates/app/src/tools/policy_ext.rs`
- Modify: `crates/app/src/tools/mod.rs`

- [ ] **Step 1: Add `pub mod policy_ext;` to tools/mod.rs**

- [ ] **Step 2: Create policy_ext.rs with test module first (6 parity tests)**

Write 6 tests mirroring `kernel/src/policy.rs` tests:
1. `denies_destructive_shell_commands` — `rm` -> `Err(ToolCallDenied)`
2. `requires_approval_for_high_risk_commands` — `curl` -> `Err(ToolCallApprovalRequired)`
3. `allows_safe_shell_commands` — `echo` -> `Ok(())`
4. `normalizes_underscore_shell_alias` — tool_name `shell_exec`, `curl` -> `Err(ToolCallApprovalRequired)`
5. `keeps_non_shell_tools_allowed` — tool_name `file.read` -> `Ok(())`
6. `allows_when_no_request_parameters` — `request_parameters: None` -> `Ok(())`

Each test constructs `PolicyExtensionContext` with appropriate `request_parameters: Some(&json!({"tool_name": "shell.exec", "payload": {"command": "rm"}}))` and calls `authorize_extension`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p loongclaw-app policy_ext`
Expected: FAIL — struct not implemented yet.

- [ ] **Step 4: Implement ToolPolicyExtension**

Struct with `hard_deny: BTreeSet<String>` and `approval_required: BTreeSet<String>`. Constructor `default_rules()` with same values as `kernel/src/policy.rs:14-40`. Implements `PolicyExtension` trait: extracts `tool_name` from `params["tool_name"]`, `command` from `params["payload"]["command"]`, checks against sets.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p loongclaw-app policy_ext`
Expected: All 6 pass.

- [ ] **Step 6: Commit**

```
git commit -am "feat(app): add ToolPolicyExtension with parity tests"
```

---

### Task 5: Implement FilePolicyExtension (TDD)

**Files:**
- Create: `crates/app/src/tools/file_policy_ext.rs`
- Modify: `crates/app/src/tools/mod.rs`

- [ ] **Step 1: Add `pub mod file_policy_ext;` to tools/mod.rs**

- [ ] **Step 2: Write failing tests first**

5 tests:
1. `denies_file_write_without_capability` — token lacks `FilesystemWrite` -> `Err(ExtensionDenied)`
2. `denies_file_read_without_capability` — token lacks `FilesystemRead` -> `Err(ExtensionDenied)`
3. `allows_file_read_with_capability` — token has `FilesystemRead` -> `Ok(())`
4. `denies_path_escape` — path `../../etc/passwd` with file_root -> `Err(ExtensionDenied)`
5. `allows_path_within_root` — path `src/main.rs` with file_root -> `Ok(())`

- [ ] **Step 3: Run tests to verify they fail**

- [ ] **Step 4: Implement FilePolicyExtension**

Struct with `file_root: Option<PathBuf>`. Implements `PolicyExtension`: checks tool_name is `file.read`/`file.write`/`claw.import`, verifies token capabilities, validates path scope using existing normalization logic.

- [ ] **Step 5: Run tests to verify they pass**

- [ ] **Step 6: Commit**

```
git commit -am "feat(app): add FilePolicyExtension with capability and path checks"
```

---

### Task 6: Wire capabilities and register extensions in bootstrap

**Files:**
- Modify: `crates/app/src/context.rs`

- [ ] **Step 1: Add `FilesystemRead`/`FilesystemWrite` to MVP pack capabilities (context.rs:62)**

```rust
granted_capabilities: BTreeSet::from([
    Capability::InvokeTool,
    Capability::MemoryRead,
    Capability::MemoryWrite,
    Capability::FilesystemRead,
    Capability::FilesystemWrite,
]),
```

- [ ] **Step 2: Register ToolPolicyExtension and FilePolicyExtension after tool adapter registration (~line 84)**

```rust
kernel.register_policy_extension(
    crate::tools::policy_ext::ToolPolicyExtension::default_rules()
);
kernel.register_policy_extension(
    crate::tools::file_policy_ext::FilePolicyExtension::new(
        Some(config_file_root)
    )
);
```

Note: `bootstrap_kernel_context` may need a `file_root` parameter. Check if `ToolRuntimeConfig` is accessible at this point or if the parameter needs to be threaded through.

- [ ] **Step 3: Run full test suite**

Run: `cargo test --workspace --all-features`

- [ ] **Step 4: Commit**

```
git commit -am "feat(app): wire capabilities and register policy extensions in bootstrap"
```

---

### Task 7: Remove shell_allowlist and shadow security paths

**Files:**
- Modify: `crates/app/src/tools/shell.rs`
- Modify: `crates/app/src/tools/runtime_config.rs`
- Modify: `crates/app/src/tools/mod.rs` (tests)
- Modify: `crates/app/src/tools/file.rs` (tests)
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/channel/mod.rs`
- Modify: `crates/app/src/config/tools_memory.rs`

- [ ] **Step 1: Remove allowlist check from shell.rs (lines 48-55)**

- [ ] **Step 2: Remove `shell_allowlist` field from `ToolRuntimeConfig` (runtime_config.rs:11)**

Remove field, update `from_env()`, remove `LOONGCLAW_SHELL_ALLOWLIST` env var parsing.

- [ ] **Step 3: Remove `LOONGCLAW_SHELL_ALLOWLIST` env export from chat.rs (lines 394-396, 405-407) and channel/mod.rs (lines 711-713, 722-724)**

- [ ] **Step 4: Add deprecation warning in config/tools_memory.rs**

Keep `shell_allowlist` in `ToolConfig` deserialization but emit warning if non-empty.

- [ ] **Step 5: Fix all test compilation — remove `shell_allowlist` from ToolRuntimeConfig constructions**

Affected test sites:
- `tools/runtime_config.rs`: lines 67, 72-81, 92, 99
- `tools/file.rs`: lines 282, 305, 330
- `tools/mod.rs`: lines 450, 534, 614, 685, 757, 834, 919

- [ ] **Step 6: Run full test suite**

Run: `cargo test --workspace --all-features`

- [ ] **Step 7: Commit**

```
git commit -am "refactor(app): remove shell_allowlist shadow path, add deprecation warning"
```

---

### Task 8: Refine classify_kernel_error for approval vs deny

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`
- Modify: `crates/app/src/conversation/turn_coordinator.rs`

- [ ] **Step 1: Add `ApprovalRequired` variant to `KernelFailureClass` (turn_engine.rs:170)**

```rust
pub(crate) enum KernelFailureClass {
    PolicyDenied,
    ApprovalRequired,
    RetryableExecution,
    NonRetryable,
}
```

- [ ] **Step 2: Update `classify_kernel_error` (turn_engine.rs:176)**

```rust
KernelError::Policy(PolicyError::ToolCallApprovalRequired { .. }) => {
    KernelFailureClass::ApprovalRequired
}
KernelError::Policy(_)
| KernelError::PackCapabilityBoundary { .. }
| KernelError::ConnectorNotAllowed { .. } => KernelFailureClass::PolicyDenied,
```

Add `use loongclaw_contracts::PolicyError;` to imports.

- [ ] **Step 3: Handle new variant at turn_engine.rs:283**

```rust
KernelFailureClass::ApprovalRequired => {
    TurnResult::needs_approval("kernel_approval_required", reason)
}
```

- [ ] **Step 4: Handle new variant at turn_coordinator.rs:1792**

```rust
KernelFailureClass::ApprovalRequired => PlanNodeErrorKind::PolicyDenied,
```

(In plan context, approval = denial since no interactive approval path.)

- [ ] **Step 5: Run tests**

Run: `cargo test -p loongclaw-app turn_engine`

- [ ] **Step 6: Commit**

```
git commit -am "feat(app): distinguish approval-required from policy-denied in turn engine"
```

---

### Task 9: Phase A verification and reflection

- [ ] **Step 1: Run full verification**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
```

If available: `task verify`

- [ ] **Step 2: Reflect**

Review what was done:
- Is `check_tool_call` fully bypassed? (Should return `Allow` for everything)
- Are all tool calls going through `PolicyExtensionChain`?
- Are `FilesystemRead`/`FilesystemWrite` actually enforced?
- Are all denials audited through kernel?
- Is `shell_allowlist` fully removed from runtime path?

Document any issues found. Adjust Phase B plan if needed.

- [ ] **Step 3: Commit any fixups**

---

## Phase B: Non-Blocking Cleanup (Tasks 10-11)

Execute after Phase A reflection. These are not blocking for Shell AST Sandbox RFC.

### Task 10: Rename LaneArbiterPolicy risk terminology

**Files:**
- Modify: `crates/app/src/conversation/lane_arbiter.rs`
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Modify: `crates/app/src/config/conversation.rs`
- Modify: `crates/app/src/config/mod.rs` (tests)

- [ ] **Step 1: Rename in lane_arbiter.rs**

- `fn risk_score` -> `fn routing_score`
- `risk_score` field in `LaneDecision` -> `routing_score`
- `safe_lane_risk_threshold` -> `safe_lane_routing_threshold`
- `"risk_score_exceeded"` -> `"routing_score_exceeded"`
- `high_risk_keywords` -> `high_complexity_keywords`

- [ ] **Step 2: Remove security keywords**

Remove: `"rm -rf"`, `"credential"`, `"token"`, `"secret"`
Keep: `"drop table"`, `"delete"`, `"prod"`, `"production"`, `"deploy"`, `"payment"`, `"wallet"`

- [ ] **Step 3: Update config/conversation.rs with serde aliases**

```rust
#[serde(default = "default_safe_lane_routing_threshold", alias = "safe_lane_risk_threshold")]
pub safe_lane_routing_threshold: u32,

#[serde(default = "default_high_complexity_keywords", alias = "high_risk_keywords")]
pub high_complexity_keywords: Vec<String>,
```

- [ ] **Step 4: Update turn_coordinator.rs references**

Key locations: lines 317-328, 335, 373, 825, 888-901.

- [ ] **Step 5: Update config/mod.rs test assertions (lines 849, 854)**

- [ ] **Step 6: Run tests**

Run: `cargo test -p loongclaw-app lane_arbiter`

- [ ] **Step 7: Commit**

```
git commit -am "refactor(app): rename lane arbiter risk terminology to routing/complexity"
```

---

### ~~Task 11: Externalize policy rules to TOML profile~~ (DEFERRED — YAGNI)

Deferred per Core Belief #10. Current deny/approval lists are small and stable. Externalization adds schema definition, loader logic, and config path resolution for minimal current benefit. Revisit when the Shell AST RFC introduces dynamic per-session rules or when the rule set grows beyond what hardcoded defaults can serve.

- [ ] **Step 4: Commit**

```
git commit -am "feat(app): externalize tool policy rules to TOML profile"
```

---

## Final Verification

After all tasks:

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo test --workspace --all-features`
- [ ] If available: `task verify`
