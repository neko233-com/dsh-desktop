#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env,
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use directories::ProjectDirs;
use serde::Deserialize;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Icon, Window, WindowId},
};
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

const APP_NAME: &str = "DSH Desktop";
const SERVICE_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3080;
const DEFAULT_MODEL: &str = "deepseek-v4-flash";
const DEFAULT_NPM_REGISTRY: &str = "https://registry.npmmirror.com";
const HARNESS_PACKAGE: &str = "@deepseek-ai/dsh@latest";
const PNPM_BUILD_PACKAGES: &[&str] = &["cpu-features", "node-pty", "ssh2"];
const LOCAL_UI_RUNTIME_DEPS: &[&str] = &[
    "@xterm/addon-fit@^0.11.0",
    "@xterm/xterm@^6.0.0",
    "cloudflared@^0.7.3",
    "clsx@^2.1.1",
    "dsh-better-sidebar@0.13.0",
    "lightningcss@^1.32.0",
    "qrcode.react@^4.2.0",
    "schemastery@^3.18.0",
    "ssh2@^1.17.0",
    "ws@^8.18.0",
    "yaml@^2.8.2",
    "zod@^4.4.3",
];
const LOCAL_UI_PACKAGE_PATHS: &[&str] = &[
    "packages/dsh-aionui-panel",
    "packages/dsh-community-plugins",
    "packages/dsh-git-graph",
    "packages/dsh-liangshen",
    "packages/dsh-pet",
    "packages/dsh-plugin-manager",
    "packages/dsh-remote-web-ui",
    "packages/dsh-skill-explorer",
    "packages/dsh-skins",
    "packages/dsh-ssh",
    "packages/dsh-task-board",
    "packages/dsh-tool-describe-image",
    "packages/dsh-web-ui-settings",
    "packages/skins/skin-center",
    "packages/dsh-web-ui-all",
];
const KEYRING_SERVICE: &str = "dsh-desktop";
const KEYRING_USER: &str = "deepseek-api-key";
const START_HTML: &str = include_str!("../assets/start.html");
const WINDOW_CHROME_SCRIPT: &str = include_str!("../assets/window-chrome.js");
const GOAL_MODE_SCRIPT: &str = include_str!("../assets/goal-mode.js");
const PET_MODE_SCRIPT: &str = include_str!("../assets/pet-mode.js");
const PET_SPRITE: &[u8] = include_bytes!("../assets/pet/maid-sprite-final.png");
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icons/icon-master.png");

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
enum UserEvent {
    SaveApiKey { key: String, model: String },
    ResetApiKey,
    SetPetVisibility(bool),
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    WindowDrag,
    RetryRuntime,
    ProcessStarted(Child),
    ServiceReady(String),
    StartupFailed(String),
}

#[derive(Debug, Deserialize)]
struct IpcMessage {
    #[serde(rename = "type")]
    kind: String,
    key: Option<String>,
    model: Option<String>,
    hidden: Option<bool>,
}

struct AppState {
    window: Option<Window>,
    webview: Option<WebView>,
    child: Option<Child>,
    proxy: EventLoopProxy<UserEvent>,
    data_dir: PathBuf,
    current_url: Option<String>,
    started: bool,
    pet_hidden: bool,
}

