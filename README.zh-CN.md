# Loong

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/logo/loong-logo-dark.png" />
    <source media="(prefers-color-scheme: light)" srcset="./assets/logo/loong-logo-light.png" />
    <img src="./assets/logo/loong-logo-light.png" alt="Loong" width="280" />
  </picture>
</p>
<p align="center"><strong><em>"发轫于东，以会群友"</em></strong></p>

<p align="center">
  <a href="https://github.com/eastreams/loong/actions/workflows/ci.yml?branch=dev"><img src="https://img.shields.io/github/actions/workflow/status/eastreams/loong/ci.yml?branch=dev&label=build&style=flat-square" alt="Build" /></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License: MIT" /></a>
  <img src="https://img.shields.io/badge/rust-edition%202024-orange.svg?style=flat-square" alt="Rust Edition 2024" />
  <a href="https://github.com/eastreams/loong/releases"><img src="https://img.shields.io/github/v/release/eastreams/loong?label=version&color=yellow&include_prereleases&style=flat-square" alt="Version" /></a>
  <br/>
  <a href="https://x.com/loongclawai"><img src="https://img.shields.io/badge/Follow-loongclawai-000000?logo=x&logoColor=white&style=flat-square" alt="X" /></a>
  <a href="https://t.me/loongclaw"><img src="https://img.shields.io/badge/Telegram-loongclaw-26A5E4?logo=telegram&logoColor=white&style=flat-square" alt="Telegram" /></a>
  <a href="https://discord.gg/7kSTX9mca"><img src="https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white&style=flat-square" alt="Discord" /></a>
  <a href="https://www.reddit.com/r/LoongClaw"><img src="https://img.shields.io/badge/Reddit-r%2Floongclaw-FF4500?logo=reddit&logoColor=white&style=flat-square" alt="Reddit" /></a>
  <br/>
  <a href="https://xhslink.com/m/1dqFqF1IKDk"><img src="https://img.shields.io/badge/Xiaohongshu-follow-FF2442?logo=xiaohongshu&logoColor=white&style=flat-square" alt="Xiaohongshu" /></a>
  <a href="https://loongclaw.ai/feishu.jpg"><img src="https://img.shields.io/badge/Feishu-QR-3370FF?logo=lark&logoColor=white&style=flat-square" alt="Feishu QR" /></a>
  <a href="https://loongclaw.ai/wechat.jpg"><img src="https://img.shields.io/badge/WeChat-QR-07C160?logo=wechat&logoColor=white&style=flat-square" alt="WeChat QR" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

---

Loong 是一套基于 Rust 构建的分层智能体操作系统内核，为垂域 AI 智能体提供安全、受治理的运行时——承载长程工作流构建、复合任务执行与闭环改进，让人与 AI 在真实场景中持续协作。

与简单的 LLM 封装不同，Loong 将契约、安全、执行、编排分离到各自独立的层级，边界明确。每一次操作都经过能力令牌门控的策略引擎和审计链路。扩展（provider、工具、频道、记忆、插件）全部位于内核之外，无需修改核心即可组合。

<p align="center">
  <a href="site/index.mdx">文档总览</a> •
  <a href="site/get-started/overview.mdx">快速上手</a> •
  <a href="site/use-loong/configuration-patterns.mdx">配置模式</a> •
  <a href="site/use-loong/common-setups.mdx">常见路线</a> •
  <a href="site/build-on-loong/overview.mdx">扩展 Loong</a> •
  <a href="CONTRIBUTING.md">参与贡献</a>
</p>

<a id="why-loong"></a>
## 为什么选 Loong

**核心能力开箱即用，可观察、可操作、可扩展：**

