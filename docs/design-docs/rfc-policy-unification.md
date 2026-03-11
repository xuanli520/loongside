# RFC: 策略系统统一 — 以 Token 为中心的安全模型

状态: 草案 | 日期: 2026-03-11 | 触发: Shell AST Sandbox RFC 评审

## 摘要

将所有权限/安全决策路径统一到基于 token 的 `PolicyEngine.authorize` + `PolicyExtensionChain` 体系上。消除并行的 `check_tool_call` 路径，以及散落在应用层中绕过内核安全治理的权限检查。

本 RFC 是 Shell + AST + Sandbox RFC 的前置依赖。Shell RFC 需要一个干净、可扩展的策略系统来承载基于 AST 的命令级决策；当前系统无法支撑。

## 问题陈述

对代码库的全面审计揭示了 **五条独立的权限/安全决策路径**，各自使用不同的机制、不同的状态模型、不同程度的内核集成：

### 路径 1: 基于 Token 的能力授权 (`authorize`)

**位置**: `kernel/src/policy.rs` → `StaticPolicyEngine::authorize()`
**调用点**: `kernel/src/kernel.rs:724` → `authorize_pack_operation()` → `authorize_or_audit_denial()`

检查项:
- Token 吊销（单个吊销 + 代际吊销）
- Token 过期
- Pack ID 匹配
- Capability 集合成员检查（`InvokeTool`, `MemoryRead` 等）

**特性**: 按 agent 隔离、按 token 隔离、动态、有审计、可吊销。

**评估**: 这是正确的基础。符合 Core Belief #1（内核优先）、#3（能力门控）、#4（审计一切）。

### 路径 2: 静态工具策略 (`check_tool_call`)

**位置**: `kernel/src/policy.rs:14-40` → `SHELL_HARD_DENY_COMMANDS`, `SHELL_APPROVAL_REQUIRED_COMMANDS`
**调用点**: `kernel/src/kernel.rs:786` → `enforce_tool_policy()`

检查项:
- Shell 命令名对照硬编码拒绝列表 → `Deny`
- Shell 命令名对照硬编码审批列表 → `RequireApproval`
- 仅检查第一个 token（命令名）；不感知参数、管道、命令替换

**特性**: 全局、无状态、硬编码、不区分 agent、运行时不可配置。

**问题**:
1. **违反 L1 设计文档**: 文档要求 "Risk detection signals/scoring must be profile-driven (external JSON), with inline overrides only as temporary overlays to avoid hardcoded policy drift"。实际实现是 `const &[&str]` 数组。
2. **无法支持动态权限**: "用户说本次会话始终允许 `gh`" — 没有状态可以存储这个决定。`check_tool_call` 是纯函数。
3. **无法区分不同 agent**: 所有 agent 共用同一套规则。Agent A 和 Agent B 看到完全相同的 allow/deny 决策，无论它们的 token 持有什么 capability。
4. **与 token 路径并行但无交互**: `authorize` 和 `check_tool_call` 在 `execute_tool_core` 中顺序执行，但不共享上下文。Token 身份对 `check_tool_call` 不可见。
5. **仅检查第一个 token**: 从 `parameters["command"]` 提取命令名。`curl https://evil.com; rm -rf /` 如果 `curl` 在审批列表中就能通过。

### 路径 3: 应用层白名单 (`shell_allowlist`)

**位置**: `app/src/tools/runtime_config.rs` → `ToolRuntimeConfig.shell_allowlist`
**调用点**: `app/src/tools/shell.rs:48-55`

检查项:
- 命令名对照 `BTreeSet<String>`（从配置/环境变量填充）
- 在 `MvpToolAdapter.execute_core_tool()` 内部检查，即**内核策略已经批准之后**

**特性**: 全局、静态（启动时通过 `OnceLock` 设置一次）、不区分 agent、不经过内核审计。

