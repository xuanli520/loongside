# Loong 开发者快速上手指南

> 本文件为简体中文版开发者入门指南。英文原版请参考 [AGENTS.md](./AGENTS.md)。

---

## 目录

1. [环境要求](#1-环境要求)
2. [快速开始](#2-快速开始)
3. [项目结构概览](#3-项目结构概览)
4. [常用命令速查](#4-常用命令速查)
5. [开发工作流](#5-开发工作流)
6. [常见问题](#6-常见问题)

---

## 1. 环境要求

### 必须安装

| 依赖 | 版本要求 | 说明 |
|------|----------|------|
| **Rust** | stable（建议 1.75+） | 通过 `rustup` 安装 |
| **Go** | 最新稳定版 | 用于 `task check:conventions` 规范检查 |
| **Git** | 任意近期版本 | 版本控制 |

### 可选但推荐安装

| 依赖 | 安装方式 | 说明 |
|------|----------|------|
| **task CLI**（go-task） | `brew install go-task/tap/go-task` | 提供 `Taskfile.yml` 中的快捷命令封装，如 `task verify` |
| **cargo-deny** | `cargo install cargo-deny` | 依赖许可/安全审计，CI 会调用 |
| **fork 仓库** | 在 GitHub 网页操作 | 贡献前需先 fork 主仓库 |

### 验证环境

```bash
rustc --version   rustc 1.95.0 (59807616e 2026-04-14)
cargo --version。 cargo 1.95.0 (f2d3ce0bd 2026-03-21)
go version.      go version go1.26.3 darwin/arm64
```

---

## 2. 快速开始

### 2.1 克隆仓库

```bash
# 克隆主仓库（如已有 fork，将主仓库添加为 upstream）
git clone https://github.com/eastreams/loong.git
cd loong

# 或者从你的 fork 克隆
git clone https://github.com/<your-username>/loong.git
cd loong
```

### 2.2 安装依赖

```bash
# 安装 Rust 依赖（自动解析 workspace Cargo.toml）
cargo fetch

# 如果使用 task CLI 安装完整开发环境
# （包括 pre-commit hook 等）
cp scripts/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

### 2.3 首次构建

```bash
# 构建整个 workspace
cargo build --workspace

# 仅构建 daemon 二进制（主入口）
cargo build -p loong-daemon

# Release 构建
cargo build --workspace --release
```

### 2.4 运行测试

```bash
# 运行所有测试（workspace 默认特性）
./scripts/cargo-local-toolchain.sh test --workspace

# 运行所有测试 + 全部特性（CI 门禁标准）
./scripts/cargo-local-toolchain.sh test --workspace --all-features

# 仅测试特定 crate
./scripts/cargo-local-toolchain.sh test -p loong-kernel
./scripts/cargo-local-toolchain.sh test -p loong-app
```

### 2.5 验证门禁（提交前必跑）

```bash
# 格式化检查
./scripts/cargo-local-toolchain.sh fmt --all -- --check

# 严格 clippy 检查（CI 同等标准）
./scripts/cargo-local-toolchain.sh clippy --workspace --all-targets --all-features -- -D warnings

# 架构边界检查（防止循环依赖）
./scripts/check_architecture_boundaries.sh

# 依赖图检查
./scripts/check_dep_graph.sh
```

> **提示**：如果安装了 `task` CLI，可以直接运行 `task verify` 一次性执行上述所有检查。

---

## 3. 项目结构概览

Loong 是一个 8-crate Rust workspace，采用分层内核架构。

### 8 个核心 Crate

```
loong/
├── Cargo.toml          # workspace 定义文件
├── crates/
│   ├── contracts       # 核心契约层（leaf，无内部依赖）
│   │   └── 定义 kernel、protocol、bridge-runtime 的接口契约
│   ├── kernel         # 运行时内核（capability / policy / audit）
│   │   └── 所有执行路径必须经过 kernel 的能力策略审计
│   ├── protocol        # 协议层（独立 leaf）
│   │   └── 协议定义和编解码
│   ├── bridge-runtime  # 桥接运行时（依赖 contracts + kernel + protocol）
│   │   └── WASMtime 等运行时桥接
│   ├── spec            # 规格说明层
│   │   └── 依赖 contracts、kernel、protocol、bridge-runtime
│   ├── bench           # 基准测试
│   │   └── 依赖 contracts、kernel、spec
│   ├── app             # 应用层（provider / tool / channel / memory）
│   │   └── 依赖 contracts、kernel
│   └── daemon          # 主程序二进制入口
│       └── 依赖上述所有 crate
```

### 架构铁律

> **禁止循环依赖**。依赖链必须严格单向：
> ```
> contracts (leaf) ← kernel ← bridge-runtime / app / spec / bench / daemon
> protocol  (leaf) ← bridge-runtime / spec / daemon
> kernel    ← app / bridge-runtime / spec / daemon
> ```

### 关键目录

| 目录 | 用途 |
|------|------|
| `scripts/` | 构建、验证、测试脚本（CI 同等逻辑） |
| `docs/` | 架构设计文档、路线图、可靠性约定 |
| `examples/` | 示例代码 |
| `tests/` | 集成测试 |
| `site/` | Mintlify 公网文档（面向用户） |
| `patches/` | 临时 patch 过的第三方依赖 |

---

## 4. 常用命令速查

> 所有命令均在仓库根目录执行。推荐使用 `./scripts/cargo-local-toolchain.sh` 而非直接 `cargo`，因为它会解析正确的 rustc 路径并隔离测试目录。

### 格式化

```bash
# 检查格式是否合规
./scripts/cargo-local-toolchain.sh fmt --all -- --check

# 自动修复格式
./scripts/cargo-local-toolchain.sh fmt --all
```

### Clippy（严格lint）

```bash
# CI 门禁标准（-D warnings = 任何警告都报错）
./scripts/cargo-local-toolchain.sh clippy --workspace --all-targets --all-features -- -D warnings
```

### 构建

```bash
# Debug 构建（整个 workspace）
cargo build --workspace

# Release 构建
cargo build --workspace --release

# 仅构建特定 crate
cargo build -p loong-daemon
cargo build -p loong-kernel
```

### 测试

```bash
# workspace 默认特性
./scripts/cargo-local-toolchain.sh test --workspace

# 所有特性（包含可选 feature flag）
./scripts/cargo-local-toolchain.sh test --workspace --all-features

# 仅测特定 crate
./scripts/cargo-local-toolchain.sh test -p loong-app
```

### 验证（推荐在提交前跑）

```bash
# 方式一：task CLI（需安装 go-task）
task verify              # 基础验证
task verify:full        # 完整验证（含架构检查）

# 方式二：直接跑脚本
./scripts/cargo-local-toolchain.sh fmt --all -- --check
./scripts/cargo-local-toolchain.sh clippy --workspace --all-targets --all-features -- -D warnings
./scripts/cargo-local-toolchain.sh test --workspace
./scripts/cargo-local-toolchain.sh test --workspace --all-features
./scripts/check_architecture_boundaries.sh
./scripts/check_dep_graph.sh
```

### 架构边界检查

```bash
# 检查循环依赖和架构边界违规
./scripts/check_architecture_boundaries.sh
```

### 依赖许可审计

```bash
# 检查依赖许可、安全公告、来源
./scripts/cargo-deny-local.sh check advisories bans licenses sources
```

---

## 5. 开发工作流

### 标准贡献流程

```
1. Fork 仓库
       ↓
2. 从 dev 分支创建功能分支
       ↓
3. 编写代码（保持 commit 聚焦）
       ↓
4. 运行验证门禁（见上方常用命令）
       ↓
5. Commit（遵循 commit 规范）
       ↓
6. Push 到你的 fork
       ↓
7. 从 GitHub 发起 PR → 目标分支 dev
       ↓
8. 等待维护者 review
       ↓
9. 根据反馈修改 → 重复 4-8
       ↓
10. Merge 入 dev
```

### 详细步骤

#### 第1步：Fork 并添加 upstream

```bash
# 克隆你的 fork
git clone https://github.com/<your-username>/loong.git
cd loong

# 添加主仓库为 upstream
git remote add upstream https://github.com/eastreams/loong.git
```

#### 第2步：创建分支

```bash
# 确保从最新的 dev 分支开始
git checkout dev
git pull upstream dev

# 创建功能分支
git checkout -b feat/your-feature-name
# 或
git checkout -b fix/issue-description
```

#### 第3步：开发与提交

```bash
# 查看当前修改状态
git status

# 暂存并提交（保持每次 commit 聚焦于一个问题）
git add path/to/changed/file
git commit -m "feat(kernel): add capability audit log

- add audit trail for denied capabilities
- update policy.rs with new audit hook"
```

#### 第4步：验证

```bash
# 提交前必跑
task verify
# 或手动跑 CI 同等检查
./scripts/cargo-local-toolchain.sh fmt --all -- --check
./scripts/cargo-local-toolchain.sh clippy --workspace --all-targets --all-features -- -D warnings
./scripts/cargo-local-toolchain.sh test --workspace --all-features
```

#### 第5步：Push 并创建 PR

```bash
# 推送你的分支
git push origin feat/your-feature-name

# 在 GitHub 打开 PR
# 目标: eastreams/loong ← 你的 fork
# 目标分支: dev
```

#### 分支命名建议

| 类型 | 命名格式 | 示例 |
|------|----------|------|
| 新功能 | `feat/描述` | `feat/add-telegram-channel` |
| Bug 修复 | `fix/描述` | `fix/kernel-panic-on-empty-config` |
| 文档 | `docs/描述` | `docs/update-readme` |
| 重构 | `refactor/描述` | `refactor/extract-provider-trait` |

### Commit 规范

- 使用清晰、具体的 commit message
- 每条 commit 解决一个问题
- 参考 [Conventional Commits](https://www.conventionalcommits.org/)

---

## 6. 常见问题

### Q1: clippy 报错 `error: unused variable: xxx`

**原因**：代码中存在未使用的变量、函数或 import，clippy 的 `-D warnings` 会将这些作为错误拒绝。

**解决方法**：
```bash
# 查看具体报错
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | grep "unused"

# 常见修复：
# 1. 删除未使用的变量：把 `let x = ...` 下划线命名 `let _ = ...`
# 2. 删除未使用的 import：`use xxx;` 前加 `_` 或直接删除
# 3. 如果是故意预留的接口，添加 `#[allow(dead_code)]`
```

---

### Q2: 测试失败 `assertion failed` 或 `thread 'xxx' panicked`

**原因**：测试用例失败可能是因为：1) 你的修改破坏了既有行为；2) 测试依赖了特定环境（如 `LOONG_HOME` 路径）；3) 并发竞态。

**解决方法**：
```bash
# 确认失败的测试
./scripts/cargo-local-toolchain.sh test --workspace 2>&1 | grep -A5 "FAILED"

# 隔离运行失败的测试（不污染其他测试状态）
cargo test -p <crate-name> <test_name> -- --nocapture

# 如果是 LOONG_HOME 相关问题，检查脚本是否正确隔离
# cargo-local-toolchain.sh 会自动设置 LOONG_HOME 到 target/test-loong-home
# 确认你没有在环境变量中覆盖它
echo $LOONG_HOME  # 应为空或 target/test-loong-home
```

---

### Q3: 架构边界检查不通过（循环依赖报错）

**原因**：你新增的代码引入了模块间的循环依赖，违反了架构铁律。

**错误示例**：
```
error[E9999]: cyclic dependency detected:
  app → kernel → spec → app
```

**解决方法**：
1. 查看报错中指出的具体循环路径
2. 重构代码，将公共类型提取到 `contracts` crate（它是架构中的 leaf，无内部依赖）
3. 使用接口而非具体类型传递依赖
4. 参考 `docs/design-docs/layered-kernel-design.md` 理解分层边界

```bash
# 检查具体哪个依赖导致了循环
./scripts/check_dep_graph.sh

# 查看 contracts 层是否已经包含了你想共享的接口
ls crates/contracts/src/
```

---

### Q4: `task: command not found`

**原因**：`task` CLI（go-task）未安装，或安装了但不在 PATH 中。

**解决方法**：
```bash
# macOS
brew install go-task/tap/go-task

# Linux
go install github.com/go-task/task/v3/cmd/task@latest

# 手动验证
task --version
```

如果不想安装 `task`，直接使用 `scripts/` 下的脚本即可，所有功能都有对应的底层脚本。

---

### Q5: `cargo-local-toolchain.sh: No such file or directory`

**原因**：在错误的目录下执行了命令，或仓库未完整克隆（`scripts/` 目录缺失）。

**解决方法**：
```bash
# 确认你在仓库根目录
pwd
ls scripts/cargo-local-toolchain.sh

# 如果文件不存在，说明克隆不完整
git pull --recursive
# 或重新克隆
git clone --recurse-submodules https://github.com/eastreams/loong.git
```

---

### Q6: 提交 PR 后 CI 全部变红，但本地 `task verify` 通过了

**原因**：CI 环境与本地环境存在差异。常见情况：
- CI 使用特定 Rust 版本（见 `rust-toolchain.toml`）
- CI 运行 `--all-features` 但本地可能漏了某些 feature
- 缓存问题

**解决方法**：
```bash
# 确保 Rust 版本一致
rustup show  # 查看当前激活的 toolchain
cat rust-toolchain.toml  # 查看 CI 要求的版本

# 强制使用 rust-toolchain.toml 指定的版本
rustup override set $(cat rust-toolchain.toml | grep channel | cut -d'"' -f2)

# 清理缓存重新验证
cargo clean
task verify:full
```

---
