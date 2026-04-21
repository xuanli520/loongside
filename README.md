# Loong

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/logo/loong-logo-dark.png" />
    <source media="(prefers-color-scheme: light)" srcset="./assets/logo/loong-logo-light.png" />
    <img src="./assets/logo/loong-logo-light.png" alt="Loong" width="280" />
  </picture>
</p>
<p align="center"><strong><em>"Originated from the East, here to benefit the world"</em></strong></p>

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

Loong is a layered Agentic OS kernel built in Rust. It provides a secure, governed runtime for vertical AI agents — supporting long-horizon workflow construction, compound task execution, and closed-loop improvement across real-world scenarios.

Unlike simple LLM wrappers, Loong separates contracts, security, execution, and orchestration into distinct layers with explicit boundaries. Every operation routes through capability-gated policy and audit. Extensions (providers, tools, channels, memory, plugins) live outside the kernel and compose without core mutation.

<p align="center">
  <a href="site/index.mdx">Documentation</a> •
  <a href="site/get-started/overview.mdx">Get Started</a> •
  <a href="site/use-loong/configuration-patterns.mdx">Configuration</a> •
  <a href="site/use-loong/common-setups.mdx">Playbooks</a> •
  <a href="site/build-on-loong/overview.mdx">Build On Loong</a> •
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<a id="why-loong"></a>
## Why Loong

**Core capabilities, ready to inspect, operate, and extend:**

- **42+ built-in providers** — OpenAI, Anthropic, Volcengine, DeepSeek, Gemini, local models, and more. Failover and rate limiting included.
- **25+ channel adapters** — Telegram, Feishu/Lark, Discord, Slack, WeChat, WeCom, Matrix, WhatsApp, Email, IRC, Nostr, Teams, iMessage, Twitch, and others.
- **Governed execution** — every tool call passes through the kernel's policy engine with capability tokens, audit trail, and human approval gates.
- **WASM plugin sandbox** — run untrusted extensions in Wasmtime with policy-driven resource limits.
- **Programmatic orchestration** — retry, circuit-breaker, adaptive concurrency, priority scheduling, and rate shaping for compound workflows.
- **60+ CLI subcommands** — `audit`, `tasks`, `skills`, `plugins`, `channels`, `runtime-snapshot`, `gateway`, `doctor`, and more.

**Fits beginners and power users alike:**

- **Easy to start** — `loong onboard` writes a working config; compatible with existing OpenClaw, Claude Code, Codex, and OpenCode configurations.
- **Transparent boundaries** — assistant, gateway, and channels operate independently.
- **Core and extensions are separate** — providers, tools, channels, memory, and policy live outside the kernel. Compile and compose as needed.
- **Not a toy** — designed for long-term use, grows with your needs.

For the full public rationale, read [Why Loong](site/reference/why-loong.mdx).

## Sponsors

<p align="center">
  <a href="https://www.byteplus.com/en/activity/codingplan?utm_campaign=loong&utm_content=loong&utm_medium=devrel&utm_source=OWO&utm_term=loong">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="./assets/sponsors_logo/volcengine/volcengine-logo-dark-en.png"/>
      <img src="assets/sponsors_logo/volcengine/volcengine-logo-light-en.png" alt="Volcengine" height="44"/>
    </picture>
  </a>
  <span>&emsp;&emsp;&emsp;</span>
  <a href="https://www.feishu.cn">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="assets/sponsors_logo/feishu/feishu-logo-dark-en.png"/>
      <img src="assets/sponsors_logo/feishu/feishu-logo-light-en.png" alt="Feishu" height="44"/>
    </picture>
  </a>
</p>

<a id="quick-start"></a>
## Quick Start

> Loong uses `loong` as the only command-line entrypoint.

### Script Install (Recommended)

Linux or macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/eastreams/loong/dev/scripts/install.sh | bash -s -- --onboard
```

Windows PowerShell:

```powershell
$script = Join-Path $env:TEMP "loong-install.ps1"
Invoke-WebRequest https://raw.githubusercontent.com/eastreams/loong/dev/scripts/install.ps1 -OutFile $script
pwsh $script -Onboard
```

### From Source

Ensure your system has a C linker (required by Rust):

```bash
# Debian / Ubuntu
sudo apt update && sudo apt install build-essential

# Fedora
sudo dnf groupinstall "Development Tools"

