//! PO-Token 本地服务栈：一键下载 / 自启 / 健康检查
//!
//! 用户手动装 bgutil 不现实，所以 Y2B 把三件套托管好、点一下全自动：
//!   bin/pot/plugin/bgutil-ytdlp-pot-provider.zip  ← 上游 provider 插件（8KB）
//!   bin/pot/server/{build,node_modules}           ← 预编译 server（Y2B Release 托管）
//!   bin/pot/node/node.exe                          ← Node portable（nodejs.org）
//!
//! 运行时：node 起 HTTP server（127.0.0.1:4416），yt-dlp 插件优先走 HTTP 取 Token；
//! server_home 兜底（script 方式）始终透传，HTTP 不可用时自动降级（已实测跑通）。
//! 上游 GPL-3.0，Y2B 仅做运行时自动下载 + 本地编排，见包内 LICENSE 与 README。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use crate::ytdlp::hide_tokio;

/// 与 provider release 配对的 server 版本（README 要求 plugin 与 server 同版）
pub const POT_PROVIDER_TAG: &str = "2.0.1";
/// Y2B Release 上托管的预编译包名
pub const POT_SERVER_BUNDLE: &str = "pot-server-2.0.1-win-x64.zip";
/// Node portable 版本（canvas 原生绑定按大版本 ABI 兼容，须用 Node 24）
pub const POT_NODE_VERSION: &str = "v24.21.0";

const POT_PORT: u16 = 4416;

fn provider_zip_url() -> String {
    format!(
        "https://github.com/Brainicism/bgutil-ytdlp-pot-provider/releases/download/{POT_PROVIDER_TAG}/bgutil-ytdlp-pot-provider.zip"
    )
}
fn server_bundle_url() -> String {
    format!("https://github.com/a1175815821/Y2B/releases/download/v0.2.0/{POT_SERVER_BUNDLE}")
}
fn node_zip_url() -> String {
    format!("https://nodejs.org/dist/{POT_NODE_VERSION}/node-{POT_NODE_VERSION}-win-x64.zip")
}

/// 三件套根目录：%APPDATA%/com.y2b.downloader/bin/pot
pub fn pot_root(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("bin")
        .join("pot")
}

/// provider 插件 zip 位置（直接丢进 yt-dlp 插件目录即可被加载，已实测）
pub fn provider_zip_path(app: &AppHandle) -> PathBuf {
    crate::ytdlp::default_plugin_dir(app).join("bgutil-ytdlp-pot-provider.zip")
}

/// server 解压目录（含 build/main.js + node_modules + package.json）
pub fn server_home(app: &AppHandle) -> PathBuf {
    pot_root(app).join("server")
}

/// portable node 位置
pub fn node_exe(app: &AppHandle) -> PathBuf {
    pot_root(app).join("node").join("node.exe")
}

