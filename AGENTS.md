# DSH Desktop Agent Contract

这是桌面产品，不是只改网页的仓库。所有 Agent 修改必须保持以下硬约束：

## 强制产品设置

- API Key 未完成配置时，应用必须停留在设置页，禁止进入 DSH 工作台。
- API Key 只能进入系统钥匙串和 DSH 子进程环境变量，不得写入项目文件、提交、URL 或日志。
- 桌面宠物默认显示，隐藏状态必须跨重启记忆；修改宠物交互时必须保留右上角「设置」入口。

## 强制桌面体验

- 使用无系统装饰的原生窗口；窗口控制必须由 `assets/window-chrome.js` 提供并保持在右上角，使用 Windows 语义清晰的 `— / □ / ×`，不得改成苹果交通灯。
- 「获取 API Key」必须通过 Rust IPC 调用本机默认浏览器打开 DeepSeek API Key 页面，不得嵌入第三方登录页。
- UI/桌面壳改动必须同时检查 Windows x64、macOS arm64、任务栏/Dock 图标和安装包。
- 修改角色或图标后，必须运行 `python scripts/generate-icons.py`，禁止只替换单个尺寸。
- 不得把远程 RDP 客户端当作本地 DSH Web UI 容器；Rust 桌面壳统一走 Wry/WebView2/WebKit。

## 强制验证

提交前必须通过：

```text
python scripts/verify-desktop-contract.py
.\scripts\validate-local.ps1  # 当前 PowerShell 会话需先设置 DEEPSEEK_API_KEY
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
node --check assets/window-chrome.js
node --check assets/goal-mode.js
node --check assets/pet-mode.js
```

如果产品行为需要改变这些约束，先更新本文件、验证脚本和 README，再改实现。
