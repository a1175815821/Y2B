//! Cookie 管理：Netscape cookies.txt 命名配置
//! 存储：%APPDATA%/com.y2b.downloader/cookies/<name>.txt
//! 默认配置名记录在 settings.json 的 default_cookie_profile

use chrono::{DateTime, Local};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::settings::{get_settings, save_settings};

fn cookies_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("cookies");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[derive(Debug, Clone, Serialize)]
pub struct CookieProfile {
    pub name: String,
    pub path: String,
    pub updated_at: Option<String>,
    pub is_default: bool,
    pub entry_count: usize,
}

pub fn cookie_file_for(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err("配置名仅允许英文/数字/-/_(最长64)".into());
    }
    Ok(cookies_dir(app)?.join(format!("{name}.txt")))
}

#[tauri::command]
pub fn cookie_list(app: AppHandle) -> Result<Vec<CookieProfile>, String> {
    let dir = cookies_dir(&app)?;
    let settings = get_settings(app.clone()).unwrap_or_default();
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("txt") {
            continue;
        }
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let content = std::fs::read_to_string(&p).unwrap_or_default();
        let count = content
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .count();
        let updated_at: Option<String> = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            });
        out.push(CookieProfile {
            is_default: settings.default_cookie_profile.as_deref() == Some(&name),
            name,
            path: p.to_string_lossy().to_string(),
            updated_at,
            entry_count: count,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// 从任意位置的 Netscape 文件导入为命名配置（拷贝 + 基础校验）
#[tauri::command]
pub fn cookie_import(app: AppHandle, name: String, src_path: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if !valid_name(&name) {
        return Err("配置名仅允许英文/数字/-/_(最长64)".into());
    }
    let content = std::fs::read_to_string(&src_path).map_err(|e| format!("读取源文件失败: {e}"))?;
    let has_header = content.lines().any(|l| l.contains("Netscape HTTP Cookie File"));
    let data_lines = content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .count();
    if !has_header && data_lines == 0 {
        return Err("文件看起来不是 Netscape cookies.txt（缺少 Header 且无数据行）".into());
    }
    let dest = cookie_file_for(&app, &name)?;
    std::fs::write(&dest, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cookie_remove(app: AppHandle, name: String) -> Result<(), String> {
    let dest = cookie_file_for(&app, &name)?;
    if dest.is_file() {
        std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
    }
    // 若删的是默认配置，顺手清空
    if let Ok(mut s) = get_settings(app.clone()) {
        if s.default_cookie_profile.as_deref() == Some(&name) {
            s.default_cookie_profile = None;
            let _ = save_settings(app, s);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn cookie_set_default(app: AppHandle, name: Option<String>) -> Result<(), String> {
    if let Some(n) = &name {
        let p = cookie_file_for(&app, n)?;
        if !p.is_file() {
            return Err(format!("配置 {n} 不存在"));
        }
    }
    let mut s = get_settings(app.clone())?;
    s.default_cookie_profile = name;
    save_settings(app, s)?;
    Ok(())
}

/// 轻量校验：用该 cookie 依次探测常青测试视频，成功即有效。
/// 之所以轮询多个，是因为单个视频可能下架（曾因此误报）；
/// 通过 stderr 区分“视频不可用”（换下一个）与“需登录/机器人验证”（Cookie 真无效）。
const COOKIE_PROBE_VIDEOS: [&str; 2] = [
    // Me at the zoo：YouTube 第一个视频，几乎不可能下架
    "https://www.youtube.com/watch?v=jNQXAC9IVRw",
    // Big Buck Bunny：Blender 官方，十年以上稳定
    "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
];

fn stderr_means_bot_check(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("not a bot")
        || s.contains("sign in to confirm")
        || (s.contains("sign in") && s.contains("bot"))
}

fn stderr_means_video_gone(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("unavailable")
        || s.contains("private video")
        || s.contains("has been deleted")
        || s.contains("does not exist")
}

#[tauri::command]
pub async fn cookie_validate(app: AppHandle, name: String) -> Result<String, String> {
    use crate::ytdlp::{hide_tokio, locate_ytdlp};
    let cookie = cookie_file_for(&app, &name)?;
    if !cookie.is_file() {
        return Err("Cookie 文件不存在".into());
    }
    let (bin, _) = locate_ytdlp(&app);
    let bin = bin.ok_or("yt-dlp 未就绪，请先下载内置 yt-dlp")?;
    let yt_settings = crate::settings::get_settings(app.clone()).unwrap_or_default();
    let mut last_err = String::new();
    for probe in COOKIE_PROBE_VIDEOS {
        let mut cmd = tokio::process::Command::new(&bin);
        hide_tokio(&mut cmd);
        crate::ytdlp::apply_youtube_options(&mut cmd, &app, &yt_settings);
        cmd.args([
            "--cookies",
            &cookie.to_string_lossy(),
            "--dump-json",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "15",
            probe,
        ]);
        let out = tokio::time::timeout(std::time::Duration::from_secs(45), cmd.output())
            .await
            .map_err(|_| "校验超时（45秒），请检查网络/代理后重试".to_string())?
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            return Ok(format!("Cookie {name} 校验通过（可正常访问 YouTube）"));
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if stderr_means_bot_check(&stderr) {
            return Err(format!(
                "Cookie 无效：YouTube 要求登录验证（not a bot），请重新导出 Cookie 后再导入。详情：{stderr}"
            ));
        }
        // 视频本身不可用 → 换下一个探测，不算 Cookie 的错
        last_err = stderr;
        if !stderr_means_video_gone(&last_err) {
            // 非预期错误也继续试下一个，保留现场
            continue;
        }
    }
    Err(format!(
        "校验未通过：{}，请稍后重试",
        crate::errhint::friendly_yt_dlp_error(&last_err)
    ))
}
