# DSH Desktop

DeepSeek 专用桌面端。给一个 DeepSeek API Key，自动准备 DSH、安装 `dsh-web-ui` 全家桶，打开本地 Web 工作台。

项目是社区开源壳，不是 DeepSeek 官方产品。核心 Harness 与参考 Web UI 仍由上游提供；本项目负责桌面体验、生命周期、密钥边界、安装和发布。

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

首次启动需要 Node.js 22+，因为 DSH 官方发布形态通过 `npx @deepseek-ai/dsh web` 启动。应用会优先使用 PATH 中的 `dsh`，找不到时自动回退到 `npx --yes @deepseek-ai/dsh`。

## 原生目标模式

目标模式不另造状态机：桌面端直接调用 DSH 原生 `/goal` 命令，因此目标的创建、轮次、暂停、恢复、完成、阻塞、持久化和 Agent 工具链都由上游 `goal` / `goal-round-driver` / `tool-goal` 负责。

- 点击工作台右下角「目标模式」：输入框进入 `/goal `。
- 快捷键：Windows `Ctrl+Shift+G`；macOS `Cmd+Shift+G`。
- 输入目标并回车：沿用 DSH 原生 Goal Bar 与目标生命周期。

## 桌面宠物

内置独立绘制的蓝发女仆 Q 版宠物帧动画，不依赖网络资源：默认显示，点击角色可直接进入目标模式，悬停后可隐藏；隐藏状态由 Rust 写入系统应用数据目录，重启后保持。宠物会根据 Goal Bar 文案切换待机、专注和完成反馈。

## 无边框原生窗口

桌面窗口不使用 Windows 系统标题栏，右上角提供苹果式三色控制：红色关闭、黄色最小化、绿色最大化；「设置」中提供 API Key 重设和宠物显示/隐藏。设置页的「获取 API Key」会调用本机默认浏览器打开 `https://platform.deepseek.com/api_keys`。

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
| `DSH_DESKTOP_WORKSPACE` | DSH 默认工作目录 |
| `DSH_DESKTOP_PORT` | 固定本地端口；默认从 `3080` 开始自动避让 |
| `DSH_NPM_REGISTRY` | DSH/npx 使用的 npm 镜像 |
| `DEEPSEEK_API_KEY` | 启动时临时覆盖钥匙串，适合 CI/调试 |
| `DEEPSEEK_BASE_URL` | 可选 API 地址，默认 `https://api.deepseek.com` |
| `DEEPSEEK_MODEL` | 可选模型，默认 `deepseek-v4-pro` |

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

`rdesktop` 是传统 RDP 客户端，`IronRDP` 是 Rust RDP 协议实现；它们适合连接远程 Windows 桌面，不适合承载 DSH 的本地 HTTP Web UI。本项目使用 Rust + Wry + WebView2/WebKit，把 DSH 本地服务封装成轻量原生窗口，不复制上游 UI 代码，也不修改上游 Harness。

产品能力边界：

```text
DSH Desktop
  ├─ API Key onboarding → OS Keychain
  ├─ dsh / npx discovery → install DSH Web UI plugin
  ├─ local process supervisor → 127.0.0.1:port
  ├─ native WebView → dsh-web-ui / DSH Web
  ├─ native goal shortcut → DSH `/goal` command
  └─ local pet overlay → embedded 4-frame mascot sprite
```

## 许可

本项目 MIT。上游 DeepSeek Harness 遵循其自身 MIT 许可；`dsh-web-ui` 及其皮肤/插件按上游仓库各自许可发布，运行时从 npm 安装。