fn versions_marker(app: &AppHandle) -> PathBuf {
    pot_root(app).join("VERSIONS.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StackVersions {
    provider_tag: Option<String>,
    server_bundle: Option<String>,
    node_version: Option<String>,
}

fn read_versions(app: &AppHandle) -> StackVersions {
    std::fs::read_to_string(versions_marker(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_versions(app: &AppHandle, v: &StackVersions) {
    if let Some(parent) = versions_marker(app).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(v) {
        let _ = std::fs::write(versions_marker(app), s);
    }
}

/// 宽松可用性（文件齐即算数，不要求版本标记）：门控 auto→mweb 用它，
/// 用户手动凑齐文件也能生效；UI 展示用严格的 stack_installed。
pub fn stack_usable(app: &AppHandle) -> bool {
    provider_zip_path(app).is_file()
        && script_home_usable(app).is_some()
        && node_exe(app).is_file()
}

/// 三件套是否齐（文件存在 + 版本标记匹配当前 pin）
pub fn stack_installed(app: &AppHandle) -> bool {
    let v = read_versions(app);
    v.provider_tag.as_deref() == Some(POT_PROVIDER_TAG)
        && v.server_bundle.as_deref() == Some(POT_SERVER_BUNDLE)
        && v.node_version.as_deref() == Some(POT_NODE_VERSION)
        && provider_zip_path(app).is_file()
        && server_home(app).join("build").join("main.js").is_file()
        && server_home(app).join("build").join("generate_once.js").is_file()
        && node_exe(app).is_file()
}

/// 脚本兜底可用的 server 目录（generate_once.js 存在即可，不要求版本标记）
pub fn script_home_usable(app: &AppHandle) -> Option<PathBuf> {
    let h = server_home(app);
    if h.join("build").join("generate_once.js").is_file() {
        Some(h)
    } else {
        None
    }
}

/// HTTP 服务是否曾被确认可用（后台/启动时刷新，调用方只读不阻塞）
static HTTP_READY: AtomicBool = AtomicBool::new(false);

pub fn http_ready() -> bool {
    HTTP_READY.load(Ordering::SeqCst)
}

fn ping_url() -> String {
    format!("http://127.0.0.1:{POT_PORT}/ping")
}

/// 快检：/ping 2 秒内返回 server_uptime 即视为服务可用
async fn ping_ok() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    let body = client.get(ping_url()).send().await.ok()?;
    if !body.status().is_success() {
        return None;
    }
    let text = body.text().await.ok()?;
    if !text.contains("server_uptime") {
        return None;
    }
    // {"server_uptime":123,"version":"2.0.1"} → 顺手带回版本
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .or(Some(String::new()))
}

async fn refresh_ready_flag() -> bool {
    let ok = ping_ok().await.is_some();
    HTTP_READY.store(ok, Ordering::SeqCst);
    ok
}

static SERVER_CHILD: OnceLock<tokio::sync::Mutex<Option<tokio::process::Child>>> =
    OnceLock::new();

fn child_slot() -> &'static tokio::sync::Mutex<Option<tokio::process::Child>> {
    SERVER_CHILD.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// 尝试复用已有服务（用户自己起的 bgutil 也行），否则用内置 node 拉起。
/// 成功后等待 /ping，最多约 30 秒。
pub async fn start_server(app: AppHandle) -> Result<String, String> {
    if refresh_ready_flag().await {
        return Ok("PO 服务已在运行（复用现有 127.0.0.1:4416）".into());
    }
    if !stack_installed(&app) {
        return Err("PO 组件还没安装，请先点「一键安装并启动」".into());
    }
    {
        // 旧子进程残留先清理
        let mut g = child_slot().lock().await;
        if let Some(mut c) = g.take() {
            let _ = c.kill().await;
        }
    }
    let node = node_exe(&app);
    let home = server_home(&app);
    let mut cmd = tokio::process::Command::new(&node);
    hide_tokio(&mut cmd);
    cmd.current_dir(&home)
        .args(["build/main.js", "--port", &POT_PORT.to_string(), "--host", "127.0.0.1"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| format!("启动 PO 服务失败: {e}"))?;
    *child_slot().lock().await = Some(child);
    // 等 /ping：冷启动 Node + 初始化一般几秒内完成
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if refresh_ready_flag().await {
            return Ok("PO 服务已启动（127.0.0.1:4416）".into());
        }
    }
    // 30 秒还没起来：端口可能被占或机器特殊，script 兜底仍可工作，不判死刑
    Ok("PO 服务启动中（/ping 暂未响应，下载时会自动走脚本兜底，可稍后重试）".into())
}

/// 停止我们拉起的服务（用户自建的不碰）
pub async fn stop_server() -> Result<String, String> {
    let mut g = child_slot().lock().await;
    if let Some(mut c) = g.take() {
        let _ = c.kill().await;
    }
    HTTP_READY.store(false, Ordering::SeqCst);
    Ok("已停止 Y2B 拉起的 PO 服务".into())
}

async fn download_to(url: &str, dest: &PathBuf, proxy: Option<&str>) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut b = reqwest::Client::builder().user_agent("Y2B-downloader");
    if let Some(p) = proxy.map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(px) = reqwest::Proxy::all(p) {
            b = b.proxy(px);
        }
    }
    let client = b.build().map_err(|e| e.to_string())?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败，HTTP {}", resp.status()));
    }
    let tmp = dest.with_extension("new");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    use std::io::Write;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(())
}