impl AppState {
    fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            webview: None,
            child: None,
            proxy,
            data_dir: app_data_dir(),
            current_url: None,
            started: false,
            pet_hidden: load_pet_hidden(&app_data_dir()),
        }
    }

    fn start_runtime(&mut self, key: String, model: String) {
        if self.started {
            return;
        }
        self.started = true;
        self.set_status("正在准备 DeepSeek 环境…", "正在加载本机前端与官方 Harness");

        let proxy = self.proxy.clone();
        let data_dir = self.data_dir.clone();
        let workspace = configured_workspace();
        let frontend_dir = configured_frontend_dir();
        thread::spawn(move || {
            if let Err(error) =
                start_runtime_worker(proxy.clone(), data_dir, workspace, frontend_dir, key, model)
            {
                let _ = proxy.send_event(UserEvent::StartupFailed(error));
            }
        });
    }

    fn set_status(&self, title: &str, detail: &str) {
        if let Some(webview) = &self.webview {
            let title = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".to_string());
            let detail = serde_json::to_string(detail).unwrap_or_else(|_| "\"\"".to_string());
            let script =
                format!("window.__dshSetStatus && window.__dshSetStatus({title}, {detail});");
            let _ = webview.evaluate_script(&script);
        }
    }

    fn show_error(&mut self, message: &str) {
        self.stop_child();
        self.started = false;
        self.current_url = None;
        self.set_status("启动失败", message);
        self.show_runtime_error(message);
    }

    fn show_runtime_error(&self, message: &str) {
        let message = serde_json::to_string(message).unwrap_or_else(|_| "\"未知错误\"".to_string());
        let script = format!(
            r#"
(() => {{
  const message = {message};
  const id = 'dsh-desktop-runtime-error';
  let panel = document.getElementById(id);
  if (!panel) {{
    panel = document.createElement('div');
    panel.id = id;
    panel.style.cssText = 'position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;padding:24px;background:rgba(8,11,18,.82);font-family:Inter,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#f7f8fb;';
    panel.innerHTML = '<div data-card style="width:min(560px,100%);padding:32px;border:1px solid rgba(255,255,255,.12);border-radius:24px;background:#111723;box-shadow:0 24px 80px rgba(0,0,0,.45)"><div style="font-size:22px;font-weight:750">工作台没有启动</div><div data-message style="margin-top:12px;color:#aab5c8;line-height:1.7;word-break:break-word"></div><div style="display:flex;gap:10px;margin-top:24px"><button data-retry style="border:0;border-radius:12px;padding:12px 18px;color:#071116;background:#8af6e0;font:inherit;font-weight:750;cursor:pointer">重新连接</button><button data-settings style="border:0;border-radius:12px;padding:12px 18px;color:#dce5f4;background:#ffffff12;font:inherit;font-weight:650;cursor:pointer">返回设置</button></div></div>';
    (document.body || document.documentElement).appendChild(panel);
    panel.querySelector('[data-retry]').onclick = () => window.ipc && window.ipc.postMessage(JSON.stringify({{type:'retry'}}));
    panel.querySelector('[data-settings]').onclick = () => window.ipc && window.ipc.postMessage(JSON.stringify({{type:'reset_key'}}));
  }}
  panel.querySelector('[data-message]').textContent = message;
}})();
"#,
            message = message
        );
        if let Some(webview) = &self.webview {
            let _ = webview.evaluate_script(&script);
        }
    }

    fn stop_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_process_tree(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl ApplicationHandler<UserEvent> for AppState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title(APP_NAME)
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(1280_u32, 820_u32))
            .with_min_inner_size(LogicalSize::new(960_u32, 640_u32));
        let attributes = attributes.with_window_icon(app_icon());
        let window = event_loop
            .create_window(attributes)
            .expect("create native window");

        let has_key = load_api_key().is_some();
        let proxy = self.proxy.clone();
        let pet_script = pet_mode_script(self.pet_hidden);
        let model_script = format!(
            "window.__dshDefaultModel = {};",
            serde_json::to_string(&load_model(&self.data_dir))
                .unwrap_or_else(|_| "null".to_string())
        );
        let builder = WebViewBuilder::new()
            .with_html(START_HTML)
            .with_initialization_script(WINDOW_CHROME_SCRIPT)
            .with_initialization_script(GOAL_MODE_SCRIPT)
            .with_initialization_script(&pet_script)
            .with_initialization_script(&model_script)
            .with_ipc_handler(move |request| {
                let parsed = serde_json::from_str::<IpcMessage>(request.body());
                let Ok(message) = parsed else { return };
                match message.kind.as_str() {
                    "save_key" => {
                        if let Some(key) = message.key {
                            let _ = proxy.send_event(UserEvent::SaveApiKey {
                                key,
                                model: message.model.unwrap_or_default(),
                            });
                        }
                    }
                    "reset_key" => {
                        let _ = proxy.send_event(UserEvent::ResetApiKey);
                    }
                    "retry" => {
                        let _ = proxy.send_event(UserEvent::RetryRuntime);
                    }
                    "pet_visibility" => {
                        if let Some(hidden) = message.hidden {
                            let _ = proxy.send_event(UserEvent::SetPetVisibility(hidden));
                        }
                    }
                    "window_minimize" => {
                        let _ = proxy.send_event(UserEvent::WindowMinimize);
                    }
                    "window_maximize" => {
                        let _ = proxy.send_event(UserEvent::WindowMaximize);
                    }
                    "window_close" => {
                        let _ = proxy.send_event(UserEvent::WindowClose);
                    }
                    "window_drag" => {
                        let _ = proxy.send_event(UserEvent::WindowDrag);
                    }
                    "open_docs" => {
                        let _ = open::that_detached("https://platform.deepseek.com/api_keys");
                    }
                    _ => {}
                }
            })
            .with_new_window_req_handler(|url, _features| {
                let _ = open::that_detached(&url);
                NewWindowResponse::Deny
            });

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        ))]
        let webview = builder.build(&window).expect("create webview");
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        )))]
        let webview = unreachable!("Linux is intentionally not a release target");

        self.window = Some(window);
        self.webview = Some(webview);

        if has_key {
            if let Some(key) = load_api_key() {
                self.start_runtime(key, load_model(&self.data_dir));
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::SaveApiKey {
                key: raw_key,
                model,
            } => {
                let key = raw_key.trim().to_string();
                if let Err(error) = validate_api_key(&key) {
                    self.show_error(&error);
                    return;
                }
                let model = if model.trim().is_empty() {
                    load_model(&self.data_dir)
                } else {
                    model.trim().to_string()
                };
                if let Err(error) = validate_model(&model) {
                    self.show_error(&error);
                    return;
                }
                if let Err(error) = save_api_key(&key) {
                    self.show_error(&format!("无法保存 API Key：{error}"));
                    return;
                }
                if let Err(error) = save_model(&self.data_dir, &model) {
                    self.show_error(&format!("无法保存模型选择：{error}"));
                    return;
                }
                self.start_runtime(key, model);
            }
            UserEvent::ResetApiKey => {
                let _ = delete_api_key();
                self.stop_child();
                self.started = false;
                self.current_url = None;
                if let Some(webview) = &self.webview {
                    let _ = webview.load_html(START_HTML);
                }
            }
            UserEvent::RetryRuntime => {
                if self.started {
                    return;
                }
                if let Some(key) = load_api_key() {
                    self.start_runtime(key, load_model(&self.data_dir));
                } else if let Some(webview) = &self.webview {
                    let _ = webview.load_html(START_HTML);
                }
            }
            UserEvent::SetPetVisibility(hidden) => {
                self.pet_hidden = hidden;
                let _ = save_pet_hidden(&self.data_dir, hidden);
            }
            UserEvent::WindowMinimize => {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
            }
            UserEvent::WindowMaximize => {
                if let Some(window) = &self.window {
                    window.set_maximized(!window.is_maximized());
                }
            }
            UserEvent::WindowClose => {
                self.stop_child();
                event_loop.exit();
            }
            UserEvent::WindowDrag => {
                if let Some(window) = &self.window {
                    let _ = window.drag_window();
                }
            }
            UserEvent::ProcessStarted(child) => {
                self.child = Some(child);
                self.set_status("正在启动 DSH", "首次启动会准备依赖，可能需要几分钟");
            }
            UserEvent::ServiceReady(url) => {
                self.current_url = Some(url.clone());
                self.set_status("已连接 DeepSeek", "正在载入工作台");
                if let Some(webview) = &self.webview {
                    if let Err(error) = webview.load_url(&url) {
                        self.show_error(&format!("载入工作台失败：{error}"));
                    }
                }
            }
            UserEvent::StartupFailed(error) => self.show_error(&error),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                if let (Some(window), Some(webview)) = (&self.window, &self.webview) {
                    let size = size.to_logical::<u32>(window.scale_factor());
                    let _ = webview.set_bounds(Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(size.width, size.height).into(),
                    });
                }
            }
            WindowEvent::CloseRequested => {
                self.stop_child();
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                terminate_process_tree(child.id());
                self.child = None;
                self.show_error(&format!("DSH 服务已退出（状态码：{}）", status));
            }
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + Duration::from_millis(250),
        ));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(app_data_dir()).ok();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut state = AppState::new(proxy);
    event_loop.run_app(&mut state)?;
    Ok(())
}

