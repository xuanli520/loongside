# RFC: Shell + AST + Sandbox Execution Architecture

Status: Draft | Date: 2026-03-10 | Trigger: Issue #18 | Prerequisite: RFC Policy System Unification

## Summary

Transform `shell.exec` from the current programmatic execution model (`Command::new()`) to a three-layer architecture: Shell execution + AST semantic parsing + Sandbox isolation. LoongClaw supports only specific known shells, with a tailored AST parsing adapter for each, and a corresponding upgrade to the command permission system.

## Motivation

### Fundamental Flaws in the Current Implementation

The current `shell.rs` directly calls `Command::new(command).args(&args)` to spawn processes without going through a shell interpreter. This causes three irreconcilable problems:

1. **Cross-platform breakage** (Issue #18): The default allowlist `echo,cat,ls,pwd` is entirely unavailable on Windows — these are shell builtins, not standalone binaries. Adding `cmd` or `powershell` to the allowlist to work around this renders the allowlist useless, since `cmd /C <anything>` can execute arbitrary commands.

2. **Capability deficit**: No support for pipes, redirections, or command composition. When an AI model issues `cargo build 2>&1 | grep error`, it fails outright. These are indispensable operations in everyday development.

3. **Fragile permission model**: `SHELL_HARD_DENY_COMMANDS` / `SHELL_APPROVAL_REQUIRED_COMMANDS` in `policy.rs` only inspect the first token. In a shell environment, semicolon chaining, command substitution, and redirections can trivially bypass this.

### Product Positioning Requirements

LoongClaw is positioned as an AI assistant for everyday users. It should complete tasks automatically and quickly, rather than constantly requesting user confirmation. This requires:

- Full shell expressiveness (pipes, redirections, conditional execution)
- Consistent cross-platform experience (macOS / Linux / Windows)
- Security model shifting from "block" to "understand + isolate + audit"

### Competitive Validation

- [Claude Code](https://code.claude.com/docs/en/sandboxing): Persistent bash session + AST parsing + OS-level sandbox (macOS Seatbelt / Linux bubblewrap)
- [Gemini CLI](https://geminicli.com/docs/cli/sandbox): Shell + tree-sitter-bash AST + tiered sandbox (macOS Seatbelt / Linux application-level soft sandbox / Docker / gVisor)

Both have validated the Shell + AST + Sandbox model in production environments.

## Architecture Overview

Note: The Stage numbers below refer to stages in the shell execution pipeline and are unrelated to the L0–L4 layers in `layered-kernel-design.md`.

```text
AI emits command string
    │
    ▼
┌─────────────────────────────┐
│  Stage 1. AST Parsing        │  ShellAstAdapter (app-internal impl)
│  Extract CommandRoots        │  Bash: tree-sitter (in-process)
│  Failure → fail-closed       │  PowerShell: native .NET Parser
└──────────────┬──────────────┘
               │ CommandAst (app-internal type, serialized into PolicyRequest.parameters)
               ▼
┌─────────────────────────────┐
│  Stage 2. Policy Check       │  PolicyExtensionChain.authorize()
│  Decide based on CommandAst  │  + ShellPolicyExtension (registered by app)
│  Select sandbox profile      │
└──────────────┬──────────────┘
               │ Result<(), PolicyError>
               ▼
┌─────────────────────────────┐
│  Stage 3. Sandboxed Exec     │  SandboxProvider (app-internal impl)
│  Persistent Shell Session    │  macOS: Seatbelt
│  OS/app-level isolation      │  Linux: bubblewrap
│  Structured audit output     │  Windows: application-level soft sandbox
└─────────────────────────────┘
```

**Mapping to the project's layered model**: The entire shell execution pipeline is encapsulated as a `CoreToolAdapter` implementation (`ShellToolAdapter`) registered in the kernel L2 `ToolPlane`. Stages 1 and 3 are internal implementation details of the adapter (app layer). Stage 2 executes through the kernel L1 `PolicyExtensionChain` mechanism (see prerequisite: RFC Policy System Unification).

## Design Decisions

### Decision 1: Shell Execution vs Programmatic Execution

**Choice: Shell execution.**

The remediation paths for programmatic execution (`Command::new()`) were evaluated and excluded in the preliminary design assessment. Whether platform-aware command mapping (high maintenance cost, `cmd /C` opens a backdoor), WSL routing (path translation pitfalls, not pre-installed), Docker wrapping (heavy dependency chain, user environment variance), or structured pipeline JSON (this format does not exist in AI training data) — none can simultaneously satisfy cross-platform consistency and full shell expressiveness.

Shell execution embraces the full expressiveness of the shell, shifting security responsibility to the three-layer defense of AST parsing + sandbox isolation + audit.

### Decision 2: Per-Shell Native AST vs Unified Parser

**Choice: ShellProvider trait + pluggable AST adapters, per-shell optimal parsing strategy.**

| Shell | Parsing Strategy | Rationale |
|-------|-----------------|-----------|
| Bash | tree-sitter-bash (Rust native bindings, in-process) | Microsecond-level parsing, zero external dependencies, high maturity |
| PowerShell | .NET native `[Parser]::ParseInput` | Highest accuracy, reuses the persistent pwsh session process (see Decision 3) |

Only Bash and PowerShell are supported initially. Zsh, Fish, and other shells are not included in the support matrix until a mature and reliable parsing solution is available.

**Excluded alternatives:**

- **Unified tree-sitter for all shells**: tree-sitter-powershell exists but is far less mature than bash. PowerShell's syntax complexity (cmdlet pipelines, script blocks, .NET method calls) makes it difficult for third-party parsers to guarantee accuracy. Inaccurate parsing directly undermines security policy.
- **Bash-only standardization**: Requiring Git Bash / WSL on Windows violates the "everyday user assistant" positioning.
- **Zsh / Fish degraded compatibility via tree-sitter-bash**: Although Zsh's common syntax is highly compatible with Bash, the compatibility boundary is ambiguous, and fail-closed on parse failure would lead to unpredictable user experience. No commitment is made without a reliable solution.

All parser outputs are unified into a `CommandAst` IR; the policy engine only sees the IR, not raw shell syntax.

**Crate placement**: `CommandAst`, `ShellAstAdapter`, `ShellSessionProvider`, and `SandboxProvider` are all app-crate-internal types and do not enter `contracts` or `kernel`. This follows the layered design L0 rule "No domain-specific semantics in L0" — shell is a domain-specific concept that does not belong in the kernel ABI. The entire shell execution chain is encapsulated as a `CoreToolAdapter` implementation (`ShellToolAdapter`), registered in the kernel L2 `ToolPlane`, consistent with the existing `MvpToolAdapter` pattern.

### Decision 3: Persistent Session vs Stateless Execution

**Choice: Persistent shell session.**

| Dimension | Persistent Session | Stateless Execution (current) |
|-----------|-------------------|-------------------------------|
| AI experience | `cd` / `export` state preserved across commands; AI doesn't need to repeat context each time | Each command is independent; AI must write `cd xxx && yyy` |
| Sandbox config | Injected once at session startup, low overhead | Sandbox reconfigured per command, high overhead |
| PowerShell AST | Reuses the same long-lived pwsh process for both parsing and execution, zero additional latency | Spawns pwsh for parsing each time, 50–200ms cold start latency |
| Isolation | In-session state may be polluted by malicious commands | Clean environment each time, stronger isolation |
| Competitive validation | Claude Code uses this model | — |

Persistent sessions and PowerShell native AST are a natural pairing: since a long-lived pwsh process is already maintained for command execution, AST parsing can reuse the same process, eliminating the parsing latency issue entirely.

**Isolation risk mitigation**: The sandbox injects OS-level restrictions (filesystem/network) at session startup. Even if in-session state is polluted, the blast radius remains constrained by the sandbox. Sessions are automatically reclaimed on idle timeout to prevent long-term pollution.

**Token revocation and session termination mechanism**: `ShellToolAdapter` (app layer) maintains a `session_id → token_id` mapping. Before each command execution, the adapter calls `PolicyEngine::authorize()` to verify the token is still valid. If the token has been revoked via `revoke_token` or `revoke_generation`, authorize returns `PolicyError`, and the adapter immediately terminates the corresponding shell process and cleans up the session. This is poll-based verification (per-command check) and does not require introducing a new callback/notification mechanism in the kernel — it reuses the existing `authorize` path.

### Decision 4: Tiered Sandbox Strategy

**Choice: OS-native first, application-level soft sandbox as fallback.**

Referencing production-validated approaches from Claude Code and Gemini CLI, a per-platform tiered strategy is adopted:

#### Sandbox Integration with the LoongClaw Capability Model

LoongClaw's permissions are not based on a "working directory" concept, but on the `Capability` enum + `CapabilityToken` + `VerticalPackManifest`. The sandbox must align with this system:

- **`FilesystemRead` / `FilesystemWrite`**: The sandbox's filesystem isolation boundary is determined by `ToolRuntimeConfig.file_root`. The current `file.rs` already implements `resolve_safe_file_path_with_config()` for path escape checking. The sandbox layer enforces the same boundary at the OS level — paths outside `file_root` are not writable at the OS level (even if application-layer checks are bypassed).
- **`NetworkEgress`**: When the pack manifest does not grant `NetworkEgress`, the sandbox cuts off network access at the OS level. When granted, allowed domains can be controlled via proxy.
- **`InvokeTool` + `ShellExec`**: Shell session creation requires the `InvokeTool` capability (consistent with the existing tool invocation path), plus the new `ShellExec` capability. `ShellExec` is an additive new variant in the `Capability` enum, allowing pack manifests to control shell access independently of generic tool invocation for finer-grained least-privilege control (Core Belief #3). Packs without `ShellExec` can still invoke other tools but cannot execute shell commands.
- **Token lifecycle**: Shell sessions are bound to a `CapabilityToken`. When the token expires or is revoked, the corresponding shell session must be terminated.

#### macOS: Seatbelt Hard Sandbox

Uses the macOS native `sandbox-exec` mechanism with dynamically generated `.sb` profiles:

- Filesystem: `file_root` directory is read-write (if the token holds `FilesystemWrite`); outside `file_root` is read-only or inaccessible; system-sensitive directories (`/System`, `~/.ssh`) are hard-locked
- Network: Determined by `NetworkEgress` capability — OS-level network cutoff without this capability; configurable proxy when granted
- Processes: All child processes inherit sandbox restrictions

Dual-mode switching:
- **Restrictive**: Network disabled + `file_root` read-only, suitable for pure analysis scenarios (token holds only `FilesystemRead`)
- **Permissive**: HTTP/HTTPS allowed + `file_root` read-write (token holds `FilesystemWrite` + `NetworkEgress`)

#### Linux: bubblewrap Hard Sandbox

Uses [bubblewrap](https://github.com/containers/bubblewrap) (same choice as Claude Code):

- Lightweight namespace isolation, no root required
- Filesystem bind-mount: `file_root` mounted per capability (ro/rw), other paths invisible
- Network namespace: Isolated network namespace created or not based on `NetworkEgress` capability

#### Windows: Application-Level Soft Sandbox

Windows lacks native lightweight sandbox primitives. A multi-layer application-level defense is adopted (validated by Gemini CLI):

1. **file_root confinement**: All file operations are resolved and verified to be within `file_root`, intercepting `../../` path escape attempts (reuses existing `resolve_safe_file_path_with_config` logic, with OS-level hardening added at the shell layer)
2. **Environment variable sanitization**: Strips sensitive credentials (`AWS_SECRET`, SSH keys, etc.)
3. **Runtime feature deprivation**: PowerShell Constrained Language Mode or bash `shopt` security patches
4. **Resource enforcement**: Process tree kill (`taskkill /t`), output truncation, binary stream sniffing

**Excluded alternatives:**

- **Unified Docker across all platforms**: High deployment barrier, poor bind mount file watching performance, "one-click install" easily becomes "one-click troubleshooting"
- **No Windows sandbox**: Violates cross-platform consistency goals

#### Sandbox Configuration Hierarchy

```text
Pack Manifest granted_capabilities (highest priority, determines capability ceiling)
  └── CapabilityToken allowed_capabilities (cannot exceed manifest-granted scope)
        └── User Settings (can tighten, cannot loosen)
              └── Platform default sandbox policy (fallback)
```

Sandbox policies can only be tightened by upper layers, never loosened (consistent with Core Belief L1 rule: "Policy extensions can only tighten behavior, never weaken core policy"). Sandbox profile generation is capability-driven — not static configuration, but dynamically determined by the capabilities held by the current token.

### Decision 5: Permission System Upgrade

**Prerequisite**: This decision depends on the completion of RFC Policy System Unification, which migrates all security decisions to the `PolicyExtensionChain` and adds `request_parameters` to `PolicyExtensionContext`. The description below assumes that RFC has been implemented.

The hardcoded command name lists in `policy.rs` will have been replaced by `ToolPolicyExtension` (per the Policy Unification RFC). This RFC adds a `ShellPolicyExtension` alongside it for AST-aware command-level decisions.

#### New Permission Model

```text
Raw command string
    │
    ▼
AST Parse → CommandAst IR
    │
    ├── command_roots: ["cargo", "grep"]    ← actual executable names
    ├── has_redirects: true
    ├── has_subshell: false
    ├── has_command_substitution: false
    │
    ▼
ShellPolicyExtension (via PolicyExtensionChain, decides based on IR)
    ├── Denylist match on command_roots → Err(ToolCallDenied)
    ├── Allowlist match on command_roots → Ok(())
    ├── Unknown commands → Err(ToolCallApprovalRequired) / auto-allow within sandbox
    ├── AST parse failure → fail-closed (Err(ToolCallApprovalRequired))
    └── Dangerous pattern detection (command_substitution + denylisted command → Err(ToolCallDenied))
```

#### Policy Persistence

When a user confirms "always allow", what is saved is the `command_roots` extracted by the AST (e.g., `cargo`), not the raw command string. Next time, as long as all root commands parsed from the AST are in the allowlist, the command is silently approved.

#### Integration with PolicyExtensionChain

`ShellToolAdapter` (app layer) serializes `CommandAst` into `PolicyRequest.parameters["command_ast"]` before the kernel calls `PolicyExtensionChain.authorize()`. The `ShellPolicyExtension` (app layer, implementing the kernel's `PolicyExtension` trait) deserializes `CommandAst` from `PolicyExtensionContext.request_parameters` and applies AST-aware rules. If `request_parameters` does not contain `command_ast`, `ShellPolicyExtension` passes through without intervention.

The `PolicyExtensionContext.request_parameters` field is provided by the Policy Unification RFC. No additional kernel changes are needed for this RFC.

### Decision 6: Structured Audit

Shell execution audit is upgraded from the current `{command, args}` structure. A new typed `AuditEventKind` variant is added (additive, does not modify existing variants):

```rust
// contracts/src/audit_types.rs — new variant
AuditEventKind::ShellCommandExecuted {
    pack_id: String,
    session_id: String,
    shell: String,                    // "bash" | "powershell"
    raw_command: String,
    command_roots: Vec<String>,       // root commands extracted by AST
    ast_parse_success: bool,
    sandbox_profile: String,          // "restrictive" | "permissive"
    policy_decision: String,          // "allow" | "deny" | "require_approval"
    exit_code: Option<i32>,
    stdout_bytes: u64,
    stderr_bytes: u64,
    truncated: bool,
    binary_detected: bool,
    duration_ms: u64,
}
```

Example audit output:

```json
{
  "raw_command": "cargo test 2>&1 | head -20",
  "shell": "bash",
  "shell_session_id": "sess-a1b2c3",
  "command_roots": ["cargo", "head"],
  "ast_parse_success": true,
  "sandbox_profile": "permissive",
  "policy_decision": "auto_allow",
  "exit_code": 0,
  "stdout_bytes": 1024,
  "stderr_bytes": 0,
  "truncated": false,
  "binary_detected": false,
  "duration_ms": 3400
}
```

stdout/stderr content itself does not enter the audit event (may contain sensitive information and is large in volume); only byte counts are recorded. Full output is returned to the caller via `ToolCoreOutcome.payload`.

### Decision 7: Output Management

In a persistent session, a single command can produce arbitrarily large output (e.g., `find / -type f`, `cat` on a large file). Without limits, this can exhaust memory.

Strategy:

1. **Output truncation**: Configurable maximum output bytes (default 16MB). When exceeded, output is truncated and `ToolCoreOutcome.payload` is marked with `truncated: true`. The audit event records the actual byte count.
2. **Binary stream detection**: The first 8KB of output is sniffed. If non-UTF-8 / binary content is detected, reading is terminated early and marked with `binary_detected: true`, avoiding large volumes of meaningless binary data entering the AI context.
3. **Streaming reads**: The persistent session uses sentinel/delimiter markers to delineate each command's output boundary. stdout/stderr are read in chunks, and truncation decisions are made during the reading process without waiting for the command to complete.

The `stdout_bytes` / `stderr_bytes` fields in the audit event record the actual byte count before truncation, facilitating post-hoc analysis.

### Decision 8: Interactive Command Limitation

The persistent session communicates via stdin/stdout pipes and **does not allocate a TTY/PTY**. This means interactive commands requiring terminal control sequences (`vim`, `less`, `top`, password prompts, etc.) inherently cannot run — they will hang or error in pipe mode.

This is an intentional design boundary, not a deficiency:

- AI agents do not need interactive editors (they have `file.read` / `file.write` tools)
- Password prompts should be handled through credential management mechanisms, not shell interaction
- This is consistent with Claude Code's limitation: "Cannot handle vim, less, or password prompts"

`ShellToolAdapter` should automatically terminate the current command on timeout (configurable, default 120s) while preserving the session, preventing interactive commands from permanently blocking the session.

## New Types and Traits

### CommandAst IR (app-crate-internal type)

```rust
/// Shell-agnostic intermediate representation of a parsed command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandAst {
    /// The executable names extracted from the command.
    /// e.g. "cargo build | grep error" → ["cargo", "grep"]
    pub command_roots: Vec<String>,
    /// Whether the command contains I/O redirections.
    pub has_redirects: bool,
    /// Whether the command contains subshell invocations.
    pub has_subshell: bool,
    /// Whether the command contains command substitutions ($(...) or `...`).
    pub has_command_substitution: bool,
    /// Whether the AST parse completed without errors.
    pub parse_success: bool,
}
```

### ShellAstAdapter trait (app-crate-internal type)

```rust
/// Parses a raw command string into a shell-agnostic CommandAst.
pub trait ShellAstAdapter: Send + Sync {
    /// The shell dialect this adapter handles.
    fn shell_name(&self) -> &str;

    /// Parse a command string into a CommandAst.
    /// Returns a CommandAst with parse_success=false on parse failure
    /// (fail-closed: policy layer treats this as RequireApproval).
    fn parse(&self, command: &str) -> CommandAst;
}
```

### ShellSessionProvider trait (app-crate-internal type)

```rust
/// Manages persistent shell sessions with sandbox integration.
pub trait ShellSessionProvider: Send + Sync {
    /// The shell type this provider manages.
    fn shell_name(&self) -> &str;

    /// Create a new sandboxed shell session.
    fn create_session(&self, config: &ShellSessionConfig) -> Result<ShellSession, ShellError>;

    /// Execute a command in an existing session.
    fn execute(&self, session: &mut ShellSession, command: &str) -> Result<ShellExecResult, ShellError>;

    /// Terminate a session and clean up resources.
    fn terminate(&self, session: ShellSession) -> Result<(), ShellError>;
}
```

### SandboxProvider trait (app-crate-internal type)

```rust
/// Provides platform-specific sandbox configuration.
pub trait SandboxProvider: Send + Sync {
    /// The platform this provider supports.
    fn platform(&self) -> &str;

    /// Generate sandbox configuration for a shell session.
    fn configure(&self, policy: &SandboxPolicy) -> Result<SandboxConfig, SandboxError>;
}
```

## Execution Flow

```text
1. Agent task starts
   → Detect platform → Select ShellSessionProvider + SandboxProvider
   → Create persistent shell session (sandbox injected at this point)

2. AI emits command "cargo build 2>&1 | grep error"
   → ShellAstAdapter.parse() → CommandAst { roots: ["cargo", "grep"], ... }

3. Kernel calls authorize_pack_operation (with request_parameters)
   → PolicyExtensionChain.authorize() invokes all registered extensions:
     - ToolPolicyExtension (from Policy Unification RFC) — baseline tool-level allow/deny
     - ShellPolicyExtension — deserializes CommandAst from request_parameters["command_ast"],
       iterates each item in command_roots against denylist/allowlist, performs AST-aware fine-grained decision
   → All extensions pass → Ok(())

4. ShellSessionProvider.execute(session, "cargo build 2>&1 | grep error")
   → Execute in the sandboxed persistent session
   → Return ShellExecResult { exit_code, stdout, stderr, duration }

5. Construct audit event + ToolCoreOutcome and return
```

## Supported Shell Matrix

| Platform | Default Shell | AST Parser | Sandbox |
|----------|--------------|------------|---------|
| macOS | bash | tree-sitter-bash | Seatbelt |
| Linux | bash | tree-sitter-bash | bubblewrap |
| Windows | PowerShell | .NET native Parser | Application-level soft sandbox |

Zsh / Fish are not included in the support matrix until a mature and reliable parsing solution is available.

## Impact on Existing Code

### Files to Modify

| File | Change |
|------|--------|
| `contracts/src/contracts.rs` | Add `ShellExec` variant to `Capability` enum (additive) |
| `contracts/src/audit_types.rs` | Add `ShellCommandExecuted` variant to `AuditEventKind` enum (additive) |
| `app/src/tools/shell.rs` | Rewrite: from `Command::new()` to `ShellToolAdapter` (implementing `CoreToolAdapter`), internally encapsulating AST parsing + sandboxed execution |
| `app/src/tools/mod.rs` | Register `ShellToolAdapter`, add new shell module structure |
| `app/src/tools/runtime_config.rs` | `shell_allowlist` removed (replaced by policy profile per Policy Unification RFC) |

### New Files (all in app crate)

| File | Content |
|------|---------|
| `app/src/tools/shell/ast.rs` | `CommandAst` type + `ShellAstAdapter` trait + tree-sitter-bash implementation |
| `app/src/tools/shell/session.rs` | `ShellSessionProvider` trait + Bash/PowerShell persistent session implementations |
| `app/src/tools/shell/sandbox.rs` | `SandboxProvider` trait + platform sandbox implementations |
| `app/src/tools/shell/policy_ext.rs` | `ShellPolicyExtension` (implements kernel `PolicyExtension` trait) |

### Files That Do Not Need Modification

- `contracts/src/policy_types.rs` — `PolicyRequest` struct unchanged; `CommandAst` serialized into existing `parameters: Value` field
- `contracts/src/tool_types.rs` — `ToolCoreRequest` / `ToolCoreOutcome` unchanged
- `kernel/src/policy.rs` — `check_tool_call` already deprecated per Policy Unification RFC; no further changes
- `kernel/src/policy_ext.rs` — `PolicyExtensionContext.request_parameters` already added per Policy Unification RFC; no further changes
- `kernel/src/tool.rs` — `CoreToolAdapter` / `ToolPlane` unchanged
- `kernel/src/audit.rs` — `AuditSink` trait unchanged; only a new `AuditEventKind` variant is added
- `app/src/tools/file.rs` — unaffected

### Files Requiring Additive Extension

- `kernel/src/policy_ext.rs` — `PolicyExtensionContext.request_parameters` field already added by Policy Unification RFC; no additional kernel changes needed for this RFC

### Dependency Changes

| Crate | New Dependency | Notes |
|-------|---------------|-------|
| app | `tree-sitter`, `tree-sitter-bash` | Bash AST parsing (feature-gated: `shell-ast-bash`) |
| app | No new crate | PowerShell AST via spawning pwsh, no Rust dependency |

tree-sitter is a mature project maintained by GitHub, consistent with Core Belief #7 (boring technology preferred).

## Alignment with Core Beliefs

| Belief | Alignment |
|--------|-----------|
| #1 Kernel-first | All shell execution paths go through the kernel's policy/audit system |
| #2 No breaking changes | New traits, IR, Capability variant, and AuditEventKind variant added; no existing signatures modified |
| #3 Capability-gated | New `ShellExec` capability; shell session creation requires `InvokeTool` + `ShellExec` |
| #4 Audit everything | New typed `AuditEventKind::ShellCommandExecuted` variant |
| #5 7-crate DAG | `ShellExec` capability and `ShellCommandExecuted` audit variant in contracts (leaf); `ShellToolAdapter` implementation in app; kernel introduces no shell domain concepts |
| #6 Tests are the contract | Each Phase delivery must include: AST parsing correctness tests, policy decision tests, sandbox boundary tests, `ShellCommandExecuted` golden audit event tests |
| #7 Boring technology | tree-sitter (GitHub-maintained), Seatbelt (macOS native), bubblewrap (container ecosystem standard) |
| #8 Repo is system of record | This RFC records all design decisions and their rationale; changes during implementation must update this document |
| #9 Enforce mechanically | AST parse failure → fail-closed enforced by code; sandbox enforced by OS primitives; `ShellExec` capability enforced by kernel authorize |
| #10 YAGNI | Initially implement Bash + PowerShell only; Zsh/Fish not included in support matrix |

## Incremental Delivery Plan

0. **Phase 0 (prerequisite)**: RFC Policy System Unification — `PolicyExtensionContext.request_parameters`, `ToolPolicyExtension`, `check_tool_call` deprecation, `shell_allowlist` removal
1. **Phase 1**: `CommandAst` IR + `ShellAstAdapter` trait + tree-sitter-bash implementation + `ShellPolicyExtension`
2. **Phase 2**: Persistent shell sessions + PowerShell native AST adapter
3. **Phase 3**: macOS Seatbelt + Linux bubblewrap sandbox
4. **Phase 4**: Windows application-level soft sandbox + structured audit

## Open Questions — Call for Comments

The following design decisions have been given initial choices in this RFC, but team members are welcome to offer differing perspectives:

1. **Persistent session vs stateless execution**: Persistent sessions improve AI experience and PowerShell AST performance, but introduce in-session state pollution risk. Are there scenarios that require supporting a stateless mode as a fallback?

2. **PowerShell native AST vs tree-sitter-powershell**: Native AST was chosen for highest accuracy and reuse of the persistent process. However, this means pwsh is a hard dependency on Windows. For Windows environments without PowerShell 7+ installed, is a fallback strategy needed?

3. **Windows soft sandbox security boundary**: Application-level soft sandboxing is inherently "best effort" and less reliable than OS-level sandboxing. Should Windows default to requiring more frequent user confirmation to compensate for weaker sandbox strength?

4. **Fail-closed strategy on AST parse failure**: The current design is parse failure → `RequireApproval`. For unattended agent scenarios, this causes task blocking. Is a configurable option for "auto-allow unparsed commands within sandbox" needed?

5. **Sandbox profile granularity**: The current design dynamically generates sandbox profiles based on Capability combinations (Restrictive / Permissive). Is finer-grained control needed (e.g., distinguishing allowed domain lists within `NetworkEgress`, further restricting writable subdirectories within `FilesystemWrite`)?

6. **Shell session and CapabilityToken lifecycle binding**: The current design terminates sessions when tokens expire/are revoked. If a token is re-issued (renewed), should the existing session be reused or should a new session be forced?

