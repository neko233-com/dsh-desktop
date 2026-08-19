#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use directories::ProjectDirs;
use serde::Deserialize;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

const APP_NAME: &str = "DSH Desktop";
const SERVICE_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3080;
const WEB_UI_PLUGIN: &str = "@linxin666/dsh-web-ui-all@latest";
const KEYRING_SERVICE: &str = "dsh-desktop";
const KEYRING_USER: &str = "deepseek-api-key";
const START_HTML: &str = include_str!("../assets/start.html");
const GOAL_MODE_SCRIPT: &str = include_str!("../assets/goal-mode.js");
const PET_MODE_SCRIPT: &str = include_str!("../assets/pet-mode.js");
const PET_SPRITE: &[u8] = include_bytes!("../assets/pet/maid-sprite-final.png");

#[derive(Debug)]
enum UserEvent {
    SaveApiKey(String),
    ResetApiKey,
    SetPetVisibility(bool),
    ProcessStarted(Child),
    ServiceReady(String),
    StartupFailed(String),
}

#[derive(Debug, Deserialize)]
struct IpcMessage {
    #[serde(rename = "type")]
    kind: String,
    key: Option<String>,
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

    fn start_runtime(&mut self, key: String) {
        if self.started {
            return;
        }
        self.started = true;
        self.set_status("正在准备 DeepSeek 环境…", "正在检查 DSH 与 Web UI 插件");

        let proxy = self.proxy.clone();
        let data_dir = self.data_dir.clone();
        let workspace = configured_workspace();
        thread::spawn(move || {
            if let Err(error) = start_runtime_worker(proxy.clone(), data_dir, workspace, key) {
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
        self.set_status("启动失败", message);
    }

    fn stop_child(&mut self) {
        if let Some(mut child) = self.child.take() {
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
            .with_inner_size(LogicalSize::new(1280_u32, 820_u32))
            .with_min_inner_size(LogicalSize::new(960_u32, 640_u32));
        let window = event_loop
            .create_window(attributes)
            .expect("create native window");

        let has_key = load_api_key().is_some();
        let proxy = self.proxy.clone();
        let pet_script = pet_mode_script(self.pet_hidden);
        let builder = WebViewBuilder::new()
            .with_html(START_HTML)
            .with_initialization_script(GOAL_MODE_SCRIPT)
            .with_initialization_script(&pet_script)
            .with_ipc_handler(move |request| {
                let parsed = serde_json::from_str::<IpcMessage>(request.body());
                let Ok(message) = parsed else { return };
                match message.kind.as_str() {
                    "save_key" => {
                        if let Some(key) = message.key {
                            let _ = proxy.send_event(UserEvent::SaveApiKey(key));
                        }
                    }
                    "reset_key" => {
                        let _ = proxy.send_event(UserEvent::ResetApiKey);
                    }
                    "pet_visibility" => {
                        if let Some(hidden) = message.hidden {
                            let _ = proxy.send_event(UserEvent::SetPetVisibility(hidden));
                        }
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
                self.start_runtime(key);
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::SaveApiKey(raw_key) => {
                let key = raw_key.trim().to_string();
                if let Err(error) = validate_api_key(&key) {
                    self.show_error(&error);
                    return;
                }
                if let Err(error) = save_api_key(&key) {
                    self.show_error(&format!("无法保存 API Key：{error}"));
                    return;
                }
                self.start_runtime(key);
            }
            UserEvent::ResetApiKey => {
                let _ = delete_api_key();
                self.stop_child();
                self.started = false;
                self.current_url = None;
                self.set_status("连接 DeepSeek", "输入 API Key 后开始使用");
            }
            UserEvent::SetPetVisibility(hidden) => {
                self.pet_hidden = hidden;
                let _ = save_pet_hidden(&self.data_dir, hidden);
            }
            UserEvent::ProcessStarted(child) => {
                self.child = Some(child);
                self.set_status("正在启动 DSH", "首次启动会准备依赖，可能需要几分钟");
            }
            UserEvent::ServiceReady(url) => {
                self.current_url = Some(url.clone());
                self.set_status("已连接 DeepSeek", "正在载入工作台");
                if let Some(webview) = &self.webview {
                    let _ = webview.load_url(&url);
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
            base_args: vec!["--yes".to_string(), "@deepseek-ai/dsh".to_string()],
        });
    }
    Err("未找到 dsh 或 npx。请先安装 Node.js 22+，或设置 DSH_DESKTOP_DSH_BIN".to_string())
}

fn command_available(program: &str) -> bool {
    if Path::new(program).is_absolute() {
        return Path::new(program).is_file();
    }
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(program).is_file()))
        .unwrap_or(false)
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
    key: String,
) -> Result<(), String> {
    validate_api_key(&key)?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("创建数据目录失败：{error}"))?;
    let runner = resolve_runner()?;
    let npm_registry = env::var("DSH_NPM_REGISTRY")
        .ok()
        .or_else(|| fs::read_to_string(data_dir.join("npm-registry")).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if !data_dir.join("web-ui-installed").is_file() {
        let mut install = command_for(
            &runner,
            &["plugin", "--profile", "web", "add", WEB_UI_PLUGIN],
        );
        configure_command(&mut install, &workspace, &key, npm_registry.as_deref());
        let output = install
            .output()
            .map_err(|error| format!("安装 Web UI 插件失败：{error}"))?;
        log_bytes(&data_dir, "plugin", &output.stdout, &key);
        log_bytes(&data_dir, "plugin", &output.stderr, &key);
        if !output.status.success() {
            return Err("Web UI 插件安装失败。可设置 DSH_NPM_REGISTRY 后重试".to_string());
        }
        File::create(data_dir.join("web-ui-installed")).map_err(|error| error.to_string())?;
    }

    let port = choose_port()?;
    let port_text = port.to_string();
    let mut command = command_for(
        &runner,
        &["web", "--host", SERVICE_HOST, "--port", &port_text],
    );
    configure_command(&mut command, &workspace, &key, npm_registry.as_deref());
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

fn command_for(runner: &Runner, args: &[&str]) -> Command {
    let mut command = Command::new(&runner.program);
    command.args(&runner.base_args).args(args);
    command
}

fn configure_command(
    command: &mut Command,
    workspace: &Path,
    key: &str,
    npm_registry: Option<&str>,
) {
    command.current_dir(workspace);
    command.env("DEEPSEEK_API_KEY", key);
    command.env(
        "DEEPSEEK_BASE_URL",
        env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
    );
    command.env(
        "DEEPSEEK_MODEL",
        env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-v4-pro".to_string()),
    );
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
            if stream.read(&mut buffer).unwrap_or(0) > 0 {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
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
    use super::validate_api_key;

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
}