**问题**:
1. **影子安全路径**: 这是在 tool adapter 内部发生的第二道权限检查，发生在内核已经说 "Allow" 之后。拒绝产生 `Result::Err(String)`，而非 `PolicyDecision::Deny` — 完全绕过内核审计系统。
2. **与 `check_tool_call` 冗余**: 两者都检查命令名。白名单默认为 `echo,cat,ls,pwd`；`check_tool_call` 中的 deny/approval 列表覆盖不同的集合。两者之间的交互令人困惑。
3. **违反 Core Belief #1**: "No shadow paths that bypass policy." 这正是一条影子路径。

### 路径 4: 应用层文件根目录沙箱 (`resolve_safe_file_path_with_config`)

**位置**: `app/src/tools/file.rs:121` → `resolve_safe_file_path_with_config()`
**另见**: `app/src/tools/claw_import.rs` → `resolve_safe_path_with_config()`

检查项:
- 路径规范化和逃逸检测，对照 `ToolRuntimeConfig.file_root`
- 阻止 `../../` 遍历到配置根目录之外

**特性**: 全局、静态、不区分 agent、不经过内核审计。

**问题**:
1. **影子安全路径**: 与路径 3 相同的模式。文件路径沙箱是在 tool adapter 内部做出的安全决策，对内核的策略/审计系统不可见。
2. **不感知 capability**: `FilesystemRead` 和 `FilesystemWrite` capability 存在于 `Capability` 枚举中，但在工具执行路径中从未被检查。MVP 引导程序（`context.rs:62`）甚至不授予它们 — 只授予 `InvokeTool`, `MemoryRead`, `MemoryWrite`。文件工具之所以能工作，是因为白名单检查在 adapter 中，而非内核中。
3. **无审计轨迹**: 路径逃逸拒绝返回 `Err(String)`。不产生 `AuditEventKind`。静默的安全拒绝违反 Core Belief #4。

### 路径 5: 车道仲裁器风险评分 (`LaneArbiterPolicy`)

**位置**: `app/src/conversation/lane_arbiter.rs`

检查项:
- 用户输入文本对照硬编码 `high_risk_keywords`（"rm -rf", "drop table", "credential", "production" 等）
- 基于词数、连接词、标点的复杂度评分
- 路由到 Fast 或 Safe 执行车道

**特性**: 按请求、基于文本启发式、与内核策略完全无关。

**问题**:
1. **与内核完全脱节**: 这是一个对话层的路由决策，使用了安全相关的关键词，但与 token 系统、capability 模型、策略引擎没有任何连接。
2. **关键词与 `check_tool_call` 重叠**: "rm -rf" 同时出现在 `high_risk_keywords` 和 `SHELL_HARD_DENY_COMMANDS` 中。两套系统独立维护相似的安全词汇表。
3. **安全术语误用**: `risk_score` 暗示这是安全评估，但实际上只是 UX 路由。这造成了概念混淆 — 开发者可能误以为 Safe lane 提供了安全保障。

### 附注: `PlanNode.risk_tier`（死代码）

**位置**: `app/src/conversation/plan_ir.rs:18-22`

```rust
pub enum RiskTier { Low, Medium, High }
```

附加在 `PlanNode` 上，在部分构建路径中使用了 `Low` 和 `Medium`（`turn_coordinator.rs:888` 的 `select_safe_lane_risk_tier` 根据 `LaneDecision` 动态选择）。但未连接到任何内核策略决策 — 仅影响计划执行层的重试和验证行为。

**评估**: 不是死代码，但与内核安全策略脱节。作为 UX/执行层概念存在，未来可考虑与策略系统集成。

### 附注: Multi-Source-of-Truth 问题

除上述五条权限路径外，代码库中还存在多处"同一判断逻辑在多个位置重复"的问题：

**工具存在性检查（3 处重复）**:
- `turn_engine.rs:216,252` — `is_known_tool_name()` 提前拦截，返回 `TurnResult::policy_denied`，不经过 kernel
- `turn_coordinator.rs:1773` — `is_known_tool_name()` 提前拦截，返回 `PlanNodeError::policy_denied`，不经过 kernel
- `tools/mod.rs:75` — `execute_tool_core_with_config` match 分支兜底，返回 `Err("tool_not_found")`

