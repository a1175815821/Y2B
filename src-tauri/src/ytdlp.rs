//! yt-dlp 内置管理 + 更新检查 + ffmpeg 状态
//!
//! 策略（Windows）：
//! 1. resources/yt-dlp.exe（随安装包打包，tauri.conf.json bundle.resources）
//! 2. %APPDATA%/com.y2b.downloader/bin/yt-dlp.exe（首次运行自动下载）
//! 3. 系统 PATH 里的 yt-dlp / yt-dlp.exe。
//!
//! 检查更新走 GitHub API：yt-dlp/yt-dlp releases/latest

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Windows 下隐藏子进程控制台窗口，防止频繁弹出 CMD。
/// 非 Windows 平台为空实现。
#[cfg(windows)]
pub fn hide_std(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000);
}
#[cfg(not(windows))]
pub fn hide_std(_cmd: &mut std::process::Command) {}

#[cfg(windows)]
pub fn hide_tokio(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x08000000);
}
#[cfg(not(windows))]
pub fn hide_tokio(_cmd: &mut tokio::process::Command) {}

pub const YTDLP_WIN_ASSET: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_API_LATEST: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

#[derive(Debug, Clone, Serialize)]
pub struct YtdlpStatus {
    pub path: Option<String>,
    pub version: Option<String>,
    pub ready: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct YtdlpUpdateInfo {
    pub current: Option<String>,
    pub latest: String,
    pub need_update: bool,
    pub download_url: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    published_at: Option<String>,
}

/// 解析出可执行文件路径（按优先级），返回 (路径, 来源)
/// 顺序：应用数据目录（用户下载/更新的最新版，优先于出厂内置）
/// → 打包 resources（出厂内置）→ 系统 PATH
pub fn locate_ytdlp(app: &AppHandle) -> (Option<PathBuf>, &'static str) {
    // 1. 应用数据目录下载/更新的副本（版本最新，优先）
    if let Ok(data) = app.path().app_data_dir() {
        let p = data.join("bin").join("yt-dlp.exe");
        if p.is_file() {
            return (Some(p), "downloaded");
        }
    }
    // 2. 打包 resources（出厂内置版本）
    if let Ok(res_dir) = app.path().resource_dir() {
        for cand in [
            res_dir.join("resources").join("yt-dlp.exe"),
            res_dir.join("resources").join("yt-dlp"),
            res_dir.join("yt-dlp.exe"),
        ] {
            if cand.is_file() {
                return (Some(cand), "bundled");
            }
        }
        // 开发模式：项目根 resources/
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("yt-dlp.exe");
        if dev.is_file() {
            return (Some(dev), "bundled");
        }
    }
    // 3. 系统 PATH
    for name in ["yt-dlp.exe", "yt-dlp"] {
        let mut where_cmd = std::process::Command::new(path_probe_cmd());
        hide_std(&mut where_cmd);
        if let Ok(out) = where_cmd.arg(name).output() {
            if out.status.success() {
                let first = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !first.is_empty() {
                    return (Some(PathBuf::from(first)), "system");
                }
            }
        }
    }
    (None, "missing")
}

/// PATH 探针命令：Windows 用 where，其他平台用 which
fn path_probe_cmd() -> &'static str {
    #[cfg(windows)]
    {
        "where"
    }
    #[cfg(not(windows))]
    {
        "which"
    }
}

async fn bin_version(path: &std::path::Path) -> Option<String> {
    let mut cmd = tokio::process::Command::new(path);
    hide_tokio(&mut cmd);
    cmd.arg("--version");
    // yt-dlp --version 本地执行，10 秒足够，超时直接视为未就绪，避免卡住 UI
    let out = tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[tauri::command]
pub async fn ytdlp_status(app: AppHandle) -> YtdlpStatus {
    let (path, source) = locate_ytdlp(&app);
    match path {
        Some(p) => {
            let v = bin_version(&p).await;
            YtdlpStatus {
                path: Some(p.to_string_lossy().to_string()),
                version: v.clone(),
                ready: v.is_some(),
                source: source.to_string(),
            }
        }
        None => YtdlpStatus {
            path: None,
            version: None,
            ready: false,
            source: "missing".into(),
        },
    }
}

/// 按设置里的代理构建 reqwest 客户端；代理地址非法时回退直连，避免整功能不可用
fn client_builder(proxy: Option<&str>) -> reqwest::ClientBuilder {
    let mut b = reqwest::Client::builder().user_agent("Y2B-downloader");
    if let Some(p) = proxy.map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(px) = reqwest::Proxy::all(p) {
            b = b.proxy(px);
        }
    }
    b
}