fn app_data_dir() -> PathBuf {
    ProjectDirs::from("com", "neko233", "DSH Desktop")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| env::temp_dir().join("dsh-desktop"))
}

fn app_icon() -> Option<Icon> {
    let decoder = png::Decoder::new(Cursor::new(APP_ICON_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    Icon::from_rgba(
        buffer[..info.buffer_size()].to_vec(),
        info.width,
        info.height,
    )
    .ok()
}

fn load_pet_hidden(data_dir: &Path) -> bool {
    fs::read_to_string(data_dir.join("pet-hidden"))
        .map(|value| matches!(value.trim(), "1" | "true"))
        .unwrap_or(false)
}

fn save_pet_hidden(data_dir: &Path, hidden: bool) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    fs::write(data_dir.join("pet-hidden"), if hidden { "1" } else { "0" })
        .map_err(|error| error.to_string())
}

fn pet_mode_script(hidden: bool) -> String {
    PET_MODE_SCRIPT
        .replace("__DSH_PET_HIDDEN__", if hidden { "true" } else { "false" })
        .replace(
            "__DSH_PET_SPRITE__",
            &format!("data:image/png;base64,{}", BASE64.encode(PET_SPRITE)),
        )
}

fn configured_workspace() -> PathBuf {
    env::var_os("DSH_DESKTOP_WORKSPACE")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(app_data_dir)
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|error| error.to_string())
}