- **42+ 内置 provider** — OpenAI、Anthropic、火山引擎、DeepSeek、Gemini、本地模型等，内置故障转移与限流。
- **25+ 接入频道** — Telegram、飞书/Lark、Discord、Slack、微信、企业微信、Matrix、WhatsApp、邮件、IRC、Nostr、Teams、iMessage、Twitch 等。
- **受治理的执行** — 每次工具调用都经过内核策略引擎，配合能力令牌、审计链路和人工审批门控。
- **WASM 插件沙箱** — 通过 Wasmtime 运行不受信扩展，策略驱动的资源限制。
- **编程式编排** — 重试、熔断、自适应并发、优先级调度、速率整形，支撑复合工作流。
- **60+ CLI 子命令** — `audit`、`tasks`、`skills`、`plugins`、`channels`、`runtime-snapshot`、`gateway`、`doctor` 等。

**无论你是新手还是极客，都适合你：**

- **易于上手** — `loong onboard` 即写入可用配置；兼容 OpenClaw、Claude Code、Codex、OpenCode 等已有配置。
- **边界透明** — 助手、网关、接入频道各自独立，互不纠缠。
- **内核与扩展分离** — provider、工具、频道、记忆、策略独立于内核，按需编译组合。
- **不是玩具** — 面向长期使用设计，能跟着你的需求一起成长。

完整的公开定位与产品立场，见 [Loong 的缘起与定位](site/reference/why-loong.mdx)。

## 赞助商

<p align="center">
  <a href="https://www.volcengine.com/activity/codingplan?utm_campaign=loong&utm_content=loong&utm_medium=devrel&utm_source=OWO&utm_term=loong">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="assets/sponsors_logo/volcengine/volcengine-logo-dark-zh.png"/>
      <img src="assets/sponsors_logo/volcengine/volcengine-logo-light-zh.png" alt="火山引擎" height="44"/>
    </picture>
  </a>
  <span>&emsp;&emsp;&emsp;</span>
  <a href="https://www.feishu.cn">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="assets/sponsors_logo/feishu/feishu-logo-dark-zh.png"/>
      <img src="assets/sponsors_logo/feishu/feishu-logo-light-zh.png" alt="飞书" height="44"/>
    </picture>
  </a>
</p>

<a id="quick-start"></a>
## 快速开始

> Loong 当前只支持 `loong` 这个命令行入口。

### 脚本安装（推荐）

Linux 或 macOS：

```bash
curl -fsSL https://raw.githubusercontent.com/eastreams/loong/dev/scripts/install.sh | bash -s -- --onboard
```

Windows PowerShell：

```powershell
$script = Join-Path $env:TEMP "loong-install.ps1"
Invoke-WebRequest https://raw.githubusercontent.com/eastreams/loong/dev/scripts/install.ps1 -OutFile $script
pwsh $script -Onboard
```

### 从源码安装

确保系统有 C 链接器（Rust 编译需要）：

```bash
# Debian / Ubuntu
sudo apt update && sudo apt install build-essential

# Fedora
sudo dnf groupinstall "Development Tools"

# macOS
xcode-select --install
```

安装 Rust 工具链（已安装可跳过）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

构建并安装：

```bash
bash scripts/install.sh --source --onboard
```

```bash
# 或者只通过 Cargo 安装（不含 onboard 引导）
cargo install --path crates/daemon
```

### 首次运行

```bash
loong onboard                # 交互式初始化，配置 provider 和 model
loong ask --message "用一句话总结这个仓库"  # 单轮提问，验证配置
loong chat                   # 进入多轮对话
loong doctor --fix           # 检查环境并自动修复常见问题
loong update                 # 升级到最新稳定版
```

### 配置

`loong onboard` 会把可用配置写到 `~/.loong/config.toml`。手动添加 provider 或频道：

```toml
active_provider = "openai"

[providers.openai]
kind = "openai"
api_key = { env = "OPENAI_API_KEY" }
model = "auto"

[providers.volcengine]
kind = "volcengine"
api_key = { env = "ARK_API_KEY" }
model = "auto"
```

接入频道示例（飞书）：

```bash
loong feishu onboard         # 终端二维码流程，自动创建 bot app
```