fn settings_proxy(app: &AppHandle) -> Option<String> {
    crate::settings::get_settings(app.clone())
        .ok()
        .and_then(|s| s.proxy)
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
}

async fn download_to(url: &str, dest: &PathBuf, proxy: Option<&str>) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let client = client_builder(proxy).build().map_err(|e| e.to_string())?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败，HTTP {}", resp.status()));
    }
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    use std::io::Write;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn ensure_ytdlp(app: AppHandle) -> Result<YtdlpStatus, String> {
    let st = ytdlp_status(app.clone()).await;
    if st.ready {
        return Ok(st);
    }
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let dest = data.join("bin").join("yt-dlp.exe");
    let proxy = settings_proxy(&app);
    download_to(YTDLP_WIN_ASSET, &dest, proxy.as_deref()).await?;
    Ok(ytdlp_status(app).await)
}

async fn fetch_latest_release(proxy: Option<&str>) -> Result<GithubRelease, String> {
    let client = client_builder(proxy).build().map_err(|e| e.to_string())?;
    let rel: GithubRelease = client
        .get(YTDLP_API_LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API 请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub API 错误: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析 release 失败: {e}"))?;
    Ok(rel)
}

#[tauri::command]
pub async fn check_ytdlp_update(app: AppHandle) -> Result<YtdlpUpdateInfo, String> {
    let proxy = settings_proxy(&app);
    let st = ytdlp_status(app).await;
    let rel = fetch_latest_release(proxy.as_deref()).await?;
    let latest = rel.tag_name.trim().to_string();
    let need = match &st.version {
        Some(cur) => cur.trim() != latest,
        None => true,
    };
    Ok(YtdlpUpdateInfo {
        current: st.version,
        latest,
        need_update: need,
        download_url: YTDLP_WIN_ASSET.to_string(),
        published_at: rel.published_at,
    })
}

#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<YtdlpStatus, String> {
    // 更新统一写入应用数据目录；按 locate 优先级，该副本会盖过出厂内置版本
    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("bin")
        .join("yt-dlp.exe");
    // 先下到临时文件再替换，避免断网损坏旧版本
    let tmp = dest.with_extension("exe.new");
    let proxy = settings_proxy(&app);
    download_to(YTDLP_WIN_ASSET, &tmp, proxy.as_deref()).await?;
    // 简单校验：能跑出 --version 才替换
    let v = bin_version(&tmp).await.ok_or("新版本校验失败（无法运行）")?;
    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    let _ = v;
    Ok(ytdlp_status(app).await)
}

// ---------------- ffmpeg ----------------

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegStatus {
    pub path: Option<String>,
    pub ready: bool,
    pub source: String,
}

fn locate_ffmpeg(app: &AppHandle) -> (Option<PathBuf>, &'static str) {
    // 应用数据目录优先（用户下载的），出厂内置次之：保证更新/下载真正生效
    if let Ok(data) = app.path().app_data_dir() {
        let p = data.join("bin").join("ffmpeg.exe");
        if p.is_file() {
            return (Some(p), "downloaded");
        }
    }
    if let Ok(res_dir) = app.path().resource_dir() {
        for cand in [
            res_dir.join("resources").join("ffmpeg.exe"),
            res_dir.join("ffmpeg.exe"),
        ] {
            if cand.is_file() {
                return (Some(cand), "bundled");
            }
        }
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("ffmpeg.exe");
        if dev.is_file() {
            return (Some(dev), "bundled");
        }
    }
    {
        let mut where_cmd = std::process::Command::new(path_probe_cmd());
        hide_std(&mut where_cmd);
        if where_cmd
            .arg("ffmpeg.exe")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return (Some(PathBuf::from("ffmpeg.exe")), "system");
        }
    }
    (None, "missing")
}

#[tauri::command]
pub fn ffmpeg_status(app: AppHandle) -> FfmpegStatus {
    let (p, s) = locate_ffmpeg(&app);
    FfmpegStatus {
        path: p.map(|x| x.to_string_lossy().to_string()),
        ready: s != "missing",
        source: s.to_string(),
    }
}

