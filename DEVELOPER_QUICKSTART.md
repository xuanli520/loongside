# Loongside 开发者快速入门指南

欢迎！本文档将帮助你在 **30 分钟内** 完成环境搭建，并成功运行 Loongside 项目的第一个构建命令。

## 1. 环境要求

在开始之前，请确保你的电脑已安装以下工具：

| 工具 | 版本要求 | 安装/验证方式 |
| :--- | :--- | :--- |
| **Git** | 任意版本 | `git --version` |
| **Rust** | 1.70 或更高 | `rustc --version` |
| **Go** | 1.20 或更高 | `go version` |
| **cargo-deny** | 最新版 | `cargo install cargo-deny` |

### 安装步骤

#### 1.1 安装 Git
- 访问 https://git-scm.com/downloads
- 下载 Windows 版本，安装时全部选默认选项即可
> 💡 **为什么**：Git 用于从 GitHub 克隆代码和管理版本。

#### 1.2 安装 Rust（必须）
1. 访问 https://rustup.rs/
2. 下载 `rustup-init.exe`（Windows）或按提示运行命令（Mac/Linux）
3. 双击运行，**直接按回车**选择默认安装
4. 安装完成后，**关闭当前终端，重新打开一个新的终端**（让环境变量生效）
5. 验证安装：`rustc --version`（应显示版本号）
> 💡 **为什么**：Rust 是项目的核心编程语言，没有它什么都跑不了。

#### 1.3 安装 Go（必须）
1. 访问 https://go.dev/dl/
2. 下载 Windows 版本（如 `go1.22.0.windows-amd64.msi`）
3. 双击运行，按提示安装（全部选默认）
4. 安装完成后，**关闭当前终端，重新打开一个新的终端**
5. 验证安装：`go version`（应显示版本号）
> 💡 **为什么**：Go 被项目的 `task verify` 命令用来做代码规范检查（convention check）。

#### 1.4 安装 cargo-deny（Rust 依赖检查工具）
```bash
cargo install cargo-deny
```
> 💡 **为什么**：`cargo-deny` 用来检查项目的依赖库是否有安全漏洞或许可证问题，是 CI 检查的一部分。

## 2. 快速开始