fn load_api_key() -> Option<String> {
    if let Ok(key) = env::var("DEEPSEEK_API_KEY") {
        if validate_api_key(&key).is_ok() {
            return Some(key);
        }
    }
    keyring_entry()
        .ok()?
        .get_password()
        .ok()
        .filter(|key| validate_api_key(key).is_ok())
}

fn save_api_key(key: &str) -> Result<(), String> {
    keyring_entry()?
        .set_password(key)
        .map_err(|error| error.to_string())
}

fn delete_api_key() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().to_lowercase().contains("not found") => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_api_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    if key.len() > 512 || key.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return Err("API Key 格式异常：请粘贴平台生成的完整 Key".to_string());
    }
    Ok(())
}

fn load_model(data_dir: &Path) -> String {
    if let Ok(model) = env::var("DEEPSEEK_MODEL") {
        if validate_model(&model).is_ok() {
            return model.trim().to_string();
        }
    }
    fs::read_to_string(data_dir.join("selected-model"))
        .ok()
        .map(|model| model.trim().to_string())
        .filter(|model| validate_model(model).is_ok())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

fn save_model(data_dir: &Path, model: &str) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    fs::write(data_dir.join("selected-model"), model.trim()).map_err(|error| error.to_string())
}

fn validate_model(model: &str) -> Result<(), String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("请选择 DeepSeek 模型".to_string());
    }
    if model.len() > 128
        || model
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err("模型名称格式异常".to_string());
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Runner {
    program: String,
    base_args: Vec<String>,
}

fn resolve_runner() -> Result<Runner, String> {
    if let Ok(custom) = env::var("DSH_DESKTOP_DSH_BIN") {
        if !custom.trim().is_empty() {
            return Ok(Runner {
                program: custom,
                base_args: Vec::new(),
            });
        }
    }

    if let Some(npx) = bundled_npx() {
        return Ok(Runner {
            program: npx.to_string_lossy().to_string(),
            base_args: vec!["--yes".to_string(), HARNESS_PACKAGE.to_string()],
        });
    }

    #[cfg(target_os = "windows")]
    let dsh_candidates = ["dsh.cmd", "dsh.exe", "dsh"];
    #[cfg(not(target_os = "windows"))]
    let dsh_candidates = ["dsh"];
    for candidate in dsh_candidates {
        if command_available(candidate) {
            return Ok(Runner {
                program: candidate.to_string(),
                base_args: Vec::new(),
            });
        }
    }

    #[cfg(target_os = "windows")]
    let npx = "npx.cmd";
    #[cfg(not(target_os = "windows"))]
    let npx = "npx";
    if command_available(npx) {
        return Ok(Runner {
            program: npx.to_string(),
            base_args: vec!["--yes".to_string(), HARNESS_PACKAGE.to_string()],
        });
    }
    Err("内置运行时未就绪。请重新运行安装器，或设置 DSH_DESKTOP_DSH_BIN".to_string())
}

fn command_available(program: &str) -> bool {
    if Path::new(program).is_absolute() {
        return Path::new(program).is_file();
    }
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(program).is_file()))
        .unwrap_or(false)
}