# macOS
xcode-select --install
```

Install the Rust toolchain (skip if already installed):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Build and install:

```bash
bash scripts/install.sh --source --onboard
```

```bash
# Or install via Cargo only (without onboard setup)
cargo install --path crates/daemon
```

### First Run

```bash
loong onboard                # Interactive setup — configure provider and model
loong ask --message "Summarize this repo in one sentence."  # Verify config
loong chat                   # Multi-turn conversation
loong doctor --fix           # Check environment and auto-fix issues
loong update                 # Upgrade to latest stable release
```

### Configuration

`loong onboard` writes a working config to `~/.loong/config.toml`. To add providers or channels manually:

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

Channel example (Lark):

```bash
loong feishu onboard --domain lark   # QR-code flow, auto-creates bot app
```

For the full provider and channel matrices, see [Documentation](#documentation).

<a id="documentation"></a>
## Documentation

| | |
| --- | --- |
| Get started | [Get Started](site/get-started/overview.mdx), or just run `onboard` / `ask` / `chat` / `doctor` |
| Full rollout path | [Common Setups](site/use-loong/common-setups.mdx) |
| Pick a provider | [Provider Guides](site/use-loong/provider-guides/index.mdx) and [Provider Recipes](site/use-loong/provider-recipes.mdx) |
| Wire up channels | [Channel Guides](site/use-loong/channel-guides/index.mdx) and [Channel Recipes](site/use-loong/channel-recipes.mdx) |
| Long-running delivery | [Gateway And Supervision](site/use-loong/gateway-and-supervision.mdx) |
| Design stance | [Why Loong](site/reference/why-loong.mdx) |
| Architecture and extension | [Build On Loong](site/build-on-loong/overview.mdx) |
| Reference | [Reference](site/reference/overview.mdx) |

<a id="architecture"></a>
## Architecture

Loong is an 8-crate Rust workspace with a strict acyclic dependency graph, organized around a governed kernel that separates contracts, security, execution, and orchestration.

```text
contracts        (stable contract vocabulary — zero internal deps)
├── kernel          -> contracts
├── protocol        (independent transport foundation)
├── bridge-runtime  -> contracts, kernel, protocol
├── app             -> contracts, kernel
├── spec            -> contracts, kernel, protocol, bridge-runtime
├── bench           -> kernel, spec
└── daemon          -> all of the above
```

The runtime is organized into layers L0–L9:

| Layer | Responsibility |
|-------|---------------|
| L0 | Contract vocabulary (stable ABI, backward-compatible) |
| L1 | Security & governance (policy engine, capability tokens, approval gates) |
| L2 | Execution planes (Runtime / Tool / Memory / Connector) |
| L3 | Orchestration (harness routing, pack boundaries) |
| L4 | Observability (audit timeline, deterministic clocking) |
| L5 | Vertical packs (domain packaging via manifests) |
| L6 | Integration control (autonomous provisioning, hotplug) |
| L7 | Plugin translation (multi-language IR, bridge-kind inference) |
| L8 | Self-awareness (architecture guard, immutable-core protection) |
| L9 | Bootstrap (plugin activation lifecycle) |

For ownership zones and design principles, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Security

- `#![forbid(unsafe_code)]` across the workspace
- Capability-based access with type-system tokens and generation-based revocation
- Policy engine gate on every kernel-bound tool call
- Plugin security scan with `block_on_high`
- External profile integrity (checksum + ed25519 signature verification)
- WASM sandbox with policy-driven resource limits
- SSRF guardrails (no-proxy, private-host blocking)
- Durable JSONL audit trail with SIEM export

See [SECURITY.md](SECURITY.md) for the full model.

## Platform Support

| Target | Status |
|--------|--------|
| Linux x86_64 (gnu) | Supported |
| Linux x86_64 (musl) | Supported |
| Linux aarch64 | Supported |
| Android aarch64 | Supported |
| macOS x86_64 | Supported |
| macOS aarch64 (Apple Silicon) | Supported |
| Windows x86_64 | Supported |

<a id="contributing"></a>
## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md).

If you want to help where it matters most right now, read [Contribution Areas](site/build-on-loong/contribution-areas.mdx).

## Star History

<p align="center">
  <a href="https://star-history.com/#eastreams/loong&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=eastreams/loong&type=Date&theme=dark"/>
      <img src="https://api.star-history.com/svg?repos=eastreams/loong&type=Date" alt="Star History Chart"/>
    </picture>
  </a>
</p>