请打开你的终端（VS Code 里按 `` Ctrl + ` ``），然后依次执行以下命令：

**2.1 克隆你的 Fork 仓库**
```bash
git clone https://github.com/[你的用户名]/Loongside.git
```
> 💡 **为什么**：把你 GitHub 上的仓库下载到本地电脑，才能开始修改代码。

**2.2 进入项目目录**
```bash
cd Loongside
```
> 💡 **为什么**：让终端定位到项目文件夹，后续命令才能正确执行。

**2.3 添加原仓库作为上游（upstream）**
```bash
git remote add upstream https://github.com/xuanli520/Loongside.git
```
> 💡 **为什么**：原仓库（师兄的仓库）会不断更新。添加 upstream 后，你可以随时拉取最新的代码，保持和官方同步。

**2.4 从上游拉取所有分支信息**
```bash
git fetch upstream
```
> 💡 **为什么**：获取原仓库的所有分支（比如 `dev` 分支）和提交记录，但不自动合并到你的代码里。

**2.5 基于上游的 `dev` 分支，在本地创建并切换到 `dev` 分支**
```bash
git checkout -b dev upstream/dev
```
> 💡 **为什么**：项目的开发主分支是 `dev`，不是 `main`。你需要切换到 `dev` 分支才能开始开发。

**2.6 运行首次构建**
```bash
cargo build
```
> 💡 **为什么**：Rust 项目需要编译才能运行。首次构建会自动下载所有依赖库，并生成可执行文件。

**2.7 运行测试，验证环境是否正常**
```bash
cargo test
```
> 💡 **为什么**：确保你的开发环境配置正确，项目本身也没有问题。如果测试全部通过，说明环境已经准备好了。

## 3. 项目结构概览

Loongside 是一个由 8 个 crate 组成的工作空间（workspace），各 crate 的职责简介如下：

| Crate | 路径 | 一句话职责 |
|-------|------|-------------|
| contracts | crates/contracts/ | 定义核心接口（trait）和数据结构 |
| kernel | crates/kernel/ | Agent 内核的核心实现 |
| protocol | crates/protocol/ | 定义 Agent 间通信协议 |
| app | crates/app/ | 应用层入口与编排逻辑 |
| spec | crates/spec/ | Spec 声明式任务编排引擎 |
| bench | crates/bench/ | 性能测试基准 |
| daemon | crates/daemon/ | 后台守护进程 |
| bridge-runtime | crates/bridge-runtime/ | 不同运行时之间的桥接 |

> 💡 更多细节请参考 [`ARCHITECTURE.md`](./ARCHITECTURE.md)。

## 4. 常用命令速查

在开发过程中，你通常会用到以下命令：

| 命令 | 用途 | 何时使用 |
|------|------|----------|
| `cargo fmt` | 自动格式化代码 | 提交代码前执行 |
| `cargo clippy` | 代码风格与常见错误检查 | 提交代码前执行 |
| `cargo test` | 运行所有单元测试和集成测试 | 验证修改是否破坏现有功能 |
| `cargo build` | 构建项目（调试模式） | 日常开发 |
| `cargo build --release` | 构建生产版本（优化） | 需要高性能时 |
| `task verify` | 完整验证流程（fmt + clippy + test） | 提交 PR 前完整检查 |

> 💡 `task` 命令需要安装 `task` CLI（可选），详见 [Taskfile 官网](https://taskfile.dev/)。

## 5. 开发工作流 (GitHub 协作标准流程)

本项目使用 **GitHub Flow** 进行协作，核心是所有的修改都基于 `dev` 分支。请严格遵循以下步骤：

1.  **Fork 主仓库**：在 GitHub 上将 [xuanli520/loongside](https://github.com/xuanli520/loongside) Fork 到你自己的账号下。
    > 💡 **为什么**：Fork 会在你的 GitHub 空间里创建一个项目副本。你没有原仓库的直接写入权限，需要在自己的副本上修改，然后通过 PR 请求合并。

2.  **克隆你自己的仓库到本地**：
    ```bash
    git clone https://github.com/你的用户名/Loongside.git
    cd Loongside
    ```
    > 💡 **为什么**：把 GitHub 上的代码下载到电脑上才能编辑。

3.  **同步上游仓库的 `dev` 分支**：
    ```bash
    git remote add upstream https://github.com/xuanli520/loongside.git
    git fetch upstream
    git checkout -b dev upstream/dev
    ```
    > 💡 **为什么**：新克隆的仓库可能没有 `dev` 分支。这三步把原仓库的 `dev` 分支同步到本地，确保你基于最新的代码开始开发。

4.  **创建你的功能分支**：
    ```bash
    git checkout -b feature/your-feature-name
    ```
    > 💡 **为什么**：功能分支把你的修改隔离开，不影响 `dev` 分支。即使你的代码写错了，也不会破坏项目的主分支。

5.  **编写代码**：在功能分支上进行你的修改。

6.  **本地验证**：
    ```bash
    cargo fmt          # 自动格式化代码，统一代码风格
    cargo clippy       # 检查代码风格和常见错误，避免潜在的 bug
    cargo test         # 运行所有测试，确保没有破坏现有功能
    ```
    > 💡 **为什么**：在 PR 之前跑这三个命令，可以让 CI 检查更容易通过，减少师兄帮你改格式的时间。

7.  **提交并推送**：
    ```bash
    git add .
    git commit -m "你的提交信息，例如：docs: update developer quickstart guide"
    git push origin feature/your-feature-name
    ```
    > 💡 **为什么**：`git push` 把你的代码上传到 GitHub 上的功能分支，这样师兄才能看到你的修改。

8.  **创建 Pull Request (PR)**：
    - 打开你的 GitHub 仓库页面，点击 **"Compare & pull request"** 绿色按钮。
    - 确保 **base repository** 是 `xuanli520/loongside`，**base** 分支是 `dev`。**head repository** 是你的仓库，**compare** 分支是你刚推送的功能分支。
    - 填写 PR 标题和描述，点击 **"Create pull request"**。
    > 💡 **为什么**：PR 是请求项目维护者将你的代码合并到主仓库的通道。没有 PR，师兄不知道你写了代码。

9.  **等待 Code Review**：根据师兄或维护者的反馈修改代码。只需继续在本地功能分支上修改、提交并推送，PR 会自动更新。

10. **PR 合并**：当 PR 被审查通过并被合并到主仓库的 `dev` 分支后，恭喜你，你的贡献就成功被项目收录了！

## 6. 常见问题 (FAQ)

### Q1: 执行 `cargo build` 时报错 `linker not found`
- **现象**：提示找不到 `link.exe`。
- **原因**：缺少 C++ 编译工具链，Rust 需要它来将代码编译成可执行文件。
- **解决**：安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)，安装时勾选 **"C++ 生成工具"**。

### Q2: 执行 `cargo test` 时部分测试失败，但我没改过代码
- **解决**：先执行 `cargo clean`，再重新运行 `cargo test`。这能清除旧的编译缓存，避免缓存导致的问题。如果仍然失败，请检查你的 Rust 版本是否为最新：`rustup update`。

### Q3: `cargo clippy` 报出大量与我无关的警告
- **解决**：确保你项目的依赖是最新的。运行 `cargo update`，然后再次运行 `cargo clippy`。如果警告依然存在，请参照提示进行修复。

### Q4: 执行 `git checkout dev` 时报错 `pathspec 'dev' did not match any file(s)`
- **原因**：你的本地仓库还没有 `dev` 分支，通常是因为你 Fork 的仓库没有自动同步原仓库（upstream）的所有分支。
- **解决**：
  ```bash
  git remote add upstream https://github.com/xuanli520/Loongside.git
  git fetch upstream
  git checkout -b dev upstream/dev
  git push origin dev
  ```

---

> 📚 **更多详细信息**：请参考 [`CONTRIBUTING.md`](./CONTRIBUTING.md)（贡献者指南）和 [`ARCHITECTURE.md`](./ARCHITECTURE.md)（架构文档）。

**祝你开发顺利！** 🚀