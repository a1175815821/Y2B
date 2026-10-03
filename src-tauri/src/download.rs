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
use crate::history::push_history;
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
    /// 前端已知的视频标题，用于历史记录展示；缺省为 None
    #[serde(default)]
    pub title: Option<String>,
    /// 文件已存在时是否覆盖重下；缺省 false = 跳过（配合 --continue 实现断点续传）
    #[serde(default)]
    pub overwrite: Option<bool>,
    /// 播放列表最多下几条；缺省 1（单视频语义）。
    /// 注意：--no-playlist 只挡「URL 同时指向视频与播放列表」的情形，
    /// 纯频道/播放列表 URL 不受它限制（实测会全量下载整个频道），必须靠 --playlist-end 兜底。
    #[serde(default)]
    pub playlist_limit: Option<u32>,
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

/// 单飞锁：同一时刻只允许一个下载任务。
/// CANCEL 是全局量、进度事件也只有 download-progress 一个名字，
/// 两个页面并发下载会互相串扰（取消杀错进程、进度跑到别的页面），直接禁止并发最省事也最安全。
static BUSY: AtomicBool = AtomicBool::new(false);

/// BUSY 的 RAII 守卫：任务正常返回、出错甚至 panic 都会复位，
/// 避免一次异常就把后续所有下载永久锁死（要重启应用才能恢复）。
struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

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
        || line.contains("has already been downloaded")
        || line.to_lowercase().contains("error")
}

/// 预设 → yt-dlp 参数
fn map_selector(sel: &str) -> (String, Vec<String>) {
    if let Some(id) = sel.strip_prefix("format_id:") {
        return (id.trim().to_string(), vec![]);
    }
    match sel {
        "best" => ("bv*+ba/b".into(), vec![]),
        "best2160" => (
            "bv*[height<=2160]+ba/b[height<=2160]/b".into(),
            vec![],
        ),
        "best1440" => (
            "bv*[height<=1440]+ba/b[height<=1440]/b".into(),
            vec![],
        ),
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
    // 单飞：已有任务在跑就直接拒绝，避免全局 CANCEL / 进度事件互相串扰
    if BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("已有下载任务正在进行，请等待完成或先点「取消」".into());
    }
    let _guard = BusyGuard;
    start_download_inner(app, request).await
}

/// PO 失败时的服务状态后缀：帮用户区分“没装 / 没启动 / 跑着还失败（多半 Cookie 或 IP 风控）”
fn po_service_note(app: &AppHandle) -> String {
    if !crate::pot::stack_usable(app) {
        "（PO组件未安装或不完整：到「设置」点一键安装并启动后重试；普通视频保持 auto 即可）"
            .to_string()
    } else if !crate::pot::http_ready() {
        "（PO服务未运行：已尝试脚本兜底仍失败，到「设置」点启动 PO 服务后重试）".to_string()
    } else {
        "（PO服务运行中仍失败：多半是 Cookie 未登录/非成人账号或 IP 被风控，检查 Cookie 并设为默认后重试）"
            .to_string()
    }
}