三处维护同一个"已知工具列表"的硬编码判断。如果新增工具只改了 match 分支没改 `is_known_tool_name`，app 层会提前拒绝一个实际可执行的工具。且前两处的拒绝不经过 kernel 审计。

**Feature gate 禁用检查（3 处重复）**:
- `shell.rs:16` — `"shell tool is disabled in this build"`
- `file.rs:19,72` — `"file tool is disabled in this build"`
- `memory/mod.rs:119,137,155` — `"sqlite memory is disabled in this build"`

这些在 adapter 内部检查 feature gate，禁用时返回 `Err(String)`。Kernel 不知道某个工具是否被编译时禁用，禁用拒绝不产生审计事件。

**工具名规范化（2 处重复）**:
- `provider/shape.rs:26` — provider 响应解析时调用 `canonical_tool_name`
- `tools/mod.rs:64` — 工具执行时再次调用 `canonical_tool_name`

同一个映射逻辑在两个不同阶段执行。虽然当前两处调用同一个函数（single source of logic），但调用点分散意味着遗漏一处不会被另一处捕获。

**建议**: 工具注册表应成为 single source of truth — 工具是否存在、是否启用、名称映射，都应由 kernel 的 `ToolPlane` 或注册机制统一管理，而非在 app 层多处硬编码。

## 架构违规汇总

| Core Belief | 违规 |
|-------------|------|
| #1 内核优先，无影子路径 | 路径 3 和 4 是 tool adapter 内部的影子安全路径；工具存在性检查和 feature gate 禁用检查也绕过 kernel |
| #3 能力门控 | `FilesystemRead`/`FilesystemWrite` capability 存在但从未被执行 |
| #4 审计一切安全关键操作 | 路径 3 和 4 产生静默拒绝，无审计事件；工具存在性拒绝和 feature gate 拒绝同样无审计 |
| L1 "每个外部操作必须经过 L1" | 路径 3 和 4 在 L2（tool adapter 层）做安全决策 |
| L1 "基于 profile 的风险检测" | 路径 2 使用硬编码 `const` 数组而非外部 profile |
| L1 "拒绝列表必须具有最高优先级" | 路径 2 和路径 3 之间的优先级不明确，依赖执行顺序 |

## 提议设计: 以 Token 为中心的统一策略

### 原则

所有安全决策流经一个系统: `PolicyEngine.authorize()` + `PolicyExtensionChain`。Tool adapter 只负责执行；不做安全决策。

### 安全属性: PolicyExtension 注册的安全性

本 RFC 将更多策略逻辑迁移到 `PolicyExtensionChain`，需要确认该机制本身的安全性：

1. **构建时锁定，运行时不可变**: `register_policy_extension` 要求 `&mut self`（`kernel.rs:125`）。Kernel 在 bootstrap 完成后被 `Arc::new()` 包装（`context.rs:94`），此后无法获取 `&mut` 引用，运行时不可能注入新的扩展。
2. **AND 语义，只能收紧**: `PolicyExtensionChain.authorize()` 依次调用所有扩展，任一返回 `Err` 即拒绝（`policy_ext.rs:38-43`）。恶意或错误的扩展只能拒绝更多操作，不能放松其他扩展或核心策略的拒绝。符合 L1 规则 "Policy extensions can only tighten behavior, never weaken core policy"。
3. **注册方是可信代码**: 注册发生在 `bootstrap_kernel_context`（`context.rs:43`），属于编译时确定的应用初始化代码，不接受外部输入。

因此，将 `ToolPolicyExtension` 和 `FilePolicyExtension` 注册到 `PolicyExtensionChain` 不引入新的攻击面。

### Contracts 层变更 (L0)

**1. 不向 contracts 添加工具特定的策略类型。** `CommandAst`、文件路径规则等留在 app 层。Contracts 只承载通用的 capability 模型。

**2. `Capability` 枚举 — 本 RFC 无需变更。** `FilesystemRead`/`FilesystemWrite` 已存在。`ShellExec` 将由 Shell AST Sandbox RFC 添加。本 RFC 聚焦于正确接线现有 capability。

### Kernel 层变更 (L1)