#[tauri::command]
pub async fn ensure_ffmpeg(app: AppHandle) -> Result<FfmpegStatus, String> {
    let st = ffmpeg_status(app.clone());
    if st.ready {
        return Ok(st);
    }
    // yt-dlp 官方 FFmpeg 构建（Windows 64-bit）
    let url = "https://github.com/yt-dlp/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip";
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let zip_path = data.join("bin").join("ffmpeg.zip");
    let proxy = settings_proxy(&app);
    download_to(url, &zip_path, proxy.as_deref()).await?;
    // 解压出 bin/ffmpeg.exe
    let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let dest = data.join("bin").join("ffmpeg.exe");
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name().ends_with("bin/ffmpeg.exe") {
            let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            break;
        }
    }
    let _ = std::fs::remove_file(&zip_path);
    // 压缩包结构变化时可能根本没解出文件，必须校验，否则会静默“成功”
    if !dest.is_file() {
        return Err("ffmpeg 下载包解压失败（包内未找到 bin/ffmpeg.exe），请稍后重试".into());
    }
    Ok(ffmpeg_status(app))
}

// ---------------- YouTube PO-Token / player_client ----------------
//
// 背景见 yt-dlp#17542 + PO-Token-Guide：年龄限制视频的 web_embedded /
// tv_downgraded 直接 UNPLAYABLE，高清只剩 mweb / web_creator，而这两个
// 需要 GVS PO Token（插件自动刷）。这里统一构造 --plugin-dirs 与
// --extractor-args，解析/下载/校验三处调用共用。

/// 默认插件目录：%APPDATA%/com.y2b.downloader/yt-dlp-plugins
/// 把 bgutil-ytdlp-pot-provider 等插件仓库克隆到该目录下即可被加载。
pub fn default_plugin_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("yt-dlp-plugins")
}

/// 归一化 player_client：auto/空 = 不传（用 yt-dlp 默认）。
/// 注意 "default" 是 yt-dlp 的有效 client 名（如 default,mweb），不可当作 auto。
fn normalized_client(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(t.to_string())
    }
}

/// 生效的 player_client：用户显式选择优先；auto + 允许自动 + 本地 PO 栈可用 → mweb
/// （mweb 高清强制要 GVS Token，没有本地栈时传 mweb 只会全 403，所以要门控）。
pub fn effective_player_client(
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) -> Option<String> {
    if let Some(c) = normalized_client(&settings.youtube_player_client) {
        return Some(c);
    }
    if settings.youtube_po_auto && crate::pot::stack_usable(app) {
        return Some("mweb".into());
    }
    None
}

/// 构造 --extractor-args 参数值列表，如
/// ["youtube:player_client=mweb", "youtube:po_token=mweb.gvs+XXX",
///  "youtubepot-bgutilscript:server_home=C:/.../server"]
/// 纯函数版本（可单测）：stack 门控与 server_home 由调用方传入。
pub fn youtube_extractor_args_for(
    settings: &crate::settings::AppSettings,
    effective_client: Option<&str>,
    script_home: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(c) = effective_client {
        out.push(format!("youtube:player_client={c}"));
    }
    if let Some(tok) = settings
        .youtube_po_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // 允许填完整体 "youtube:po_token=..."，否则按 youtube:po_token= 补前缀
        if tok.contains(':') {
            out.push(tok.to_string());
        } else {
            out.push(format!("youtube:po_token={tok}"));
        }
    }
    // 脚本兜底：HTTP 服务不可用时插件自动降级走 node 直调（已实测跑通），
    // HTTP 可用时该参数无副作用（HTTP 优先）。
    if let Some(h) = script_home {
        out.push(format!("youtubepot-bgutilscript:server_home={h}"));
    }
    out
}

/// 供调用方使用的版本：自动计算生效客户端与脚本目录
pub fn youtube_extractor_args(
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) -> Vec<String> {
    let client = effective_player_client(app, settings);
    let home = crate::pot::script_home_usable(app)
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    youtube_extractor_args_for(settings, client.as_deref(), home.as_deref())
}

/// 收集插件目录：默认目录 + 自定义目录（存在的才传，去重）。
/// 目录不存在/未设置时返回空，调用方不传 --plugin-dirs。
pub fn youtube_plugin_dirs(
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let def = default_plugin_dir(app);
    if def.is_dir() {
        dirs.push(def);
    }
    if let Some(custom) = settings
        .youtube_plugin_dirs
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let p = PathBuf::from(custom);
        if p.is_dir() && !dirs.iter().any(|d| d == &p) {
            dirs.push(p);
        }
    }
    dirs
}

