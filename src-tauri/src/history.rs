//! 下载历史：记录每次下载结果 + 打开所在目录
//! 存储：%APPDATA%/com.y2b.downloader/history.json（最多 200 条，新在前）
//! 打开目录走后端 explorer 调用，不需要新增前端 capability。

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub out_dir: String,
    pub format_selector: String,
    pub status: String,
    pub detail: Option<String>,
    pub created_at: String,
}

fn history_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("history.json"))
}

fn read_all(app: &AppHandle) -> Vec<HistoryEntry> {
    let Ok(p) = history_path(app) else {
        return vec![];
    };
    if !p.is_file() {
        return vec![];
    }
    let Ok(data) = std::fs::read_to_string(&p) else {
        return vec![];
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn write_all(app: &AppHandle, list: &[HistoryEntry]) -> Result<(), String> {
    let p = history_path(app)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    std::fs::write(&p, data).map_err(|e| e.to_string())
}

/// 由 download 完成后调用，非 command
pub fn push_history(
    app: &AppHandle,
    url: String,
    title: Option<String>,
    out_dir: String,
    format_selector: String,
    status: &str,
    detail: Option<String>,
) {
    let mut list = read_all(app);
    let id = format!(
        "{}_{}",
        Local::now().format("%Y%m%d%H%M%S%f"),
        url.chars().take(8).collect::<String>()
    );
    list.insert(
        0,
        HistoryEntry {
            id,
            url,
            title,
            out_dir,
            format_selector,
            status: status.to_string(),
            detail,
            created_at: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        },
    );
    list.truncate(MAX_ENTRIES);
    let _ = write_all(app, &list);
}

#[tauri::command]
pub fn history_list(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    Ok(read_all(&app))
}

#[tauri::command]
pub fn history_remove(app: AppHandle, id: String) -> Result<(), String> {
    let mut list = read_all(&app);
    list.retain(|e| e.id != id);
    write_all(&app, &list)
}

#[tauri::command]
pub fn history_clear(app: AppHandle) -> Result<(), String> {
    write_all(&app, &[])
}

/// 打开文件/目录所在位置（Windows 用 explorer，macOS/Linux 用 open/xdg-open）
#[tauri::command]
pub fn open_in_folder(path: String) -> Result<(), String> {
    use std::path::Path;
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("路径不存在：{path}"));
    }
    #[cfg(windows)]
    {
        // 文件 -> 选中该文件；目录 -> 直接打开
        let mut cmd = std::process::Command::new("explorer");
        if p.is_file() {
            cmd.args(["/select,", &path]);
        } else {
            cmd.arg(&path);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let target = if p.is_file() {
            p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if p.is_file() {
            p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