fn bundled_runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = env::var_os("DSH_DESKTOP_NODE_BIN") {
        if let Some(parent) = PathBuf::from(path).parent() {
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            if let Some(install_dir) = bin_dir.parent() {
                roots.push(install_dir.join("runtime").join("node"));
            }
            if let Some(contents_dir) = bin_dir.parent() {
                roots.push(contents_dir.join("Resources").join("runtime").join("node"));
            }
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        roots.push(current_dir.join("runtime").join("node"));
    }
    roots
}

fn bundled_runtime_path(name: &str) -> Option<PathBuf> {
    for root in bundled_runtime_roots() {
        #[cfg(target_os = "windows")]
        let candidates = [root.join(format!("{name}.cmd")), root.join(name)];
        #[cfg(not(target_os = "windows"))]
        let candidates = [root.join("bin").join(name)];
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn bundled_npx() -> Option<PathBuf> {
    if let Some(path) = env::var_os("DSH_DESKTOP_NODE_BIN") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    bundled_runtime_path("npx")
}

fn bundled_corepack() -> Option<PathBuf> {
    bundled_runtime_path("corepack")
}

fn locate_package_manager() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let names = ["pnpm.cmd", "pnpm.exe", "pnpm"];
    #[cfg(not(target_os = "windows"))]
    let names = ["pnpm"];

    for root in bundled_runtime_roots() {
        for name in names {
            let candidate = root.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        #[cfg(not(target_os = "windows"))]
        for name in names {
            let candidate = root.join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for name in names {
        if let Some(paths) = env::var_os("PATH") {
            if let Some(path) = env::split_paths(&paths)
                .map(|path| path.join(name))
                .find(|path| path.is_file())
            {
                return Some(path);
            }
        }
    }
    None
}

fn prepend_runtime_path(command: &mut Command) {
    let mut paths = bundled_runtime_roots();
    #[cfg(not(target_os = "windows"))]
    paths.extend(
        bundled_runtime_roots()
            .into_iter()
            .map(|root| root.join("bin")),
    );
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

fn choose_port() -> Result<u16, String> {
    if let Ok(raw) = env::var("DSH_DESKTOP_PORT") {
        let port = raw
            .parse::<u16>()
            .map_err(|_| "DSH_DESKTOP_PORT 不是有效端口".to_string())?;
        if port != 0 && TcpListener::bind((SERVICE_HOST, port)).is_ok() {
            return Ok(port);
        }
        return Err(format!("端口 {port} 不可用"));
    }
    (DEFAULT_PORT..=DEFAULT_PORT + 100)
        .find(|port| TcpListener::bind((SERVICE_HOST, *port)).is_ok())
        .ok_or_else(|| "没有可用的本地端口".to_string())
}

fn start_runtime_worker(
    proxy: EventLoopProxy<UserEvent>,
    data_dir: PathBuf,
    workspace: PathBuf,
    frontend_dir: PathBuf,
    key: String,
    model: String,
) -> Result<(), String> {
    validate_api_key(&key)?;
    validate_model(&model)?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("创建数据目录失败：{error}"))?;
    let runner = resolve_runner()?;
    let npm_registry = env::var("DSH_NPM_REGISTRY")
        .ok()
        .or_else(|| fs::read_to_string(data_dir.join("npm-registry")).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(DEFAULT_NPM_REGISTRY.to_string()));

    if !local_frontend_is_installed(&data_dir, &frontend_dir) {
        install_local_frontend(&data_dir, &frontend_dir, &key, npm_registry.as_deref())?;
        fs::write(
            data_dir.join("web-ui-local-source"),
            frontend_dir.to_string_lossy().as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    }
    normalize_local_profile_bundles()?;

    let port = choose_port()?;
    let port_text = port.to_string();
    let mut command = command_for(
        &runner,
        &["web", "--host", SERVICE_HOST, "--port", &port_text],
    );
    configure_command(
        &mut command,
        &workspace,
        &key,
        &model,
        npm_registry.as_deref(),
    );
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 DSH 失败：{error}"))?;

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(data_dir.clone(), "stdout", stdout, key.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(data_dir.clone(), "stderr", stderr, key.clone());
    }

    proxy
        .send_event(UserEvent::ProcessStarted(child))
        .map_err(|_| "桌面窗口已关闭".to_string())?;
    let url = format!("http://{SERVICE_HOST}:{port}/");
    if wait_for_http(port, Duration::from_secs(120)) {
        proxy
            .send_event(UserEvent::ServiceReady(url))
            .map_err(|_| "桌面窗口已关闭".to_string())?;
        Ok(())
    } else {
        Err("DSH 服务启动超时。点击重试，或查看日志定位 Node.js / 网络问题".to_string())
    }
}

fn local_frontend_is_installed(data_dir: &Path, frontend_dir: &Path) -> bool {
    let marker = data_dir.join("web-ui-local-source");
    let Ok(installed) = fs::read_to_string(marker) else {
        return false;
    };
    installed.trim() == frontend_dir.to_string_lossy()
}

fn normalize_local_profile_bundles() -> Result<(), String> {
    let profile_dir =
        dsh_profile_dir().ok_or_else(|| "无法定位 DSH web profile 目录".to_string())?;
    let package_path = profile_dir.join("package.json");
    let source = fs::read_to_string(&package_path)
        .map_err(|error| format!("读取 DSH profile 配置失败：{error}"))?;
    let mut document: serde_json::Value = serde_json::from_str(&source)
        .map_err(|error| format!("解析 DSH profile 配置失败：{error}"))?;
    document["name"] = serde_json::json!("dsh-profile-web");
    document["private"] = serde_json::json!(true);
    document["dsh"]["profile"]["bundles"] = serde_json::json!([
        "@deepseek-ai/dsh-base",
        "@deepseek-ai/dsh-web-app",
        "@linxin666/dsh-web-ui-all"
    ]);
    let rendered = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("写入 DSH profile 配置失败：{error}"))?;
    fs::write(package_path, format!("{rendered}\n"))
        .map_err(|error| format!("写入 DSH profile 配置失败：{error}"))?;

    let empty_patch = "[]\n";
    for name in ["cordis.yml", "cordis.patch.yml"] {
        let path = profile_dir.join(name);
        if !path.is_file() {
            fs::write(path, empty_patch)
                .map_err(|error| format!("创建 DSH profile 文件失败：{error}"))?;
        }
    }
    let workspace_path = profile_dir.join("pnpm-workspace.yaml");
    if !workspace_path.is_file() {
        fs::write(
            workspace_path,
            "packages:\n  - .\n\nnodeLinker: hoisted\nautoInstallPeers: false\n",
        )
        .map_err(|error| format!("创建 DSH profile pnpm 配置失败：{error}"))?;
    }
    Ok(())
}

fn install_local_frontend(
    data_dir: &Path,
    frontend_dir: &Path,
    key: &str,
    npm_registry: Option<&str>,
) -> Result<(), String> {
    let package_dirs = local_frontend_package_dirs(frontend_dir)?;
    let profile_dir =
        dsh_profile_dir().ok_or_else(|| "无法定位 DSH web profile 目录".to_string())?;
    fs::create_dir_all(&profile_dir).map_err(|error| format!("创建 DSH profile 失败：{error}"))?;
    let workspace_path = profile_dir.join("pnpm-workspace.yaml");
    if !workspace_path.is_file() {
        fs::write(
            &workspace_path,
            "packages:\n  - .\n\nnodeLinker: hoisted\nautoInstallPeers: false\n",
        )
        .map_err(|error| format!("创建 DSH profile pnpm 配置失败：{error}"))?;
    }
    ensure_package_manager(data_dir, key, npm_registry)?;

    let mut command = package_manager_command()
        .ok_or_else(|| "未找到 pnpm。请重新启动以自动准备内置 Corepack".to_string())?;
    hide_console_window(&mut command);
    command
        .current_dir(&profile_dir)
        .arg("--config.minimumReleaseAge=0")
        .arg("add")
        .arg("--save-prod");
    for dependency in LOCAL_UI_RUNTIME_DEPS {
        command.arg(dependency);
    }
    for package_dir in &package_dirs {
        command.arg(format!("link:{}", package_dir.to_string_lossy()));
    }
    configure_package_manager(&mut command, npm_registry);
    let output = command
        .output()
        .map_err(|error| format!("接入内置 Web UI 组件失败：{error}"))?;
    log_bytes(data_dir, "web-ui-local", &output.stdout, key);
    log_bytes(data_dir, "web-ui-local", &output.stderr, key);
    if !output.status.success() {
        return Err("内置 Web UI 组件接入失败。请检查 Node.js、pnpm 或镜像配置".to_string());
    }
    let _ = approve_dsh_build_scripts(data_dir, key);
    normalize_local_profile_bundles()
}

fn ensure_package_manager(
    data_dir: &Path,
    key: &str,
    npm_registry: Option<&str>,
) -> Result<(), String> {
    if locate_package_manager().is_some() {
        return Ok(());
    }
    let corepack = bundled_corepack()
        .ok_or_else(|| "内置 Node.js 未包含 Corepack，无法自动准备 pnpm".to_string())?;
    let root = bundled_runtime_roots()
        .into_iter()
        .find(|path| path.is_dir())
        .ok_or_else(|| "无法定位内置 Node.js 目录".to_string())?;
    #[cfg(target_os = "windows")]
    let install_dir = root.clone();
    #[cfg(not(target_os = "windows"))]
    let install_dir = root.join("bin");

    let mut command = Command::new(corepack);
    hide_console_window(&mut command);
    command
        .arg("enable")
        .arg("--install-directory")
        .arg(&install_dir);
    if let Some(registry) = npm_registry {
        command.env("NPM_CONFIG_REGISTRY", registry);
        command.env("npm_config_registry", registry);
    }
    prepend_runtime_path(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("启用内置 pnpm 失败：{error}"))?;
    log_bytes(data_dir, "corepack", &output.stdout, key);
    log_bytes(data_dir, "corepack", &output.stderr, key);
    if output.status.success() && locate_package_manager().is_some() {
        return Ok(());
    }
    Err("无法自动准备 pnpm。请检查网络或设置 DSH_NPM_REGISTRY 后重试".to_string())
}

fn local_frontend_package_dirs(frontend_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut packages = Vec::with_capacity(LOCAL_UI_PACKAGE_PATHS.len());
    for relative in LOCAL_UI_PACKAGE_PATHS {
        let path = frontend_dir.join(relative);
        let has_runtime_entry = path.join("lib/index.js").is_file();
        let has_bundle_patch = path.join("cordis.patch.yml").is_file();
        if !path.join("package.json").is_file() || (!has_runtime_entry && !has_bundle_patch) {
            return Err(format!(
                "内置 Web UI 不完整：缺少 {}（发布包必须包含 frontend/dsh-web-ui）",
                path.display()
            ));
        }
        packages.push(path);
    }
    Ok(packages)
}

fn command_for(runner: &Runner, args: &[&str]) -> Command {
    let mut command = Command::new(&runner.program);
    command.args(&runner.base_args).args(args);
    hide_console_window(&mut command);
    command
}

#[cfg(target_os = "windows")]
fn hide_console_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console_window(_command: &mut Command) {}

#[cfg(target_os = "windows")]
fn terminate_process_tree(pid: u32) {
    let mut command = Command::new("taskkill.exe");
    hide_console_window(&mut command);
    let _ = command
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(not(target_os = "windows"))]
fn terminate_process_tree(_pid: u32) {}

fn approve_dsh_build_scripts(data_dir: &Path, key: &str) -> bool {
    let Some(profile_dir) = dsh_profile_dir() else {
        return false;
    };
    if fs::create_dir_all(&profile_dir).is_err() {
        return false;
    }
    let Some(pnpm) = package_manager_command() else {
        return false;
    };

    let mut approve = pnpm;
    hide_console_window(&mut approve);
    approve
        .current_dir(&profile_dir)
        .arg("--config.minimumReleaseAge=0")
        .arg("approve-builds")
        .args(PNPM_BUILD_PACKAGES);
    let Ok(output) = approve.output() else {
        return false;
    };
    log_bytes(data_dir, "pnpm-approve", &output.stdout, key);
    log_bytes(data_dir, "pnpm-approve", &output.stderr, key);
    if !output.status.success() {
        return false;
    }

    let Some(pnpm) = package_manager_command() else {
        return true;
    };
    let mut rebuild = pnpm;
    hide_console_window(&mut rebuild);
    rebuild
        .current_dir(&profile_dir)
        .arg("--config.minimumReleaseAge=0")
        .arg("rebuild")
        .args(PNPM_BUILD_PACKAGES);
    if let Ok(output) = rebuild.output() {
        log_bytes(data_dir, "pnpm-rebuild", &output.stdout, key);
        log_bytes(data_dir, "pnpm-rebuild", &output.stderr, key);
    }
    true
}

#[cfg(target_os = "windows")]
fn package_manager_command() -> Option<Command> {
    locate_package_manager().map(Command::new)
}

#[cfg(not(target_os = "windows"))]
fn package_manager_command() -> Option<Command> {
    locate_package_manager().map(Command::new)
}

fn configure_package_manager(command: &mut Command, npm_registry: Option<&str>) {
    prepend_runtime_path(command);
    command.env("npm_config_minimum_release_age", "0");
    command.env("NPM_CONFIG_MINIMUM_RELEASE_AGE", "0");
    command.env("pnpm_config_minimum_release_age", "0");
    if let Some(registry) = npm_registry {
        command.env("NPM_CONFIG_REGISTRY", registry);
        command.env("npm_config_registry", registry);
    }
}

fn dsh_profile_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("DSH_PROFILE_DIR") {
        return Some(PathBuf::from(path));
    }
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(|home| {
            PathBuf::from(home)
                .join(".dsh")
                .join("profiles")
                .join("web")
        })
}

fn configured_frontend_dir() -> PathBuf {
    if let Some(path) = env::var_os("DSH_DESKTOP_FRONTEND_DIR") {
        return PathBuf::from(path);
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            if let Some(install_dir) = bin_dir.parent() {
                candidates.push(install_dir.join("frontend").join("dsh-web-ui"));
            }
            if let Some(contents_dir) = bin_dir.parent() {
                candidates.push(
                    contents_dir
                        .join("Resources")
                        .join("frontend")
                        .join("dsh-web-ui"),
                );
            }
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("frontend").join("dsh-web-ui"));
    }
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("frontend/dsh-web-ui"))
}

fn configure_command(
    command: &mut Command,
    workspace: &Path,
    key: &str,
    model: &str,
    npm_registry: Option<&str>,
) {
    command.current_dir(workspace);
    prepend_runtime_path(command);
    command.env("DEEPSEEK_API_KEY", key);
    command.env(
        "DEEPSEEK_BASE_URL",
        env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
    );
    command.env("DEEPSEEK_MODEL", model);
    if let Some(registry) = npm_registry {
        command.env("NPM_CONFIG_REGISTRY", registry);
        command.env("npm_config_registry", registry);
    }
}

fn wait_for_http(port: u16, timeout: Duration) -> bool {
    let started = SystemTime::now();
    while started.elapsed().unwrap_or(timeout) < timeout {
        if let Ok(mut stream) = TcpStream::connect((SERVICE_HOST, port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ =
                stream.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
            let mut buffer = [0_u8; 256];
            let size = stream.read(&mut buffer).unwrap_or(0);
            if http_response_is_success(&buffer[..size]) {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn http_response_is_success(response: &[u8]) -> bool {
    String::from_utf8_lossy(response)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|status| status.starts_with('2'))
        .unwrap_or(false)
}

fn spawn_log_reader<R: Read + Send + 'static>(
    data_dir: PathBuf,
    stream_name: &str,
    mut reader: R,
    key: String,
) {
    let stream_name = stream_name.to_string();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if reader.read_to_end(&mut bytes).is_ok() {
            log_bytes(&data_dir, &stream_name, &bytes, &key);
        }
    });
}

fn log_bytes(data_dir: &Path, stream_name: &str, bytes: &[u8], key: &str) {
    let text = String::from_utf8_lossy(bytes).replace(key, "***");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("dsh.log"))
    {
        let _ = writeln!(file, "[{timestamp}] [{stream_name}] {text}");
    }
}

#[cfg(test)]
mod tests {
    use super::{http_response_is_success, validate_api_key};

    #[test]
    fn validates_key_shape_without_requiring_prefix() {
        assert!(validate_api_key("sk-test-key").is_ok());
        assert!(validate_api_key("other-provider-token").is_ok());
        assert!(validate_api_key("").is_err());
        assert!(validate_api_key("has space").is_err());
    }

    #[test]
    fn redact_does_not_echo_key() {
        assert_eq!("hello sk-secret".replace("sk-secret", "***"), "hello ***");
    }

    #[test]
    fn only_accepts_successful_http_boot_response() {
        assert!(http_response_is_success(b"HTTP/1.1 200 OK\r\n"));
        assert!(http_response_is_success(b"HTTP/1.1 204 No Content\r\n"));
        assert!(!http_response_is_success(b"HTTP/1.1 404 Not Found\r\n"));
        assert!(!http_response_is_success(b"not an http response"));
    }
}
