//! 下载执行：spawn yt-dlp，长耗时任务通过 window 事件推送进度
//! 事件名：download-progress
//! 预设映射见 map_selector()

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::cookies::cookie_file_for;
use crate::ytdlp::{hide_tokio, locate_ytdlp};

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

static RE_FULL: OnceLock<Regex> = OnceLock::new();
static RE_SIMPLE: OnceLock<Regex> = OnceLock::new();

fn re_full() -> &'static Regex {
    RE_FULL.get_or_init(|| {
        Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%.*?at\s+(\S+).*?ETA\s+(\S+)").unwrap()
    })
}
fn re_simple() -> &'static Regex {
    RE_SIMPLE
        .get_or_init(|| Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%").unwrap())
}

fn parse_progress(line: &str) -> (Option<f64>, Option<String>, Option<String>) {
    if let Some(c) = re_full().captures(line) {
        (
            c.get(1).and_then(|m| m.as_str().parse().ok()),
            c.get(2).map(|m| m.as_str().to_string()),
            c.get(3).map(|m| m.as_str().to_string()),
        )
    } else if let Some(c) = re_simple().captures(line) {
        (c.get(1).and_then(|m| m.as_str().parse().ok()), None, None)
    } else {
        (None, None, None)
    }
}

fn is_interesting(line: &str, pct: Option<f64>) -> bool {
    pct.is_some()
        || line.contains("[Merger]")
        || line.contains("[ExtractAudio]")
        || line.contains("Destination:")
        || line.to_lowercase().contains("error")
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
    if fmt.is_empty() {
        return Err("Format ID 为空，请先选择一个格式".into());
    }
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
    hide_tokio(&mut cmd);
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
    // 防止闪现 CMD 窗口由 hide_tokio 处理；此处再确保不继承控制台
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| format!("启动 yt-dlp 失败: {e}"))?;
    let child = Arc::new(tokio::sync::Mutex::new(child));
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

    // stderr 兜底：保留最后 30 行用于失败时定位原因
    let stderr_tail: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(30)));
    // 进度节流：避免每秒几十次 emit 卡死前端
    let last_emit: Arc<Mutex<Instant>> =
        Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));

    let (mut stdout_taken, mut stderr_taken) = {
        let mut g = child.lock().await;
        (g.stdout.take(), g.stderr.take())
    };

    // 取消监听：轮询标志并 kill，避免 cancel 后还等到进程自然退出
    let child_for_cancel = child.clone();
    let cancel_watcher = tokio::spawn(async move {
        loop {
            if CANCEL.load(Ordering::SeqCst) {
                let mut g = child_for_cancel.lock().await;
                let _ = g.kill().await;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    let app_out = app.clone();
    let task_out = task_id.clone();
    let url_out = request.url.clone();
    let tail_out = stderr_tail.clone();
    let emit_out = last_emit.clone();
    let stdout_task = tokio::spawn(async move {
        let Some(stdout) = stdout_taken.take() else {
            return;
        };
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if CANCEL.load(Ordering::SeqCst) {
                break;
            }
            let line_t = line.trim().to_string();
            if line_t.is_empty() {
                continue;
            }
            // stdout 的非进度行也可能是关键信息，同样收集到 tail 便于诊断
            {
                let mut tail = tail_out.lock().unwrap();
                if tail.len() >= 30 {
                    tail.pop_front();
                }
                tail.push_back(line_t.clone());
            }
            let (pct, speed, eta) = parse_progress(&line_t);
            if !is_interesting(&line_t, pct) {
                continue;
            }
            // 节流：纯进度行 250ms 最多一次，关键行立即推
            if pct.is_some() {
                let mut last = emit_out.lock().unwrap();
                if last.elapsed() < Duration::from_millis(250) && pct.unwrap_or(0.0) < 100.0 {
                    continue;
                }
                *last = Instant::now();
            }
            let _ = app_out.emit(
                "download-progress",
                DownloadProgress {
                    task_id: task_out.clone(),
                    url: url_out.clone(),
                    status: "progress".into(),
                    percent: pct,
                    speed,
                    eta,
                    line: Some(line_t),
                },
            );
        }
    });

    let app_err = app.clone();
    let task_err = task_id.clone();
    let url_err = request.url.clone();
    let tail_err = stderr_tail.clone();
    let emit_err = last_emit.clone();
    let stderr_task = tokio::spawn(async move {
        let Some(stderr) = stderr_taken.take() else {
            return;
        };
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if CANCEL.load(Ordering::SeqCst) {
                break;
            }
            let line_t = line.trim().to_string();
            if line_t.is_empty() {
                continue;
            }
            {
                let mut tail = tail_err.lock().unwrap();
                if tail.len() >= 30 {
                    tail.pop_front();
                }
                tail.push_back(line_t.clone());
            }
            let (pct, speed, eta) = parse_progress(&line_t);
            if !is_interesting(&line_t, pct) {
                continue;
            }
            if pct.is_some() {
                let mut last = emit_err.lock().unwrap();
                if last.elapsed() < Duration::from_millis(250) && pct.unwrap_or(0.0) < 100.0 {
                    continue;
                }
                *last = Instant::now();
            }
            let _ = app_err.emit(
                "download-progress",
                DownloadProgress {
                    task_id: task_err.clone(),
                    url: url_err.clone(),
                    status: "progress".into(),
                    percent: pct,
                    speed,
                    eta,
                    line: Some(line_t),
                },
            );
        }
    });

    let _ = tokio::join!(stdout_task, stderr_task);
    // 读写任务结束后再回收 watcher
    cancel_watcher.abort();

    // 被取消：直接返回，不再 wait 残留进程
    if CANCEL.load(Ordering::SeqCst) {
        // 确保进程已退出
        {
            let mut g = child.lock().await;
            let _ = g.kill().await;
        }
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                task_id: task_id.clone(),
                url: request.url.clone(),
                status: "error".into(),
                percent: None,
                speed: None,
                eta: None,
                line: Some("已取消".into()),
            },
        );
        return Err("已取消下载".into());
    }

    let status = {
        let mut g = child.lock().await;
        g.wait().await.map_err(|e| e.to_string())?
    };
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
        let tail: String = {
            let t = stderr_tail.lock().unwrap();
            t.iter().cloned().collect::<Vec<_>>().join(" | ")
        };
        let tail_short: String = tail.chars().take(800).collect();
        let msg = if tail_short.trim().is_empty() {
            format!("yt-dlp 退出码: {}", status.code().unwrap_or(-1))
        } else {
            format!(
                "yt-dlp 退出码: {}，{}",
                status.code().unwrap_or(-1),
                tail_short
            )
        };
        emit(DownloadProgress {
            task_id: task_id.clone(),
            url: request.url.clone(),
            status: "error".into(),
            percent: None,
            speed: None,
            eta: None,
            line: Some(msg.clone()),
        });
        Err(msg)
    }
}
