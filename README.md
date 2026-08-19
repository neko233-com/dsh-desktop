# DSH Desktop

DeepSeek 专用桌面端。给一个 DeepSeek API Key、选择模型，即可打开本地工作台；官方 DeepSeek Harness 负责执行时，本项目负责桌面体验、目标模式和生命周期。

项目是社区开源桌面端，不是 DeepSeek 官方产品。`frontend/dsh-web-ui` 是已经复制进本仓库的完整前端源码，之后由本仓库维护；运行时不会导入 GitHub 前端仓库、不会使用其远程更新器。官方 Harness 仍按其官方发布形态运行。

## 用户安装

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/neko233-com/dsh-desktop/main/scripts/install.ps1 | iex
```

指定镜像前缀。镜像应接收完整 GitHub URL，例如 `https://ghfast.top/`、`https://ghproxy.net/`：

```powershell
$env:DSH_DESKTOP_MIRROR = "https://ghfast.top/"
irm https://raw.githubusercontent.com/neko233-com/dsh-desktop/main/scripts/install.ps1 | iex
```

### macOS Apple Silicon

```sh
curl -fsSL https://raw.githubusercontent.com/neko233-com/dsh-desktop/main/scripts/install.sh | sh
```

指定镜像：

```sh
DSH_DESKTOP_MIRROR=https://ghfast.top/ curl -fsSL https://raw.githubusercontent.com/neko233-com/dsh-desktop/main/scripts/install.sh | sh
```

安装包已内置 Node.js 运行时（干净的 Node.js 24 LTS），用户不需要单独安装 Harness、Node.js 或 pnpm。首次启动会自动启用 pnpm，并通过 `npx --yes @deepseek-ai/dsh@latest web` 准备/更新官方 Harness；默认使用 `https://registry.npmmirror.com`，也可用 `DSH_NPM_REGISTRY` 切换官方源或其他镜像。内置 Web UI 源码和构建产物也随桌面安装包提供，不再从 `@linxin666/dsh-web-ui-all` 拉取。

当前发布只提供 Windows x64，使用 `scripts/package-windows.ps1` 在本机手动打包并通过 GitHub CLI 手动上传；不使用 GitHub Actions 消耗构建额度。

## 内置前端源码

`frontend/dsh-web-ui` 是参考项目的源码副本，不是 Git submodule，也不是运行时依赖。修改前端后，在该目录执行：

```sh
pnpm install --config.minimumReleaseAge=0
pnpm build
```

桌面端首次启动会把这些本地包链接到 DSH 的 web profile，并保留官方 Harness 的依赖；不会逐包反复安装，避免破坏 profile。前端自更新入口已从桌面发行版本移除，后续更新由本仓库自己的 Release 管理。

## 原生目标模式

目标模式不另造状态机：桌面端直接调用 DSH 原生 `/goal` 命令，因此目标的创建、轮次、暂停、恢复、完成、阻塞、持久化和 Agent 工具链都由官方 Harness 的 `goal` / `goal-round-driver` / `tool-goal` 负责。

- 点击工作台右下角「目标模式」：输入框进入 `/goal `。
- 快捷键：Windows `Ctrl+Shift+G`；macOS `Cmd+Shift+G`。
- 输入目标并回车：沿用 DSH 原生 Goal Bar 与目标生命周期。

## 桌面宠物

内置独立绘制的蓝发女仆 Q 版宠物帧动画，不依赖网络资源：默认显示，点击角色可直接进入目标模式，悬停后可隐藏；隐藏状态由 Rust 写入系统应用数据目录，重启后保持。宠物会根据 Goal Bar 文案切换待机、专注和完成反馈。

## 无边框原生窗口

桌面窗口不使用系统标题栏，右上角提供易懂的 Windows 风格控制：`—` 最小化、`□` 最大化、`×` 关闭；「设置」中提供 API Key 重设和宠物显示/隐藏。设置页的「获取 API Key」会调用本机默认浏览器打开 `https://platform.deepseek.com/api_keys`。

Windows 下 DSH/npx/pnpm 子进程默认不显示控制台窗口；关闭或重试时会回收完整进程树，避免残留 Node 进程占用 Web UI 锁。

## 开发

```sh
cargo run
cargo fmt --all
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

环境变量：

| 变量 | 用途 |
| --- | --- |
| `DSH_DESKTOP_DSH_BIN` | 指定 `dsh` 可执行文件或脚本 |
| `DSH_DESKTOP_NODE_BIN` | 指定内置/自定义 `npx`，仅用于调试或替换运行时 |
| `DSH_DESKTOP_WORKSPACE` | DSH 默认工作目录 |
| `DSH_DESKTOP_PORT` | 固定本地端口；默认从 `3080` 开始自动避让 |
| `DSH_NPM_REGISTRY` | DSH/npx/pnpm 使用的 npm 镜像；默认 `https://registry.npmmirror.com` |
| `DEEPSEEK_API_KEY` | 启动时临时覆盖钥匙串，适合 CI/调试 |
| `DEEPSEEK_BASE_URL` | 可选 API 地址，默认 `https://api.deepseek.com` |
| `DEEPSEEK_MODEL` | 可选模型，默认 `deepseek-v4-flash`；设置页也可选择并记忆 |

### Key 安全边界

- 用户输入的 Key 使用 Windows Credential Manager / macOS Keychain 保存。
- DSH 子进程通过环境变量接收 Key；配置文件不保存 Key。
- stdout/stderr 写入本地 `dsh.log` 前会脱敏。
- 服务只绑定 `127.0.0.1`，不自动暴露局域网端口。

### 本机全自动验收

Token 只放在当前 PowerShell 会话，不要写入脚本：

```powershell
$env:DEEPSEEK_API_KEY = "在当前会话临时设置"
.\scripts\validate-local.ps1
Remove-Item Env:DEEPSEEK_API_KEY
```

脚本会先调用指定模型（默认 `deepseek-v4-flash`），再编译并覆盖本机安装包，启动桌面端，探测本地 DSH 服务，最后自动停止测试进程。

## 架构取舍

`rdesktop` 是传统 RDP 客户端，`IronRDP` 是 Rust RDP 协议实现；它们适合连接远程 Windows 桌面，不适合承载 DSH 的本地 HTTP Web UI。本项目使用 Rust + Wry + WebView2/WebKit，把官方 DeepSeek Harness 本地服务封装成轻量原生窗口；前端源码已复制到本仓库，不依赖上游前端仓库。

产品能力边界：

```text
DSH Desktop
  ├─ API Key onboarding → OS Keychain
  ├─ dsh / npx discovery → start official DeepSeek Harness
  ├─ local process supervisor → 127.0.0.1:port
  ├─ local vendored frontend → DSH Web UI / DSH Web
  ├─ native goal shortcut → DSH `/goal` command
  └─ local pet overlay → embedded 4-frame mascot sprite
```

## 许可

本项目 MIT。上游 DeepSeek Harness 遵循其自身 MIT 许可；`frontend/dsh-web-ui` 保留参考项目及其各子包的许可证文件，发布时随本仓库源码分发。
