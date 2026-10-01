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
        if let Ok(out) = std::process::Command::new("where")
            .arg(name)
            .output()
        {
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

fn bin_version(path: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
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
pub fn ytdlp_status(app: AppHandle) -> YtdlpStatus {
    let (path, source) = locate_ytdlp(&app);
    match path {
        Some(p) => {
            let v = bin_version(&p);
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

async fn download_to(url: &str, dest: &PathBuf) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let client = reqwest::Client::builder()
        .user_agent("Y2B-downloader")
        .build()
        .map_err(|e| e.to_string())?;
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
    let st = ytdlp_status(app.clone());
    if st.ready {
        return Ok(st);
    }
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let dest = data.join("bin").join("yt-dlp.exe");
    download_to(YTDLP_WIN_ASSET, &dest).await?;
    Ok(ytdlp_status(app))
}

async fn fetch_latest_release() -> Result<GithubRelease, String> {
    let client = reqwest::Client::builder()
        .user_agent("Y2B-downloader")
        .build()
        .map_err(|e| e.to_string())?;
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
    let st = ytdlp_status(app);
    let rel = fetch_latest_release().await?;
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
    download_to(YTDLP_WIN_ASSET, &tmp).await?;
    // 简单校验：能跑出 --version 才替换
    let v = bin_version(&tmp).ok_or("新版本校验失败（无法运行）")?;
    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    let _ = v;
    Ok(ytdlp_status(app))
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
    if std::process::Command::new("where")
        .arg("ffmpeg.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return (Some(PathBuf::from("ffmpeg.exe")), "system");
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
    download_to(url, &zip_path).await?;
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