**1. 扩展 `PolicyExtensionContext` 以携带请求参数:**

```rust
pub struct PolicyExtensionContext<'a> {
    pub pack: &'a VerticalPackManifest,
    pub token: &'a CapabilityToken,
    pub now_epoch_s: u64,
    pub required_capabilities: &'a BTreeSet<Capability>,
    pub request_parameters: Option<&'a serde_json::Value>,  // 新增
}
```

这是一个通用的、领域无关的扩展。`PolicyExtension` 实现可以检查请求参数以做出细粒度决策。现有实现忽略此字段（向后兼容）。

**2. 将 `check_tool_call` 逻辑迁移到 `PolicyExtensionChain`:**

`PolicyEngine` trait 上的 `check_tool_call` 变为默认空操作（或标记废弃）。Shell deny/approval 逻辑迁移到在内核引导时注册的 `ToolPolicyExtension`。该扩展:
- 通过 `PolicyExtensionContext` 接收 `request_parameters`
- 基于工具名和参数应用 deny/approval 规则
- 拒绝时返回 `PolicyError::ExtensionDenied`（由内核审计）
- 可配置: 规则从 profile（JSON/TOML）加载，非硬编码

**`RequireApproval` 语义映射**: 当前 `check_tool_call` 返回三种结果: `Allow`、`Deny(reason)`、`RequireApproval(prompt)`。而 `PolicyExtension::authorize_extension` 返回 `Result<(), PolicyError>` — 表面上只有 allow 和 error 两种语义。

但 `PolicyError` 已经包含 `ToolCallApprovalRequired { tool_name, prompt }` 变体（`contracts/src/errors.rs:41`），当前仅被 `enforce_tool_policy` 使用。`ToolPolicyExtension` 可以直接返回此变体来表达"需要审批"语义，无需新增类型。上层 `classify_kernel_error`（`turn_engine.rs`）已经将 `KernelError::Policy(_)` 分类为 `PolicyDenied`，但需要细化以区分 `ToolCallDenied`（硬拒绝）和 `ToolCallApprovalRequired`（可审批），确保 `TurnResult::NeedsApproval` 和 `TurnResult::ToolDenied` 的路由正确。

**3. `enforce_tool_policy` 重构:**

当前 `enforce_tool_policy`（`kernel.rs:786`）做三件事: (a) 构造 `PolicyRequest`，(b) 调用 `check_tool_call`，(c) 处理 `Deny`/`RequireApproval` 的审计记录。迁移后:
- (a) 构造逻辑保留，但改为将 tool name 和 parameters 注入 `PolicyExtensionContext.request_parameters`
- (b) `check_tool_call` 调用移除（逻辑已在 `ToolPolicyExtension` 中）
- (c) 审计记录由 `authorize_or_audit_denial` 统一处理（已有此能力）

具体来说，`authorize_pack_operation` 需要扩展签名以接收可选的 `request_parameters: Option<&serde_json::Value>`，用于填充 `PolicyExtensionContext.request_parameters`。非工具调用路径（task、connector、runtime、memory — 共 8 个调用点）传 `None`，工具调用路径（`execute_tool_core`、`execute_tool_extension` — 共 2 个调用点）传 `Some`。总计影响 `kernel.rs` 中 10 个 `authorize_pack_operation` 调用点，变更是机械性的。

### App 层变更

**1. 从 `ToolRuntimeConfig` 移除 `shell_allowlist`:**

白名单概念被 `ToolPolicyExtension` 规则替代。配置从 `ToolConfig.shell_allowlist` 迁移到策略 profile。

**2. 从 tool adapter 移除权限检查:**

- `shell.rs`: 移除白名单检查（第 48-55 行）。Adapter 只负责执行；策略已经决定。
- `file.rs`: `resolve_safe_file_path_with_config` 保留用于路径规范化，但安全决策（这个路径是否允许？）迁移到 `FilePolicyExtension`，由其检查 `FilesystemRead`/`FilesystemWrite` capability 和路径范围。
- `claw_import.rs`: 同样模式 — 路径安全检查迁移到策略扩展。

