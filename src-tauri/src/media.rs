//! URL 解析 + 完整格式列表
//! resolve_url: 单视频 / 播放列表 / 频道统一走
//!   yt-dlp --dump-single-json --flat-playlist --playlist-end N
//! list_formats: yt-dlp -J --no-playlist（完整 formats）

use serde::Serialize;
use tauri::AppHandle;

use crate::cookies::cookie_file_for;
use crate::settings::get_settings;
use crate::ytdlp::{hide_tokio, locate_ytdlp};

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
    pub height: Option<u32>,
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

/// flat-playlist 抓取，返回解析后的 JSON
async fn fetch_flat_json(
    app: &AppHandle,
    bin: &std::path::Path,
    url: &str,
    max: u32,
) -> Result<serde_json::Value, String> {
    let settings = get_settings(app.clone()).unwrap_or_default();
    let mut cmd = tokio::process::Command::new(bin);
    hide_tokio(&mut cmd);
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
        if let Ok(p) = cookie_file_for(app, prof) {
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
    cmd.arg(url);

    // 总超时兜底：socket-timeout 只管单连接，整命令卡住时前端不再无限转圈
    let out = tokio::time::timeout(std::time::Duration::from_secs(90), cmd.output())
        .await
        .map_err(|_| "解析超时（90秒），请检查网络/代理后重试".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(crate::errhint::friendly(
            "解析失败",
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("解析 yt-dlp 输出失败: {e}"))
}

/// 频道页在 flat 模式下返回的是 Videos/Shorts/Live 等 Tab 条目而非视频。
/// 识别后返回应钻取的 Tab 地址（优先 Videos），否则返回 None。
fn find_videos_tab_url(entries: &[serde_json::Value], channel_url: &str) -> Option<String> {
    const TAB_SLUGS: [&str; 6] = ["videos", "shorts", "streams", "playlists", "podcasts", "community"];
    let mut first_tab: Option<(String, String)> = None; // (slug, url)
    let mut videos_tab: Option<String> = None;
    for e in entries {
        let id = e.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let title = e.get("title").and_then(|x| x.as_str()).unwrap_or("");
        let raw_url = e
            .get("webpage_url")
            .and_then(|x| x.as_str())
            .or_else(|| e.get("url").and_then(|x| x.as_str()))
            .unwrap_or("");
        // slug 判定：id 本身 / url 尾段 / 标题后缀 " - Videos"
        let mut slug: Option<String> = None;
        if let Some(t) = TAB_SLUGS.iter().find(|t| id.eq_ignore_ascii_case(t)) {
            slug = Some(t.to_string());
        }
        if slug.is_none() {
            if let Some(seg) = raw_url.split(['?', '#']).next().and_then(|u| u.rsplit('/').next()) {
                if let Some(t) = TAB_SLUGS.iter().find(|t| seg.eq_ignore_ascii_case(t)) {
                    slug = Some(t.to_string());
                }
            }
        }
        if slug.is_none() {
            if let Some(suffix) = title.rsplit(" - ").next() {
                let s = suffix.to_lowercase();
                if s == "videos" {
                    slug = Some("videos".into());
                } else if s == "shorts" {
                    slug = Some("shorts".into());
                } else if s == "live" || s == "streams" {
                    // Live Tab 的地址是 /streams
                    slug = Some("streams".into());
                } else if s == "playlists" || s == "podcasts" || s == "community" {
                    slug = Some(s);
                }
            }
        }
        let slug = match slug {
            Some(s) => s,
            None => continue,
        };
        let full = if raw_url.starts_with("http") {
            raw_url.to_string()
        } else {
            let base = channel_url
                .split(['?', '#'])
                .next()
                .unwrap_or(channel_url)
                .trim_end_matches('/');
            // 去掉 base 自带的 tab 后缀，避免 /videos/videos
            let base = TAB_SLUGS.iter().fold(base.to_string(), |acc, t| {
                acc.strip_suffix(&format!("/{t}"))
                    .map(|s| s.to_string())
                    .unwrap_or(acc)
            });
            format!("{base}/{slug}")
        };
        if first_tab.is_none() {
            first_tab = Some((slug.clone(), full.clone()));
        }
        if slug == "videos" {
            videos_tab = Some(full);
            break;
        }
    }
    // 优先 Videos Tab；没有就退回第一个 Tab
    videos_tab.or_else(|| first_tab.map(|(_, u)| u))
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

    let mut json = fetch_flat_json(&app, &bin, &url, max).await?;

    // 频道页在 flat 模式下返回 Videos/Shorts/Live 等 Tab 条目而非视频时，
    // 自动钻取 Videos Tab（一层，且地址不同才钻，避免循环）
    let jtype_first = json.get("_type").and_then(|x| x.as_str()).unwrap_or("video");
    if jtype_first != "video" && json.get("formats").is_none() {
        if let Some(entries) = json.get("entries").and_then(|e| e.as_array()) {
            if let Some(tab_url) = find_videos_tab_url(entries, &url) {
                if tab_url != url {
                    json = fetch_flat_json(&app, &bin, &tab_url, max).await?;
                }
            }
        }
    }


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

/// storyboard 等不可下载的伪格式，不展示
fn is_pseudo_format(f: &serde_json::Value) -> bool {
    if f.get("protocol").and_then(|p| p.as_str()) == Some("mhtml") {
        return true;
    }
    if f
        .get("format_id")
        .and_then(|id| id.as_str())
        .map(|id| id.starts_with("sb"))
        .unwrap_or(false)
    {
        return true;
    }
    false
}

fn has_video(f: &serde_json::Value) -> bool {
    matches!(
        f.get("vcodec").and_then(|v| v.as_str()),
        Some(v) if v != "none"
    )
}

fn has_audio(f: &serde_json::Value) -> bool {
    matches!(
        f.get("acodec").and_then(|a| a.as_str()),
        Some(a) if a != "none"
    )
}

fn num_height(f: &serde_json::Value) -> u64 {
    f.get("height").and_then(|h| h.as_u64()).unwrap_or(0)
}

fn num_tbr(f: &serde_json::Value) -> f64 {
    f.get("tbr").and_then(|t| t.as_f64()).unwrap_or(0.0)
}

/// 过滤伪格式并按画质从高到低排序：
/// 含视频轨在前（高度降序→码率降序→合并流优先），纯音频沉底（码率降序）
fn prepare_formats(formats: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    let mut sorted: Vec<&serde_json::Value> =
        formats.iter().filter(|f| !is_pseudo_format(f)).collect();
    sorted.sort_by(|a, b| {
        let ga = if has_video(a) { 0u8 } else { 1u8 };
        let gb = if has_video(b) { 0u8 } else { 1u8 };
        ga.cmp(&gb)
            .then_with(|| num_height(b).cmp(&num_height(a)))
            .then_with(|| num_tbr(b).total_cmp(&num_tbr(a)))
            .then_with(|| (has_audio(b) as u8).cmp(&(has_audio(a) as u8)))
    });
    sorted
}

#[tauri::command]
pub async fn list_formats(app: AppHandle, url: String) -> Result<Vec<FormatItem>, String> {
    let (bin, _) = locate_ytdlp(&app);
    let bin = bin.ok_or("yt-dlp 未就绪")?;
    let settings = get_settings(app.clone()).unwrap_or_default();
    let mut cmd = tokio::process::Command::new(&bin);
    hide_tokio(&mut cmd);
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
    let out = tokio::time::timeout(std::time::Duration::from_secs(60), cmd.output())
        .await
        .map_err(|_| "获取格式超时（60秒），请重试".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(crate::errhint::friendly(
            "获取格式失败",
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("解析失败: {e}"))?;
    let formats = json
        .get("formats")
        .and_then(|f| f.as_array())
        .ok_or("该视频无可用格式")?;
    Ok(prepare_formats(formats)
        .into_iter()
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
            height: f
                .get("height")
                .and_then(|x| x.as_u64())
                .map(|h| h as u32),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 仿 YouTube 真实 -J 结构：storyboard + 音频 + 低清 + 合并流 + 高清
    fn fixture() -> Vec<serde_json::Value> {
        json!([
            {"format_id": "sb0", "ext": "mhtml", "protocol": "mhtml", "vcodec": "none", "acodec": "none"},
            {"format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "tbr": 129.0},
            {"format_id": "251", "ext": "webm", "vcodec": "none", "acodec": "opus", "tbr": 160.0},
            {"format_id": "133", "ext": "mp4", "vcodec": "avc1.4d4015", "acodec": "none", "height": 240, "tbr": 300.0},
            {"format_id": "22", "ext": "mp4", "vcodec": "avc1.64001F", "acodec": "mp4a.40.2", "height": 720, "tbr": 2000.0},
            {"format_id": "137", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080, "tbr": 4500.0},
            {"format_id": "248", "ext": "webm", "vcodec": "vp9", "acodec": "none", "height": 1080, "tbr": 8500.0}
        ])
        .as_array()
        .unwrap()
        .clone()
    }

    #[test]
    fn storyboard_filtered_and_sorted_best_first() {        let data = fixture();
        let out = prepare_formats(&data);
        let ids: Vec<&str> = out
            .iter()
            .map(|f| f.get("format_id").and_then(|x| x.as_str()).unwrap())
            .collect();
        // sb0 被过滤；1080p 在前（同高度按码率，vp9 优先）；合并流 22 排在纯视频 133 前因高度更高；
        // 纯音频沉底按码率
        assert_eq!(ids, vec!["248", "137", "22", "133", "251", "140"]);
    }

    /// 频道页返回 Tab 条目时，能定位到 Videos Tab 地址
    #[test]
    fn channel_tabs_drill_to_videos() {
        let entries = json!([
            {"id": "videos", "title": "ReiRei_ - Videos", "url": "https://www.youtube.com/@ReiRei_/videos"},
            {"id": "streams", "title": "ReiRei_ - Live", "url": "https://www.youtube.com/@ReiRei_/streams"},
            {"id": "shorts", "title": "ReiRei_ - Shorts", "url": "https://www.youtube.com/@ReiRei_/shorts"}
        ]);
        let entries = entries.as_array().unwrap();
        assert_eq!(
            find_videos_tab_url(entries, "https://www.youtube.com/@ReiRei_"),
            Some("https://www.youtube.com/@ReiRei_/videos".to_string())
        );
        // 没有 Videos Tab 时退回第一个 Tab；纯 id 形式也能拼出地址
        let entries2 = json!([
            {"id": "shorts", "title": "X - Shorts", "url": "shorts"}
        ]);
        let entries2 = entries2.as_array().unwrap();
        assert_eq!(
            find_videos_tab_url(entries2, "https://www.youtube.com/@X"),
            Some("https://www.youtube.com/@X/shorts".to_string())
        );
        // 普通视频条目不触发
        let entries3 = json!([
            {"id": "dQw4w9WgXcQ", "title": "Some video", "url": "dQw4w9WgXcQ"}
        ]);
        let entries3 = entries3.as_array().unwrap();
        assert_eq!(
            find_videos_tab_url(entries3, "https://www.youtube.com/@X"),
            None
        );
    }
}