/// 把 YouTube 相关参数追加到 tokio Command（解析/下载/校验统一走这里，
/// 避免某条链路漏传导致 18+ 视频时好时坏）。
pub fn apply_youtube_options(
    cmd: &mut tokio::process::Command,
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) {
    for d in youtube_plugin_dirs(app, settings) {
        cmd.arg("--plugin-dirs").arg(d);
    }
    // 内置 portable node：provider 脚本与 [jsc] 挑战都走它，不依赖用户装 node/deno
    let node = crate::pot::node_exe(app);
    if node.is_file() {
        cmd.arg("--js-runtimes")
            .arg(format!("node:{}", node.to_string_lossy()));
    }
    for a in youtube_extractor_args(app, settings) {
        cmd.arg("--extractor-args").arg(a);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PotStatus {
    pub plugin_dir: String,
    pub plugin_files: Vec<String>,
    pub has_plugin: bool,
    pub extractor_args_preview: Vec<String>,
    /// 本地 PO 服务栈（三件套）是否就绪
    pub stack_installed: bool,
    /// HTTP 服务（127.0.0.1:4416）是否可达
    pub server_running: bool,
    pub server_version: Option<String>,
    /// 实际生效的客户端（auto 经门控解析后）
    pub effective_client: String,
}

/// 前端设置页展示用：默认插件目录 + 目录内容 + 当前 extractor-args 预览 + 服务栈状态。
/// 附带一次 2 秒 /ping 快检，调用仍廉价。
#[tauri::command]
pub async fn pot_status(app: AppHandle) -> PotStatus {
    let settings = crate::settings::get_settings(app.clone()).unwrap_or_default();
    let dir = default_plugin_dir(&app);
    let mut files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if let Some(n) = e.file_name().to_str().map(|s| s.to_string()) {
                files.push(n);
            }
        }
        files.sort();
        files.truncate(50);
    }
    let mut has_plugin = !files.is_empty();
    // 自定义目录有文件也算有插件（展示时加 custom/ 前缀区分）
    if let Some(custom) = settings
        .youtube_plugin_dirs
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let p = PathBuf::from(custom);
        if p.is_dir() && p != dir {
            if let Ok(entries) = std::fs::read_dir(&p) {
                let mut extra: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .map(|s| format!("custom/{s}"))
                    })
                    .collect();
                extra.sort();
                if !extra.is_empty() {
                    has_plugin = true;
                    files.extend(extra);
                    files.sort();
                    files.truncate(50);
                }
            }
        }
    }
    let stack = crate::pot::stack_status(&app).await;
    PotStatus {
        plugin_dir: dir.to_string_lossy().to_string(),
        plugin_files: files,
        has_plugin,
        extractor_args_preview: youtube_extractor_args(&app, &settings),
        stack_installed: stack.installed,
        server_running: stack.server_running,
        server_version: stack.server_version,
        effective_client: stack.effective_client,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;

    #[test]
    fn auto_client_means_no_extractor_args() {
        let mut s = AppSettings::default();
        s.youtube_player_client = "auto".into();
        s.youtube_po_token = None;
        assert!(youtube_extractor_args_for(&s, None, None).is_empty());
        // 空字符串同样视为 auto
        s.youtube_player_client = "  ".into();
        assert!(youtube_extractor_args_for(&s, None, None).is_empty());
    }

    #[test]
    fn mweb_client_and_manual_token() {
        let mut s = AppSettings::default();
        s.youtube_player_client = "mweb".into();
        s.youtube_po_token = Some("mweb.gvs+XXX".into());
        assert_eq!(
            youtube_extractor_args_for(&s, Some("mweb"), None),
            vec![
                "youtube:player_client=mweb".to_string(),
                "youtube:po_token=mweb.gvs+XXX".to_string(),
            ]
        );
    }

    #[test]
    fn full_token_passthrough_kept_as_is() {
        let mut s = AppSettings::default();
        s.youtube_player_client = "auto".into();
        s.youtube_po_token = Some("youtube:po_token=web_creator.gvs+YYY".into());
        assert_eq!(
            youtube_extractor_args_for(&s, None, None),
            vec!["youtube:po_token=web_creator.gvs+YYY".to_string()]
        );
    }

    #[test]
    fn script_home_fallback_appended() {
        let s = AppSettings::default();
        assert_eq!(
            youtube_extractor_args_for(&s, Some("mweb"), Some("C:/x/server")),
            vec![
                "youtube:player_client=mweb".to_string(),
                "youtubepot-bgutilscript:server_home=C:/x/server".to_string(),
            ]
        );
    }

    #[test]
    fn default_keyword_is_a_real_client_not_auto() {
        // "default,mweb" 是 yt-dlp 有效写法，必须透传
        let mut s = AppSettings::default();
        s.youtube_player_client = "default,mweb".into();
        s.youtube_po_token = None;
        assert_eq!(
            youtube_extractor_args_for(&s, Some("default,mweb"), None),
            vec!["youtube:player_client=default,mweb".to_string()]
        );
    }
}
