// Y2B 后端入口：注册所有 command
mod cookies;
mod download;
mod errhint;
mod history;
mod media;
mod pot;
mod settings;
mod ytdlp;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // 确保应用数据目录存在
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(dir.join("bin"));
                let _ = std::fs::create_dir_all(dir.join("cookies"));
                // PO-Token Provider 插件目录（yt-dlp --plugin-dirs 指向这里）
                let _ = std::fs::create_dir_all(dir.join("yt-dlp-plugins"));
            }
            // PO 服务后台自启（三件套齐才起，缺件不静默下载）
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::pot::autostart(h).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ytdlp::ytdlp_status,
            ytdlp::ensure_ytdlp,
            ytdlp::check_ytdlp_update,
            ytdlp::update_ytdlp,
            ytdlp::ffmpeg_status,
            ytdlp::ensure_ffmpeg,
            ytdlp::pot_status,
            pot::pot_ensure,
            pot::pot_stop,
            media::resolve_url,
            media::list_formats,
            download::start_download,
            download::cancel_download,
            history::history_list,
            history::history_remove,
            history::history_clear,
            history::open_in_folder,
            cookies::cookie_list,
            cookies::cookie_import,
            cookies::cookie_remove,
            cookies::cookie_set_default,
            cookies::cookie_validate,
            settings::get_settings,
            settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Y2B");
}
