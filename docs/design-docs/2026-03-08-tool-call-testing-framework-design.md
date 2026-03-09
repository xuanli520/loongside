# Tool-Call Testing Framework Design

Date: 2026-03-08
Status: Approved

## Context

The tool-call foundation (TurnEngine, ProviderTurn, policy-gated execution) has 20 unit tests but no integration/regression tests that prove the full path works end-to-end: provider response → parse → TurnEngine gate → real tool execution → persistence → audit.

The spec-runner framework tests kernel-level operations. It doesn't cover the app-layer TurnEngine orchestration. We need a separate integration harness at the app layer boundary.

## Decision

Add a `TurnTestHarness` in the app crate that composes real components (real kernel, real tools, fake provider responses) to test the full TurnEngine flow.

## Components

### FakeProviderBuilder

Ergonomic builder for constructing fake `ProviderTurn` responses:

```rust
FakeProviderBuilder::new()
    .with_text("checking file")
    .with_tool_call("file.read", json!({"path": "test.txt"}))
    .build()  // → ProviderTurn
```

### TurnTestHarness

Wires real kernel + real tools + fake provider into an executable test:

- Real `StaticPolicyEngine` with configurable capabilities
- Real `InMemoryAuditSink` for audit capture
- Real tool executors: `file.read`/`file.write` in temp dir, `shell.exec` with safe commands
- `ToolRuntimeConfig` pointed at temp dir with safe shell allowlist
- Real `TurnEngine` with `max_tool_steps=1`
- Persistence capture: deferred (TurnEngine does not yet persist tool lifecycle records)

```rust
TurnTestHarness::new()
    .with_capability(InvokeTool)
    .with_provider_turn(fake_turn)
    .execute()  // → TurnResult + audit events + persisted records
```

### Cleanup

Harness uses `tempfile` or manual temp dir with cleanup on drop.

## Test Scenarios

| # | Scenario | Expected Result |
|---|----------|-----------------|
| 1 | Text-only reply | `FinalText` with provider text |
| 2 | Known tool + allowed | Real execution → `FinalText` with tool output |
| 3 | Known tool + policy denied | `ToolDenied` with policy reason |
| 4 | Unknown tool | `ToolDenied("tool_not_found")` |
| 5 | max_tool_steps exceeded | `ToolDenied("max_tool_steps_exceeded")` |
| 6 | file.read on real temp file | Correct file content returned |
| 7 | shell.exec with echo | "hello" in output |
| 8 | Tool lifecycle persistence | Deferred — persistence layer not yet wired into TurnEngine |
| 9 | Audit events emitted | Kernel audit sink captures tool execution events |

## Location

`crates/app/src/conversation/integration_tests.rs` — separate module from unit tests, gated behind `#[cfg(test)]`.

## Out of Scope

- Extending spec-runner to cover app-layer (different architectural boundary)
- Multi-step tool loops (max_tool_steps > 1)
- Real provider HTTP calls
- Channel adapter rendering tests
