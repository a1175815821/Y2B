//! URL 解析 + 完整格式列表
//! resolve_url: 单视频 / 播放列表 / 频道统一走
//!   yt-dlp --dump-single-json --flat-playlist --playlist-end N
//! list_formats: yt-dlp -J --no-playlist（完整 formats）

use serde::Serialize;
use tauri::AppHandle;

use crate::cookies::cookie_file_for;
use crate::settings::get_settings;
use crate::ytdlp::locate_ytdlp;

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedMedia {
    pub kind: String,
    pub id: Option<String>,
    pub title: Option<String>,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub video_count: Option<u64>,
    pub entries_preview: Vec<VideoEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoEntry {
    pub id: String,
    pub title: Option<String>,
    pub url: String,
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    pub upload_date: Option<String>,
    pub view_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormatItem {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<u64>,
    pub filesize_approx: Option<u64>,
    pub tbr: Option<f64>,
    pub protocol: Option<String>,
    pub format_note: Option<String>,
}

fn v_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(|s| s.to_string())
}
fn v_u64(v: &serde_json::Value, key: &str) -> Option<u64> {
    v.get(key)?.as_u64()
}

fn pick_thumbnail(v: &serde_json::Value) -> Option<String> {
    if let Some(t) = v.get("thumbnail").and_then(|x| x.as_str()) {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    // thumbnails 数组取最后一个（通常分辨率最高）
    v.get("thumbnails")?
        .as_array()?
        .iter()
        .rev()
        .find_map(|t| t.get("url")?.as_str().map(|s| s.to_string()))
}

#[tauri::command]
pub async fn resolve_url(
    app: AppHandle,
    url: String,
    max_entries: Option<u32>,
) -> Result<ResolvedMedia, String> {
    let (bin, _) = locate_ytdlp(&app);
    let bin = bin.ok_or("yt-dlp 未就绪，请先到 设置/更新 下载内置 yt-dlp")?;
    let max = max_entries.unwrap_or(100).clamp(1, 500);

    let settings = get_settings(app.clone()).unwrap_or_default();
    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args([
        "--dump-single-json",
        "--flat-playlist",
        "--no-warnings",
        "--socket-timeout",
        "20",
        "--playlist-end",
        &max.to_string(),
    ]);
    if let Some(ref prof) = settings.default_cookie_profile {
        if let Ok(p) = cookie_file_for(&app, prof) {
            if p.is_file() {
                cmd.arg("--cookies").arg(p);
            }
        }
    }
    if let Some(ref proxy) = settings.proxy {
        if !proxy.trim().is_empty() {
            cmd.arg("--proxy").arg(proxy.trim());
        }
    }
    cmd.arg(&url);

    let out = cmd.output().await.map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "解析失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("解析 yt-dlp 输出失败: {e}"))?;

    // _type 缺失或为 video → 单视频
    let jtype = json.get("_type").and_then(|x| x.as_str()).unwrap_or("video");
    if jtype == "video" || json.get("formats").is_some() {
        let id = v_str(&json, "id").unwrap_or_default();
        let direct = if id.is_empty() {
            url.clone()
        } else {
            format!("https://www.youtube.com/watch?v={id}")
        };
        return Ok(ResolvedMedia {
            kind: "video".into(),
            id: v_str(&json, "id"),
            title: v_str(&json, "title"),
            uploader: v_str(&json, "uploader").or_else(|| v_str(&json, "channel")),
            thumbnail: pick_thumbnail(&json),
            video_count: Some(1),
            entries_preview: vec![VideoEntry {
                id: id.clone(),
                title: v_str(&json, "title"),
                url: v_str(&json, "webpage_url").unwrap_or(direct),
                duration: v_u64(&json, "duration"),
                thumbnail: pick_thumbnail(&json),
                uploader: v_str(&json, "uploader"),
                upload_date: v_str(&json, "upload_date"),
                view_count: v_u64(&json, "view_count"),
            }],
            truncated: false,
        });
    }

    // playlist / channel
    let entries = json
        .get("entries")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    let truncated = entries.len() as u32 >= max;
    let preview = entries
        .into_iter()
        .filter_map(|e| {
            let id = e.get("id")?.as_str()?.to_string();
            let entry_url = e
                .get("url")
                .and_then(|x| x.as_str())
                .map(|u| {
                    if u.starts_with("http") {
                        u.to_string()
                    } else {
                        format!("https://www.youtube.com/watch?v={id}")
                    }
                })
                .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
            Some(VideoEntry {
                id,
                title: v_str(&e, "title"),
                url: v_str(&e, "webpage_url").unwrap_or(entry_url),
                duration: v_u64(&e, "duration"),
                thumbnail: pick_thumbnail(&e),
                uploader: v_str(&e, "uploader").or_else(|| v_str(&e, "channel")),
                upload_date: v_str(&e, "upload_date"),
                view_count: v_u64(&e, "view_count"),
            })
        })
        .collect::<Vec<_>>();
    let total = json
        .get("playlist_count")
        .and_then(|x| x.as_u64())
        .or(Some(preview.len() as u64));
    Ok(ResolvedMedia {
        kind: if url.contains("/@") || url.contains("/channel/") || url.contains("/c/") {
            "channel".into()
        } else {
            "playlist".into()
        },
        id: v_str(&json, "id"),
        title: v_str(&json, "title"),
        uploader: v_str(&json, "uploader").or_else(|| v_str(&json, "channel")),
        thumbnail: pick_thumbnail(&json),
        video_count: total,
        entries_preview: preview,
        truncated,
    })
}

#[tauri::command]
pub async fn list_formats(app: AppHandle, url: String) -> Result<Vec<FormatItem>, String> {
    let (bin, _) = locate_ytdlp(&app);
    let bin = bin.ok_or("yt-dlp 未就绪")?;
    let settings = get_settings(app.clone()).unwrap_or_default();
    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args(["-J", "--no-playlist", "--no-warnings", "--socket-timeout", "20"]);
    if let Some(ref prof) = settings.default_cookie_profile {
        if let Ok(p) = cookie_file_for(&app, prof) {
            if p.is_file() {
                cmd.arg("--cookies").arg(p);
            }
        }
    }
    if let Some(ref proxy) = settings.proxy {
        if !proxy.trim().is_empty() {
            cmd.arg("--proxy").arg(proxy.trim());
        }
    }
    cmd.arg(&url);
    let out = cmd.output().await.map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "获取格式失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("解析失败: {e}"))?;
    let formats = json
        .get("formats")
        .and_then(|f| f.as_array())
        .ok_or("该视频无可用格式")?;
    Ok(formats
        .iter()
        .map(|f| FormatItem {
            format_id: f
                .get("format_id")
                .and_then(|x| x.as_str())
                .unwrap_or("?")
                .to_string(),
            ext: f
                .get("ext")
                .and_then(|x| x.as_str())
                .unwrap_or("?")
                .to_string(),
            resolution: f
                .get("resolution")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            fps: f.get("fps").and_then(|x| x.as_f64()),
            vcodec: f.get("vcodec").and_then(|x| x.as_str()).map(|s| {
                if s == "none" {
                    "-".into()
                } else {
                    s.to_string()
                }
            }),
            acodec: f.get("acodec").and_then(|x| x.as_str()).map(|s| {
                if s == "none" {
                    "-".into()
                } else {
                    s.to_string()
                }
            }),
            filesize: f.get("filesize").and_then(|x| x.as_u64()),
            filesize_approx: f.get("filesize_approx").and_then(|x| x.as_u64()),
            tbr: f.get("tbr").and_then(|x| x.as_f64()),
            protocol: f
                .get("protocol")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            format_note: f
                .get("format_note")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
        })
        .collect())
}
