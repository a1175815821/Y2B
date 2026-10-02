//! 设置持久化：%APPDATA%/com.y2b.downloader/settings.json
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

fn default_player_client() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub out_dir: Option<String>,
    pub default_format: String,
    pub concurrent_fragments: u32,
    pub proxy: Option<String>,
    pub filename_template: String,
    pub default_cookie_profile: Option<String>,
    pub ytdlp_version: Option<String>,
    /// YouTube 播放器客户端：auto(=不传，用 yt-dlp 默认) / mweb / web_creator / tv / default,mweb ……
    /// 18+ 视频建议 mweb（需 PO-Token 插件 + 登录 Cookie）。
    #[serde(default = "default_player_client")]
    pub youtube_player_client: String,
    /// 手动 PO-Token，原样透传，如 "mweb.gvs+XXX"。插件模式下留空。
    /// 注意 Token 绑定 videoID，一次一换，仅作逃生通道。
    #[serde(default)]
    pub youtube_po_token: Option<String>,
    /// 自定义 yt-dlp 插件目录（PO-Token Provider 装这里）。
    /// 为空则只用内置默认目录：%APPDATA%/com.y2b.downloader/yt-dlp-plugins
    #[serde(default)]
    pub youtube_plugin_dirs: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            out_dir: None,
            default_format: "best".into(),
            concurrent_fragments: 4,
            proxy: None,
            filename_template: "%(title)s [%(id)s].%(ext)s".into(),
            default_cookie_profile: None,
            ytdlp_version: None,
            youtube_player_client: default_player_client(),
            youtube_po_token: None,
            youtube_plugin_dirs: None,
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("settings.json"))
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let p = settings_path(&app)?;
    if !p.is_file() {
        return Ok(AppSettings::default());
    }
    let data = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let p = settings_path(&app)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&p, data).map_err(|e| e.to_string())?;
    Ok(())
}