fn unzip_all(zip_path: &PathBuf, dest_dir: &PathBuf) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        let out = dest_dir.join(path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// node portable 包里只取 node.exe（win-x64/node.exe）
fn unzip_node_exe(zip_path: &PathBuf, dest_exe: &PathBuf) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name().ends_with("win-x64/node.exe") {
            if let Some(p) = dest_exe.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut f = std::fs::File::create(dest_exe).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("node 压缩包里没找到 win-x64/node.exe".into())
}

fn settings_proxy(app: &AppHandle) -> Option<String> {
    crate::settings::get_settings(app.clone())
        .ok()
        .and_then(|s| s.proxy)
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
}

#[derive(Debug, Clone, Serialize)]
pub struct PotStackStatus {
    pub installed: bool,
    pub server_running: bool,
    pub server_version: Option<String>,
    pub provider_tag: String,
    pub server_bundle: String,
    pub node_version: String,
    pub effective_client: String,
    pub extractor_args_preview: Vec<String>,
    pub plugin_dir: String,
}

pub async fn stack_status(app: &AppHandle) -> PotStackStatus {
    let running_ver = ping_ok().await;
    HTTP_READY.store(running_ver.is_some(), Ordering::SeqCst);
    let settings = crate::settings::get_settings(app.clone()).unwrap_or_default();
    PotStackStatus {
        installed: stack_installed(app),
        server_running: running_ver.is_some(),
        server_version: running_ver.filter(|v| !v.is_empty()),
        provider_tag: POT_PROVIDER_TAG.into(),
        server_bundle: POT_SERVER_BUNDLE.into(),
        node_version: POT_NODE_VERSION.into(),
        effective_client: crate::ytdlp::effective_player_client(app, &settings)
            .unwrap_or_else(|| "auto".into()),
        extractor_args_preview: crate::ytdlp::youtube_extractor_args(app, &settings),
        plugin_dir: crate::ytdlp::default_plugin_dir(app).to_string_lossy().to_string(),
    }
}

/// 一键安装（缺啥下啥）并启动。refresh=true 时强制重下（更新用）。
#[tauri::command]
pub async fn pot_ensure(app: AppHandle, refresh: bool) -> Result<PotStackStatus, String> {
    if refresh {
        let root = pot_root(&app);
        if root.is_dir() {
            std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        let zp = provider_zip_path(&app);
        if zp.is_file() {
            let _ = std::fs::remove_file(&zp);
        }
        let _ = stop_server().await;
    }
    let proxy = settings_proxy(&app);
    let tmp = std::env::temp_dir().join("y2b-pot");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    // 1. 插件 zip（8KB）
    let zp = provider_zip_path(&app);
    if refresh || !zp.is_file() {
        let dl = tmp.join("provider.zip");
        download_to(&provider_zip_url(), &dl, proxy.as_deref()).await?;
        if let Some(p) = zp.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&dl, &zp).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&dl);
    }
    // 2. server 预编译包（~27MB）
    let home = server_home(&app);
    if refresh || !home.join("build").join("generate_once.js").is_file() {
        let dl = tmp.join(POT_SERVER_BUNDLE);
        download_to(&server_bundle_url(), &dl, proxy.as_deref()).await?;
        if home.is_dir() {
            std::fs::remove_dir_all(&home).map_err(|e| e.to_string())?;
        }
        // 包内顶层即 server/ 目录，解到 pot_root 下
        unzip_all(&dl, &pot_root(&app))?;
        let _ = std::fs::remove_file(&dl);
        if !home.join("build").join("main.js").is_file() {
            return Err("PO 服务包解压异常（缺 build/main.js），请重试".into());
        }
    }
    // 3. portable node（~30MB，只取 node.exe）
    let node = node_exe(&app);
    if refresh || !node.is_file() {
        let dl = tmp.join("node.zip");
        download_to(&node_zip_url(), &dl, proxy.as_deref()).await?;
        unzip_node_exe(&dl, &node)?;
        let _ = std::fs::remove_file(&dl);
    }
    write_versions(
        &app,
        &StackVersions {
            provider_tag: Some(POT_PROVIDER_TAG.into()),
            server_bundle: Some(POT_SERVER_BUNDLE.into()),
            node_version: Some(POT_NODE_VERSION.into()),
        },
    );
    // 4. 起服务（ best-effort：起不来也有 script 兜底）
    let msg = start_server(app.clone()).await.unwrap_or_else(|e| e);
    let mut st = stack_status(&app).await;
    // 把启动结论塞进 effective_client？不，状态里另起字段太碎，前端看 server_running 即可
    let _ = msg;
    Ok(st)
}

#[tauri::command]
pub async fn pot_stop() -> Result<String, String> {
    stop_server().await
}

/// App 启动时后台调用：三件套齐才自启服务（缺件不静默下载，等用户点一键安装）
pub async fn autostart(app: AppHandle) {
    if !stack_installed(&app) {
        return;
    }
    let _ = start_server(app).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_pinned_and_wellformed() {
        assert!(provider_zip_url().contains(POT_PROVIDER_TAG));
        assert!(server_bundle_url().contains(POT_SERVER_BUNDLE));
        assert!(node_zip_url().contains(POT_NODE_VERSION));
        assert!(provider_zip_url().starts_with("https://"));
    }
}
