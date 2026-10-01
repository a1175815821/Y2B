//! yt-dlp 内置管理 + 更新检查 + ffmpeg 状态
//!
//! 策略（Windows）：
//! 1. resources/yt-dlp.exe（随安装包打包，tauri.conf.json bundle.resources）
//! 2. %APPDATA%/com.y2b.downloader/bin/yt-dlp.exe（首次运行自动下载）
//! 3. 系统 PATH 里的 yt-dlp / yt-dlp.exe
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
pub fn locate_ytdlp(app: &AppHandle) -> (Option<PathBuf>, &'static str) {
    // 1. 打包 resources
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
    // 2. 应用数据目录下载的副本
    if let Ok(data) = app.path().app_data_dir() {
        let p = data.join("bin").join("yt-dlp.exe");
        if p.is_file() {
            return (Some(p), "downloaded");
        }
    }
    // 3. 系统 PATH
    for name in ["yt-dlp.exe", "yt-dlp"] {
        let mut where_cmd = std::process::Command::new("where");
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
    let (path, source) = locate_ytdlp(&app);
    // bundled 的不覆盖安装包文件，统一更新到 app_data/bin
    let dest = if source == "downloaded" {
        path.unwrap()
    } else {
        app.path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("bin")
            .join("yt-dlp.exe")
    };
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
    if let Ok(data) = app.path().app_data_dir() {
        let p = data.join("bin").join("ffmpeg.exe");
        if p.is_file() {
            return (Some(p), "downloaded");
        }
    }
    {
        let mut where_cmd = std::process::Command::new("where");
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
    Ok(ffmpeg_status(app))
}