**3. 接线 `FilesystemRead`/`FilesystemWrite` capability:**

MVP 引导程序（`context.rs:62`）在启用文件工具时必须授予 `FilesystemRead`/`FilesystemWrite`。工具执行路径必须要求这些 capability。当前只要求 `InvokeTool`。

**4. `LaneArbiterPolicy` — 重命名并去重，不移除:**

这是 UX 路由机制，不是安全机制。变更:
- 将 `risk_score` 重命名为 `complexity_score` 或类似名称以避免与安全策略混淆
- 从 `high_risk_keywords` 中移除与安全策略重叠的关键词（如 "rm -rf"、"credential"、"token"、"secret"）。安全拦截由 `ToolPolicyExtension` 负责，`LaneArbiterPolicy` 只关注任务复杂度路由
- 保留纯复杂度信号（如 "deploy"、"production" 可保留为复杂度指标，但不应暗示安全含义）

### 迁移路径

这是一个重构 RFC — 无新的用户可见功能。迁移可以增量完成:

**步骤 1: 扩展 `PolicyExtensionContext`** — 添加 `request_parameters` 字段。零行为变更。所有现有代码继续工作。

**步骤 2: 实现 `ToolPolicyExtension`** — 在内核引导时注册。读取当前硬编码在 `check_tool_call` 中的相同 deny/approval 规则。与当前系统行为一致，但现在通过 `PolicyExtensionChain` 运行。

**步骤 2.5: 行为一致性验证** — 在 `check_tool_call` 和 `ToolPolicyExtension` 并行运行期间，添加对照测试: 对同一组输入，两条路径必须产生相同的决策结果。现有 `kernel/src/policy.rs` 中的 6 个 policy 测试（`static_policy_denies_destructive_shell_commands` 等）需要复制为 `ToolPolicyExtension` 的等价测试。只有对照测试全部通过后才能进入步骤 3。

**步骤 3: 废弃 `check_tool_call`** — 使 `StaticPolicyEngine::check_tool_call` 无条件返回 `Allow`。所有 shell 策略逻辑现在在 `ToolPolicyExtension` 中。验证所有现有测试通过。

**步骤 4: 接线文件 capability** — `file.read`/`file.write` 的 `execute_tool` 要求 `FilesystemRead`/`FilesystemWrite`。MVP 引导程序授予它们。`FilePolicyExtension` 执行路径范围检查。移除 `resolve_safe_file_path_with_config` 的安全门控角色（保留为规范化工具）。

**步骤 5: 外部化策略规则** — 将硬编码的 deny/approval 列表迁移到启动时加载的 JSON/TOML profile。满足 L1 "profile-driven" 要求。

**步骤 6: 清理** — 从 `ToolRuntimeConfig` 和 `ToolConfig` 移除 `shell_allowlist`。从 `PolicyEngine` trait 移除 `check_tool_call`（或保留为废弃空操作一个发布周期）。重命名 `LaneArbiterPolicy` 的风险术语。

### 本 RFC 为后续 RFC 开启的能力

统一完成后，后续 RFC 可以:
- 通过注册新的 `PolicyExtension` 实现来添加领域特定的策略逻辑，无需修改内核
- 利用 `request_parameters` 传递领域特定的上下文信息给策略扩展
- 所有新增的策略决策自动获得内核审计覆盖
- 利用 `PolicyExtensionContext` 中的 `token` 信息实现按 agent 差异化的策略决策

## 与 Core Beliefs 的对齐

| Belief | 本 RFC 如何对齐 |
|--------|----------------|
| #1 内核优先 | 消除所有影子安全路径；每个决策都经过 L1 |
| #2 无破坏性变更 | `PolicyExtensionContext` 扩展是增量的；`check_tool_call` 废弃而非移除 |
| #3 能力门控 | `FilesystemRead`/`FilesystemWrite` 实际被执行；所有工具调用要求正确的 capability |
| #4 审计一切 | 所有拒绝流经 `PolicyExtensionChain` → 内核审计；不再有静默的 `Err(String)` |
| #5 7-crate DAG | 无新 crate；策略扩展在 app 注册，trait 在 kernel 定义 |
| #6 测试即契约 | 步骤 2.5 要求对照测试验证行为一致性；每步迁移需现有测试全部通过 |
| #7 无聊技术 | JSON/TOML 策略 profile；无新依赖 |
| #8 仓库即记录系统 | 本 RFC 记录所有设计决策和迁移路径；实施中的变更必须更新本文档 |
| #9 机械化执行 | 策略规则从 profile 加载，非硬编码常量 |
| #10 YAGNI | 本 RFC 不做 capability 参数化；只正确接线现有 capability |

