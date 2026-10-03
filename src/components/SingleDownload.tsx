import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { notifyDownload, readClipboardLink } from "../utils";
import {
  TAURI_COMMANDS,
  AppSettings,
  FormatItem,
  ResolvedMedia,
  DownloadProgress,
  FORMAT_PRESETS,
  formatBytes,
  formatRate,
} from "../types";
import { IconLink, IconFilm, IconFolder, IconPlay, IconSearch, IconInfo, IconRefresh } from "./icons";

export default function SingleDownload({
  settings,
  onSettingsChange,
  onGoBatch,
}: {
  settings: AppSettings | null;
  onSettingsChange: () => void;
  onGoBatch?: () => void;
}) {
  const [url, setUrl] = useState("");
  const [resolving, setResolving] = useState(false);
  const [media, setMedia] = useState<ResolvedMedia | null>(null);
  const [formats, setFormats] = useState<FormatItem[]>([]);
  const [loadingFormats, setLoadingFormats] = useState(false);
  const [fq, setFq] = useState("");
  const [preset, setPreset] = useState<string>(settings?.default_format ?? "best");
  const [manualId, setManualId] = useState("");
  const [outDir, setOutDir] = useState(settings?.out_dir ?? "");
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [overwrite, setOverwrite] = useState(false);
  const [clipLink, setClipLink] = useState<string | null>(null);
  // settings 是异步到达的：首挂常为 null。用 touched 标记避免覆盖用户已手动修改的值
  const presetTouched = useRef(false);
  const outDirTouched = useRef(false);
  // 记录「上次同步进来」的默认目录：设置里改了目录要跟随，而不是只填一次空
  const syncedOutDir = useRef<string | null>(null);

  const pushLog = (s: string) => setLog((prev) => [...prev.slice(-199), s]);

  // settings 到达 / 变化后同步默认值（用户未手动改过才同步）
  useEffect(() => {
    if (!settings) return;
    if (!presetTouched.current && settings.default_format) {
      setPreset(settings.default_format);
    }
    if (!outDirTouched.current) {
      const next = settings.out_dir ?? "";
      if (syncedOutDir.current !== next) {
        syncedOutDir.current = next;
        if (next) setOutDir(next);
      }
    }
  }, [settings]);

  // 剪贴板嗅探：首次挂载时读一次，有链接且输入框为空才提示
  useEffect(() => {
    let alive = true;
    readClipboardLink().then((l) => {
      if (alive && l) setClipLink((prev) => prev ?? l);
    });
    return () => {
      alive = false;
    };
  }, []);

  const resolve = async () => {
    if (!url.trim()) return;
    setResolving(true);
    setMsg(null);
    try {
      const r = await invoke<ResolvedMedia>(TAURI_COMMANDS.resolveUrl, {
        url: url.trim(),
        maxEntries: 5,
      });
      setMedia(r);
      pushLog(`解析成功：${r.title ?? r.kind} (${r.kind})`);
      if (r.kind === "video") await loadFormats(url.trim());
      else setFormats([]);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setResolving(false);
    }
  };

  const loadFormats = async (target?: string) => {
    const u = (target ?? url).trim();
    if (!u) return;
    setLoadingFormats(true);
    try {
      const f = await invoke<FormatItem[]>(TAURI_COMMANDS.listFormats, { url: u });
      setFormats(f);
      pushLog(`获取到 ${f.length} 个格式`);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setLoadingFormats(false);
    }
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      outDirTouched.current = true;
      setOutDir(dir);
    }
  };

  const selector = preset === "manual" ? `format_id:${manualId.trim()}` : preset;

  const cancel = async () => {
    try {
      await invoke(TAURI_COMMANDS.cancelDownload);
      pushLog("已发送取消请求…");
    } catch (e) {
      pushLog(`取消失败：${String(e)}`);
    }
  };

  const download = async () => {
    if (downloading) return;
    // 频道 / 播放列表链接在单视频页没有意义：后端只下 1 条，整批请走「创作者批量」
    if (media && media.kind !== "video") {
      setMsg(
        `这是一个${media.kind === "channel" ? "频道" : "播放列表"}链接（共 ${
          media.video_count ?? media.entries_preview.length
        } 个视频），单视频页只会下载第 1 个。整批下载请到「创作者批量」扫描后勾选。`
      );
      return;
    }
    // 输入框为空时回退到设置里的默认输出目录（与批量页一致）
    const effectiveOutDir = outDir.trim() || settings?.out_dir?.trim() || "";
    if (!url.trim() || !effectiveOutDir) {
      setMsg("请填写链接和输出目录");
      return;
    }
    setMsg(null);
    setProgress(null);
    setDownloading(true);
    const unlisten = await listen<DownloadProgress>("download-progress", (ev) => {
      setProgress(ev.payload);
      if (ev.payload.line) pushLog(ev.payload.line);
    });
    try {
      pushLog(`开始下载：${url.trim()} [${selector}]`);
      const result = await invoke<string>(TAURI_COMMANDS.startDownload, {
        request: {
          url: url.trim(),
          format_selector: selector,
          out_dir: effectiveOutDir,
          cookie_profile: settings?.default_cookie_profile ?? null,
          concurrent_fragments: settings?.concurrent_fragments ?? 4,
          proxy: settings?.proxy ?? null,
          filename_template: settings?.filename_template ?? "%(title)s [%(id)s].%(ext)s",
          title: media?.title ?? null,
          overwrite,
          // 单视频语义：即便链接其实是频道/播放列表，也只下 1 条，不会灌满整个目录
          playlist_limit: 1,
        },
      });
      if (result === "skipped") {
        pushLog("文件已存在，已跳过（想重下请勾选「覆盖」）");
      } else {
        pushLog("下载命令已完成");
        notifyDownload("Y2B 下载完成", media?.title || url.trim());
      }
      onSettingsChange();
    } catch (e) {
      const msg = String(e);
      setMsg(msg);
      pushLog(`ERROR: ${msg}`);
      if (!msg.includes("已取消")) notifyDownload("Y2B 下载失败", (media?.title || url.trim()).slice(0, 100));
    } finally {
      unlisten();
      setDownloading(false);
    }
  };

  const shownFormats = formats.filter((f) => {
    const q = fq.trim().toLowerCase();
    if (!q) return true;
    return `${f.format_id} ${f.ext} ${f.resolution ?? ""} ${f.vcodec ?? ""} ${f.acodec ?? ""} ${f.format_note ?? ""}`
      .toLowerCase()
      .includes(q);
  });

  return (
    <div>
      <div className="page-head">
        <h1>新建下载</h1>
        <p>粘贴视频 / 播放列表链接，解析后选择画质并下载。批量下载创作者主页请前往「创作者批量」。</p>
      </div>

      {/* —— 01 链接解析 —— */}
      <div className="card">
        <div className="card-head">
          <span className="step-num">01</span>
          <div>
            <h3>链接解析</h3>
            <p>支持单视频、播放列表、频道 / @handle 链接</p>
          </div>
        </div>
        <div className="row">
          <div className="field grow">
            <input
              className="input"
              type="text"
              placeholder="https://www.youtube.com/watch?v=…"
              value={url}
              onChange={(e) => {
                setUrl(e.target.value);
                // 链接一改，上一次的解析结果就不再可信：
                // 否则会出现「拿着频道的解析态去下视频链接」被误拦的情况
                if (media) {
                  setMedia(null);
                  setFormats([]);
                  setMsg(null);
                }
              }}
              onKeyDown={(e) => e.key === "Enter" && resolve()}
            />
          </div>
          <button className="btn btn-primary" onClick={resolve} disabled={resolving}>
            <IconLink size={15} />
            {resolving ? "解析中…" : "解析"}
          </button>
          <button
            className={`icon-btn ${loadingFormats ? "spinning" : ""}`}
            onClick={() => loadFormats()}
            title="刷新完整格式列表"
          >
            <IconRefresh size={16} />
          </button>
        </div>
        {clipLink && !url.trim() && (
          <div className="callout info mt12">
            <IconInfo size={16} />
            <span className="grow">检测到剪贴板链接：{clipLink.slice(0, 80)}</span>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => {
                setUrl(clipLink);
                setClipLink(null);
              }}
            >
              填入
            </button>
            <button className="btn btn-ghost btn-sm" onClick={() => setClipLink(null)}>
              忽略
            </button>
          </div>
        )}
        {media && (
          <div className="panel-soft mt16">
            <div className="media-hero">
              {media.thumbnail && <img src={media.thumbnail} alt="" />}
              <div className="grow">
                <div className="t">{media.title}</div>
                <div className="m">
                  {media.kind} {media.uploader ? `· ${media.uploader}` : ""}
                </div>
                <div className="mt8">
                  <span className="pill info">{media.kind}</span>{" "}
                  {formats.length > 0 && <span className="pill ok">{formats.length} 个格式</span>}
                </div>
              </div>
            </div>
          </div>
        )}
        {media && media.kind !== "video" && (
          <div className="callout warn mt12">
            <IconInfo size={16} />
            <span className="grow">
              这是{media.kind === "channel" ? "频道" : "播放列表"}（共{" "}
              {media.video_count ?? media.entries_preview.length} 个视频），本页只处理单条视频。
              整批下载请到「创作者批量」扫描后勾选。
            </span>
            {onGoBatch && (
              <button className="btn btn-ghost btn-sm" onClick={onGoBatch}>
                前往创作者批量
              </button>
            )}
          </div>
        )}
      </div>

      {/* —— 02 画质与音频 —— */}
      <div className="card">
        <div className="card-head">
          <span className="step-num">02</span>
          <div>
            <h3>画质与音频</h3>
            <p>快捷预设一键选，或点击下表任意行直接锁定该 Format ID</p>
          </div>
          <div style={{ marginLeft: "auto" }} className="row">
            <IconSearch size={15} />
            <input
              className="input"
              style={{ width: 180, height: 34 }}
              placeholder="筛选格式…"
              value={fq}
              onChange={(e) => setFq(e.target.value)}
            />
          </div>
        </div>
        <div className="seg">
          {FORMAT_PRESETS.map((p) => (
            <button
              key={p.value}
              className={`seg-btn ${preset === p.value ? "active" : ""}`}
              onClick={() => {
                presetTouched.current = true;
                setPreset(p.value);
              }}
            >
              {p.label}
            </button>
          ))}
        </div>
        {preset === "manual" && (
          <div className="row mt12">
            <input
              className="input mono"
              style={{ maxWidth: 260 }}
              placeholder="Format ID，如 137+140"
              value={manualId}
              onChange={(e) => setManualId(e.target.value)}
            />
            <span className="hint">实际 -f 参数：{selector || "(空)"}</span>
          </div>
        )}
        {preset !== "manual" && (
          <div className="hint mt8">实际 -f 参数：{selector}</div>
        )}

        {shownFormats.length > 0 ? (
          <>
          <div className="hint mt8">共 {shownFormats.length} 个可下载格式，已按画质从高到低排序（纯音频沉底）</div>
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>扩展</th>
                  <th>分辨率</th>
                  <th>视频编码</th>
                  <th>音频编码</th>
                  <th>大小</th>
                  <th>码率</th>
                  <th>备注</th>
                </tr>
              </thead>
              <tbody>
                {shownFormats.map((f) => (
                  <tr
                    key={f.format_id}
                    className={manualId === f.format_id ? "sel" : ""}
                    onClick={() => {
                      presetTouched.current = true;
                      setPreset("manual");
                      setManualId(f.format_id);
                    }}
                    title="点击锁定该格式"
                  >
                    <td className="mono">{f.format_id}</td>
                    <td>{f.ext}</td>
                    <td>{f.resolution ?? "-"}</td>
                    <td>{f.vcodec ?? "-"}</td>
                    <td>{f.acodec ?? "-"}</td>
                    <td>{formatBytes(f.filesize ?? f.filesize_approx)}</td>
                    <td>{formatRate(f.tbr)}</td>
                    <td>{f.format_note ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          </>
        ) : (
          <div className="empty mt12">
            <IconFilm size={26} />
            暂无格式数据 — 点击「解析」自动获取，或点右上刷新按钮（yt-dlp -J）。
          </div>
        )}
      </div>

      {/* —— 03 输出与执行 —— */}
      <div className="card">
        <div className="card-head">
          <span className="step-num">03</span>
          <div>
            <h3>输出与执行</h3>
            <p>选择保存目录后开始下载，进度实时显示</p>
          </div>
        </div>
        <div className="row">
          <div className="field grow">
            <input
              className="input"
              placeholder="输出目录"
              value={outDir}
              onChange={(e) => {
                // 清空等于放弃手动选择，重新跟随设置里的默认目录
                outDirTouched.current = e.target.value.trim() !== "";
                setOutDir(e.target.value);
              }}
            />
          </div>
          <button className="btn btn-ghost" onClick={pickDir} disabled={downloading}>
            <IconFolder size={15} />
            选择…
          </button>
          {downloading ? (
            <button className="btn btn-danger" onClick={cancel}>
              取消下载
            </button>
          ) : (
            <button className="btn btn-primary" onClick={download} disabled={resolving}>
              <IconPlay size={15} />
              开始下载
            </button>
          )}
        </div>
        <div className="hint mt8">
          Cookie：{settings?.default_cookie_profile ?? "未使用"} · 客户端：
          {settings?.youtube_player_client ?? "auto"}（18+ 请在「设置」切 mweb
          并装好 PO 插件）
        </div>
        <label className="row mt8" style={{ gap: 8, cursor: "pointer" }}>
          <input
            type="checkbox"
            className="check"
            checked={overwrite}
            onChange={(e) => setOverwrite(e.target.checked)}
          />
          <span className="hint">文件已存在时覆盖重下（不勾选则跳过：中断后重跑自动续传，已下完的不重复下载）</span>
        </label>

        {progress && (
          <div className="mt16">
            <div className="progress-meta">
              <strong>{progress.percent != null ? `${progress.percent.toFixed(1)}%` : progress.status}</strong>
              <span>{progress.speed ?? ""}</span>
              <span>{progress.eta ? `ETA ${progress.eta}` : ""}</span>
            </div>
            <div className="progress">
              <div style={{ width: `${progress.percent ?? 0}%` }} />
            </div>
          </div>
        )}

        {msg && (
          <div className="callout error mt12">
            <IconInfo size={16} />
            <span>{msg}</span>
          </div>
        )}
        {log.length > 0 && <div className="log mt12">{log.join("\n")}</div>}
      </div>
    </div>
  );
}
