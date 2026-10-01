//! 下载执行：spawn yt-dlp，长耗时任务通过 window 事件推送进度
//! 事件名：download-progress
//! 预设映射见 map_selector()

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::cookies::cookie_file_for;
use crate::ytdlp::locate_ytdlp;

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub format_selector: String,
    pub out_dir: String,
    pub cookie_profile: Option<String>,
    pub concurrent_fragments: u32,
    pub proxy: Option<String>,
    pub filename_template: String,
    pub task_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub task_id: String,
    pub url: String,
    pub status: String,
    pub percent: Option<f64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub line: Option<String>,
}

static CANCEL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn cancel_download() -> Result<(), String> {
    CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

/// 预设 → yt-dlp 参数
fn map_selector(sel: &str) -> (String, Vec<String>) {
    if let Some(id) = sel.strip_prefix("format_id:") {
        return (id.trim().to_string(), vec![]);
    }
    match sel {
        "best" => ("bv*+ba/b".into(), vec![]),
        "best1080" => (
            "bv*[height<=1080]+ba/b[height<=1080]/b".into(),
            vec![],
        ),
        "best720" => ("bv*[height<=720]+ba/b[height<=720]/b".into(), vec![]),
        "best480" => ("bv*[height<=480]+ba/b[height<=480]/b".into(), vec![]),
        "audio_mp3" => (
            "ba/b".into(),
            vec![
                "--extract-audio".into(),
                "--audio-format".into(),
                "mp3".into(),
            ],
        ),
        "audio_m4a" => (
            "ba/b".into(),
            vec![
                "--extract-audio".into(),
                "--audio-format".into(),
                "m4a".into(),
            ],
        ),
        other => (other.to_string(), vec![]),
    }
}

#[tauri::command]
pub async fn start_download(app: AppHandle, request: DownloadRequest) -> Result<String, String> {
    CANCEL.store(false, Ordering::SeqCst);
    let (bin, _) = locate_ytdlp(&app);
    let bin = bin.ok_or("yt-dlp 未就绪，请先下载内置 yt-dlp")?;

    std::fs::create_dir_all(&request.out_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    let (fmt, extra) = map_selector(request.format_selector.trim());
    let task_id = request
        .task_label
        .clone()
        .unwrap_or_else(|| format!("{}", chrono::Local::now().format("%H%M%S")));

    let out_template = format!(
        "{}/{}",
        request.out_dir.trim_end_matches(['/', '\\']),
        if request.filename_template.trim().is_empty() {
            "%(title)s [%(id)s].%(ext)s"
        } else {
            request.filename_template.trim()
        }
    );

    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args([
        "--newline",
        "--progress",
        "--no-playlist",
        "-f",
        &fmt,
        "-o",
        &out_template,
        "--merge-output-format",
        "mp4",
        "--concurrent-fragments",
        &request.concurrent_fragments.clamp(1, 16).to_string(),
        "--socket-timeout",
        "20",
        "--retries",
        "3",
    ]);
    for a in &extra {
        cmd.arg(a);
    }
    // cookie：请求级优先，否则用默认
    let cookie_name = request.cookie_profile.clone().or_else(|| {
        crate::settings::get_settings(app.clone())
            .ok()
            .and_then(|s| s.default_cookie_profile)
    });
    if let Some(name) = cookie_name {
        if let Ok(p) = cookie_file_for(&app, &name) {
            if p.is_file() {
                cmd.arg("--cookies").arg(p);
            }
        }
    }
    let proxy = request.proxy.clone().or_else(|| {
        crate::settings::get_settings(app.clone())
            .ok()
            .and_then(|s| s.proxy)
    });
    if let Some(px) = proxy {
        if !px.trim().is_empty() {
            cmd.arg("--proxy").arg(px.trim().to_string());
        }
    }
    cmd.arg(&request.url);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动 yt-dlp 失败: {e}"))?;
    let emit = |payload: DownloadProgress| {
        let _ = app.emit("download-progress", payload);
    };
    emit(DownloadProgress {
        task_id: task_id.clone(),
        url: request.url.clone(),
        status: "started".into(),
        percent: Some(0.0),
        speed: None,
        eta: None,
        line: Some(format!("开始下载 {}", request.url)),
    });

    // 进度正则：[download]  12.3% of ~... at ... ETA ...
    let re = Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%.*?at\s+(\S+).*?ETA\s+(\S+)").unwrap();
    let re_simple = Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%").unwrap();

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        // 借用 app 发送事件需要 clone AppHandle
        let app2 = app.clone();
        let url2 = request.url.clone();
        let tid = task_id.clone();
        while let Ok(Some(line)) = lines.next_line().await {
            if CANCEL.load(Ordering::SeqCst) {
                let _ = child.kill().await;
                let _ = app2.emit(
                    "download-progress",
                    DownloadProgress {
                        task_id: tid.clone(),
                        url: url2.clone(),
                        status: "error".into(),
                        percent: None,
                        speed: None,
                        eta: None,
                        line: Some("已取消".into()),
                    },
                );
                return Err("已取消下载".into());
            }
            let line_t = line.trim().to_string();
            if line_t.is_empty() {
                continue;
            }
            let (pct, speed, eta) = if let Some(c) = re.captures(&line_t) {
                (
                    c.get(1).and_then(|m| m.as_str().parse().ok()),
                    c.get(2).map(|m| m.as_str().to_string()),
                    c.get(3).map(|m| m.as_str().to_string()),
                )
            } else if let Some(c) = re_simple.captures(&line_t) {
                (c.get(1).and_then(|m| m.as_str().parse().ok()), None, None)
            } else {
                (None, None, None)
            };
            // 只转发有意义的行，避免刷屏：进度行 + 关键行
            let interesting = pct.is_some()
                || line_t.contains("[Merger]")
                || line_t.contains("[ExtractAudio]")
                || line_t.contains("Destination:")
                || line_t.to_lowercase().contains("error");
            if interesting {
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        task_id: task_id.clone(),
                        url: request.url.clone(),
                        status: "progress".into(),
                        percent: pct,
                        speed,
                        eta,
                        line: Some(line_t.clone()),
                    },
                );
            }
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    // stderr 兜底错误信息
    if status.success() {
        emit(DownloadProgress {
            task_id: task_id.clone(),
            url: request.url.clone(),
            status: "finished".into(),
            percent: Some(100.0),
            speed: None,
            eta: None,
            line: Some("下载完成".into()),
        });
        Ok("ok".into())
    } else {
        emit(DownloadProgress {
            task_id: task_id.clone(),
            url: request.url.clone(),
            status: "error".into(),
            percent: None,
            speed: None,
            eta: None,
            line: Some(format!("yt-dlp 退出码: {}", status.code().unwrap_or(-1))),
        });
        Err(format!("yt-dlp 退出码: {}", status.code().unwrap_or(-1)))
    }
}

#[allow(dead_code)]
pub fn _keep_arc(_: Arc<()>) {}