/// 真正的下载流程；由 start_download 包裹以保证 BUSY 标志一定复位
async fn start_download_inner(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<String, String> {
    start_download_inner_with_client(app, request, None).await
}

/// 单次下载尝试。auto 默认走 yt-dlp 默认客户端（普通视频最稳，不再预先强制 mweb）；
/// 失败且命中 PO/年龄限制特征、用户允许自动切、PO 栈可用时，调用方再用 mweb 调一次（最多一次）。
async fn start_download_inner_with_client(
    app: AppHandle,
    request: DownloadRequest,
    forced_client: Option<String>,
) -> Result<String, String> {
    if forced_client.is_none() {
        CANCEL.store(false, Ordering::SeqCst);
    }
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
    // 播放列表兜底上限：--no-playlist 挡不住纯频道/播放列表 URL（会全量下载），
    // 这里硬性限制条目数。单视频语义下默认 1，批量走单条 URL 不受影响。
    let playlist_limit = request.playlist_limit.unwrap_or(1).clamp(1, 500);
    cmd.args([
        "--newline",
        "--progress",
        "--no-playlist",
        "--playlist-end",
        &playlist_limit.to_string(),
        // 断点续传：同名 .part 文件自动续下（默认行为，此处显式声明）
        "--continue",
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
    // 已存在文件默认跳过不重下：批量中断后重跑只会补缺口，不会覆盖已完成文件；
    // 用户在单视频页勾选“覆盖”时才去掉该保护
    if !request.overwrite.unwrap_or(false) {
        cmd.arg("--no-overwrites");
    }
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
            cmd.arg("--proxy").arg(px.trim());
        }
    }
    // PO-Token / player_client / plugin-dirs：插件目录与脚本兜底常带（按需供 Token，
    // 对普通视频无副作用，见 bgutil 官方“像平常一样用”）；player_client 只在用户显式
    // 选择或 mweb 自动重试时才传，auto 永不预先强制 mweb。
    {
        let mut s = crate::settings::get_settings(app.clone()).unwrap_or_default();
        if let Some(c) = forced_client.clone() {
            s.youtube_player_client = c;
        }
        crate::ytdlp::apply_youtube_options(&mut cmd, &app, &s);
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
        push_history(
            &app,
            request.url.clone(),
            request.title.clone(),
            request.out_dir.clone(),
            request.format_selector.clone(),
            "error",
            Some("已取消".into()),
        );
        return Err("已取消下载".into());
    }

    let status = {
        let mut g = child.lock().await;
        g.wait().await.map_err(|e| e.to_string())?
    };
    if status.success() {
        // --no-overwrites 命中时 yt-dlp 退出码同样为 0，需区分“真下完”和“跳过”
        let skipped = {
            let t = stderr_tail.lock().unwrap();
            t.iter().any(|l| l.contains("has already been downloaded"))
        };
        // 跳过的不能谎报「下载完成」：返回 skipped 让前端区分通知与日志
        let (done_line, result_code, detail) = if skipped {
            ("文件已存在，跳过下载", "skipped", Some("文件已存在，跳过下载".into()))
        } else {
            ("下载完成", "ok", None)
        };
        emit(DownloadProgress {
            task_id: task_id.clone(),
            url: request.url.clone(),
            status: "finished".into(),
            percent: Some(100.0),
            speed: None,
            eta: None,
            line: Some(done_line.into()),
        });
        push_history(
            &app,
            request.url.clone(),
            request.title.clone(),
            request.out_dir.clone(),
            request.format_selector.clone(),
            "ok",
            detail,
        );
        Ok(result_code.into())
    } else {
        let tail: String = {
            let t = stderr_tail.lock().unwrap();
            t.iter().cloned().collect::<Vec<_>>().join("\n")
        };
        // PO 自动重试：auto + 允许 + 栈可用 + 命中 PO/年龄特征 → 用 mweb 再试一次。
        // 注意放在写历史/发 error 事件之前，避免历史里多一条误导性的失败记录。
        if forced_client.is_none()
            && !CANCEL.load(Ordering::SeqCst)
            && crate::ytdlp::is_po_retryable_error(&tail)
            && crate::settings::get_settings(app.clone())
                .map(|s| crate::ytdlp::should_auto_retry_po(&s))
                .unwrap_or(false)
            && crate::pot::stack_usable(&app)
        {
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    task_id: task_id.clone(),
                    url: request.url.clone(),
                    status: "progress".into(),
                    percent: None,
                    speed: None,
                    eta: None,
                    line: Some("默认客户端拿不下（PO/年龄限制特征），正用 mweb 自动重试…".into()),
                },
            );
            return Box::pin(start_download_inner_with_client(
                app.clone(),
                request,
                Some("mweb".into()),
            ))
            .await;
        }
        let mut msg = if tail.trim().is_empty() {
            format!("下载失败：yt-dlp 异常退出（退出码 {}），请重试", status.code().unwrap_or(-1))
        } else {
            format!(
                "下载失败：{}（退出码 {}{}）",
                crate::errhint::friendly_yt_dlp_error(&tail),
                status.code().unwrap_or(-1),
                if forced_client.is_some() { "，已用 mweb" } else { "" },
            )
        };
        // PO 相关失败追加服务状态：没装 / 没启动 / 跑着还失败，三种去向不一样
        if crate::ytdlp::is_po_retryable_error(&tail) {
            msg.push_str(&po_service_note(&app));
        }
        emit(DownloadProgress {
            task_id: task_id.clone(),
            url: request.url.clone(),
            status: "error".into(),
            percent: None,
            speed: None,
            eta: None,
            line: Some(msg.clone()),
        });
        push_history(
            &app,
            request.url.clone(),
            request.title.clone(),
            request.out_dir.clone(),
            request.format_selector.clone(),
            "error",
            Some(msg.clone()),
        );
        Err(msg)
    }
}