完整的 provider / channel 矩阵，见下方[文档](#documentation)表。

<a id="documentation"></a>
## 文档

| | |
| --- | --- |
| 快速上手 | [Get Started](site/get-started/overview.mdx)，或直接用 `onboard` / `ask` / `chat` / `doctor` |
| 完整路径 | [Common Setups](site/use-loong/common-setups.mdx) |
| 选 Provider | [Provider Guides](site/use-loong/provider-guides/index.mdx) 与 [Provider Recipes](site/use-loong/provider-recipes.mdx) |
| 接入频道 | [Channel Guides](site/use-loong/channel-guides/index.mdx) 与 [Channel Recipes](site/use-loong/channel-recipes.mdx) |
| 长期托管 | [Gateway 与监督](site/use-loong/gateway-and-supervision.mdx) |
| 设计立场 | [Why Loong](site/reference/why-loong.mdx) |
| 架构与扩展 | [Build On Loong](site/build-on-loong/overview.mdx) |
| 参考资料 | [Reference](site/reference/overview.mdx) |

<a id="architecture"></a>
## 架构

Loong 是一个 8-crate Rust workspace，依赖图严格无环，围绕受治理的内核组织，将契约、安全、执行、编排分离。

```text
contracts        (稳定契约词汇表 — 零内部依赖)
├── kernel          -> contracts
├── protocol        (独立传输基础)
├── bridge-runtime  -> contracts, kernel, protocol
├── app             -> contracts, kernel
├── spec            -> contracts, kernel, protocol, bridge-runtime
├── bench           -> kernel, spec
└── daemon          -> 以上全部
```

运行时按 L0–L9 分层组织：

| 层级 | 职责 |
|------|------|
| L0 | 契约词汇（稳定 ABI，向后兼容） |
| L1 | 安全与治理（策略引擎、能力令牌、审批门控） |
| L2 | 执行平面（Runtime / Tool / Memory / Connector） |
| L3 | 编排（harness 路由、pack 边界） |
| L4 | 可观测性（审计时间线、确定性时钟） |
| L5 | 垂域 Pack（通过 manifest 打包领域能力） |
| L6 | 集成控制（自主供给、热插拔） |
| L7 | 插件翻译（多语言 IR、bridge-kind 推断） |
| L8 | 自感知（架构守卫、不可变核心保护） |
| L9 | 引导（插件激活生命周期） |

ownership 分区与设计原则，见 [ARCHITECTURE.md](ARCHITECTURE.md)。

## 安全

- 全 workspace `#![forbid(unsafe_code)]`
- 基于类型系统的能力令牌，支持代际撤销
- 每次内核工具调用都经过策略引擎门控
- 插件安全扫描，高风险自动阻断
- 外部 profile 完整性校验（checksum + ed25519 签名）
- WASM 沙箱，策略驱动的资源限制
- SSRF 防护（禁代理、私有主机阻断）
- 持久化 JSONL 审计链路，支持 SIEM 导出

详见 [SECURITY.md](SECURITY.md)。

## 平台支持

| 目标平台 | 状态 |
|---------|------|
| Linux x86_64 (gnu) | 已支持 |
| Linux x86_64 (musl) | 已支持 |
| Linux aarch64 | 已支持 |
| Android aarch64 | 已支持 |
| macOS x86_64 | 已支持 |
| macOS aarch64 (Apple Silicon) | 已支持 |
| Windows x86_64 | 已支持 |

## 贡献

欢迎贡献。先从 [CONTRIBUTING.md](CONTRIBUTING.md) 开始。

如果你想先看哪些方向最值得补强，可以读 [Contribution Areas](site/build-on-loong/contribution-areas.mdx)。

## Star History

<p align="center">
  <a href="https://star-history.com/#eastreams/loong&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=eastreams/loong&type=Date&theme=dark"/>
      <img src="https://api.star-history.com/svg?repos=eastreams/loong&type=Date" alt="Star History Chart"/>
    </picture>
  </a>
</p>