## 开放问题与决策建议

### 1. `check_tool_call` 移除时间线

**决策: 本 RFC 中废弃（默认返回 Allow），下一个版本移除 trait 方法。**

理由: 直接移除改变 `PolicyEngine` trait 签名，所有实现者（包括测试中的 mock）都要改。折中方案: 本 RFC 让 `StaticPolicyEngine::check_tool_call` 返回 `Allow`，在方法上加 `#[deprecated]` 注解。下一个版本正式从 trait 移除。这给外部消费者（如果有的话）一个迁移窗口，同时不拖延清理。

### 2. 策略 profile 格式

**决策: TOML，与现有配置体系一致。**

理由: 项目已依赖 `toml`（`config-toml` feature），用户配置（`LoongClawConfig`）全部是 TOML。策略 profile 是配置的一部分，不是独立的安全制品。用同一种格式降低认知负担。JSON 更适合机器生成的安全 profile（如 SIEM 导出），但那是审计输出，不是策略输入。如果未来需要 JSON profile（比如从外部安全平台导入），加一个 loader 不难，但现在 YAGNI。

### 3. `LaneArbiterPolicy` 范围

**决策: 保持原样，只重命名术语。不集成到策略扩展系统。**

理由: `LaneArbiterPolicy` 是对话层的 UX 路由（Fast vs Safe lane），决定的是"用多复杂的执行流程处理这个请求"，不是"这个操作是否被允许"。把它塞进 `PolicyExtensionChain` 会模糊安全策略和 UX 策略的边界。重命名 `risk_score` → `routing_score`，从 `high_risk_keywords` 中移除安全词汇（`rm -rf`, `credential`, `secret`），让它只关注任务复杂度信号。

### 4. `PlanNode.RiskTier` 是否接入策略系统

**决策: 不接入，保持为执行层概念。**

理由: `RiskTier` 当前影响的是计划执行的重试策略和验证行为（`select_safe_lane_risk_tier`），不是安全决策。把它接入策略系统意味着 kernel 需要理解"计划节点"这个领域概念，违反 L0 "no domain-specific semantics"。如果未来需要"高风险操作需要额外审批"，应该通过 `PolicyExtension` 在工具调用时判断，而不是在计划构建时标记。YAGNI。

### 5. `shell_allowlist` 配置向后兼容

**决策: 发出废弃警告，保留一个版本周期的自动转换。**

理由: 用户可能在 TOML 中配置了 `shell_allowlist`。静默忽略会导致行为变化（之前允许的命令突然被拒绝）且用户不知道为什么。方案: 启动时检测到 `shell_allowlist` 配置项存在时，(a) 自动转换为新的策略 profile 等价规则，(b) 输出一条废弃警告说明迁移方式。下一个版本移除自动转换，未迁移的配置报错。

### 6. `PolicyDecision` 类型的未来

**决策: 与 `check_tool_call` 一起废弃，同版本移除。**

理由: `PolicyDecision` 的三种语义（`Allow`/`Deny`/`RequireApproval`）已被 `PolicyError` 的变体覆盖 — `Ok(())` = Allow，`ToolCallDenied` = Deny，`ToolCallApprovalRequired` = RequireApproval。保留 `PolicyDecision` 意味着 contracts 中有两套表达相同语义的类型，增加混淆。它没有被 `PolicyExtension` 使用，唯一消费者是 `enforce_tool_policy`，而 `enforce_tool_policy` 本身也在被简化。跟 `check_tool_call` 同步移除是最干净的。
