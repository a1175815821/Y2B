export const TAURI_COMMANDS = {
  ytdlpStatus: "ytdlp_status",
  ensureYtdlp: "ensure_ytdlp",
  checkYtdlpUpdate: "check_ytdlp_update",
  updateYtdlp: "update_ytdlp",
  ffmpegStatus: "ffmpeg_status",
  ensureFfmpeg: "ensure_ffmpeg",
  resolveUrl: "resolve_url",
  listFormats: "list_formats",
  startDownload: "start_download",
  cancelDownload: "cancel_download",
  cookieList: "cookie_list",
  cookieImport: "cookie_import",
  cookieRemove: "cookie_remove",
  cookieSetDefault: "cookie_set_default",
  cookieValidate: "cookie_validate",
  getSettings: "get_settings",
  saveSettings: "save_settings",
} as const;

// ---------- yt-dlp 类型 ----------

export interface YtdlpStatus {
  /** 可执行文件绝对路径，未就绪时为 null */
  path: string | null;
  version: string | null;
  ready: boolean;
  /** bundled | downloaded | system | missing */
  source: string;
}

export interface YtdlpUpdateInfo {
  current: string | null;
  latest: string;
  need_update: boolean;
  download_url: string;
  published_at: string | null;
}

export interface FfmpegStatus {
  path: string | null;
  ready: boolean;
  /** bundled | downloaded | system | missing */
  source: string;
}

export interface ResolvedMedia {
  kind: "video" | "playlist" | "channel";
  id: string | null;
  title: string | null;
  uploader: string | null;
  thumbnail: string | null;
  video_count: number | null;
  entries_preview: VideoEntry[];
  /** flat-playlist 截断时为 true，前端提示“仅显示前 N 条” */
  truncated: boolean;
}

export interface VideoEntry {
  id: string;
  title: string | null;
  url: string;
  duration: number | null;
  thumbnail: string | null;
  uploader: string | null;
  upload_date: string | null;
  view_count: number | null;
}

export interface FormatItem {
  format_id: string;
  ext: string;
  resolution: string | null;
  fps: number | null;
  vcodec: string | null;
  acodec: string | null;
  filesize: number | null;
  filesize_approx: number | null;
  tbr: number | null;
  protocol: string | null;
  format_note: string | null;
}

export interface DownloadRequest {
  url: string;
  /** yt-dlp -f 参数。预设由后端映射，最常用：best / best720 / best1080 / audio_mp3 / audio_m4a / format_id:<id> */
  format_selector: string;
  out_dir: string;
  /** cookie profile 名，为空表示不使用 cookie */
  cookie_profile: string | null;
  /** 并发片段 */
  concurrent_fragments: number;
  /** 代理，如 http://127.0.0.1:7890 */
  proxy: string | null;
  /** 文件名模板，默认 %(title)s [%(id)s].%(ext)s */
  filename_template: string;
  /** 批量下载时附带 video id，便于前端区分进度事件 */
  task_label?: string;
}

export interface DownloadProgress {
  task_id: string;
  url: string;
  status: "started" | "progress" | "finished" | "error";
  percent: number | null;
  speed: string | null;
  eta: string | null;
  line: string | null;
}

// ---------- Cookie ----------

export interface CookieProfile {
  name: string;
  path: string;
  updated_at: string | null;
  is_default: boolean;
  /** Netscape 文件里非注释行数（粗略行数） */
  entry_count: number;
}

// ---------- 设置 ----------

export interface AppSettings {
  out_dir: string | null;
  default_format: string;
  concurrent_fragments: number;
  proxy: string | null;
  filename_template: string;
  default_cookie_profile: string | null;
  ytdlp_version: string | null;
}

export const FORMAT_PRESETS = [
  { value: "best", label: "最佳画质（自动合并）" },
  { value: "best1080", label: "最高 1080p" },
  { value: "best720", label: "最高 720p" },
  { value: "best480", label: "最高 480p" },
  { value: "audio_mp3", label: "仅音频 → mp3" },
  { value: "audio_m4a", label: "仅音频 → m4a" },
  { value: "manual", label: "手动指定 Format ID…" },
] as const;

export function formatBytes(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return "-";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v.toFixed(1)} ${units[u]}`;
}
