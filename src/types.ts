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
  historyList: "history_list",
  historyRemove: "history_remove",
  historyClear: "history_clear",
  openInFolder: "open_in_folder",
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
  height: number | null;
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
  /** yt-dlp -f 参数。预设由后端映射，如 best / best2160 / best1440 / best1080 / audio_mp3 / format_id:<id> */
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
  /** 文件已存在时是否覆盖重下；false = 跳过（断点续传） */
  overwrite: boolean;
  /** 已知标题，用于历史记录展示 */
  title?: string | null;
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
  { value: "best2160", label: "最高 2160p（4K）" },
  { value: "best1440", label: "最高 1440p（2K）" },
  { value: "best1080", label: "最高 1080p" },
  { value: "best720", label: "最高 720p" },
  { value: "best480", label: "最高 480p" },
  { value: "audio_mp3", label: "仅音频 → mp3" },
  { value: "audio_m4a", label: "仅音频 → m4a" },
  { value: "manual", label: "手动指定 Format ID…" },
] as const;

export interface HistoryEntry {
  id: string;
  url: string;
  title: string | null;
  out_dir: string;
  format_selector: string;
  /** ok | error */
  status: string;
  detail: string | null;
  created_at: string;
}

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

/** tbr 单位 Kbps：≥1000 显示 x.xM，否则 xxxk */
export function formatRate(tbr: number | null | undefined): string {
  if (tbr == null || Number.isNaN(tbr) || tbr <= 0) return "-";
  if (tbr >= 1000) return `${(tbr / 1000).toFixed(1)}M`;
  return `${Math.round(tbr)}k`;
}
