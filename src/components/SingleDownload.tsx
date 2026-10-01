import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  TAURI_COMMANDS, AppSettings, FormatItem, ResolvedMedia, DownloadProgress, FORMAT_PRESETS, formatBytes,
} from "../types";

export default function SingleDownload({ settings, onSettingsChange }: { settings: AppSettings | null; onSettingsChange: () => void }) {
  const [url, setUrl] = useState("");
  const [resolving, setResolving] = useState(false);
  const [media, setMedia] = useState<ResolvedMedia | null>(null);
  const [formats, setFormats] = useState<FormatItem[]>([]);
  const [loadingFormats, setLoadingFormats] = useState(false);
  const [preset, setPreset] = useState<string>(settings?.default_format ?? "best");
  const [manualId, setManualId] = useState("");
  const [outDir, setOutDir] = useState(settings?.out_dir ?? "");
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [msg, setMsg] = useState<string | null>(null);

  const pushLog = (s: string) => setLog((prev) => [...prev.slice(-199), s]);

  const resolve = async () => {
    if (!url.trim()) return;
    setResolving(true); setMsg(null);
    try {
      const r = await invoke<ResolvedMedia>(TAURI_COMMANDS.resolveUrl, { url: url.trim(), maxEntries: 5 });
      setMedia(r);
      pushLog(`解析成功：${r.title ?? r.kind} (${r.kind})`);
      if (r.kind === "video") await loadFormats(url.trim()); else setFormats([]);
    } catch (e) { setMsg(`解析失败：${String(e)}`); }
    finally { setResolving(false); }
  };

  const loadFormats = async (target?: string) => {
    const u = (target ?? url).trim(); if (!u) return;
    setLoadingFormats(true);
    try {
      const f = await invoke<FormatItem[]>(TAURI_COMMANDS.listFormats, { url: u });
      setFormats(f); pushLog(`获取到 ${f.length} 个格式`);
    } catch (e) { setMsg(`获取格式失败：${String(e)}`); }
    finally { setLoadingFormats(false); }
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") setOutDir(dir);
  };

  const selector = preset === "manual" ? `format_id:${manualId.trim()}` : preset;

  const download = async () => {
    if (!url.trim() || !outDir.trim()) { setMsg("请填写链接和输出目录"); return; }
    setMsg(null); setProgress(null);
    const unlisten = await listen<DownloadProgress>("download-progress", (ev) => {
      setProgress(ev.payload); if (ev.payload.line) pushLog(ev.payload.line);
    });
    try {
      pushLog(`开始下载：${url.trim()} [${selector}]`);
      await invoke(TAURI_COMMANDS.startDownload, {
        request: {
          url: url.trim(), format_selector: selector, out_dir: outDir.trim(),
          cookie_profile: settings?.default_cookie_profile ?? null,
          concurrent_fragments: settings?.concurrent_fragments ?? 4,
          proxy: settings?.proxy ?? null,
          filename_template: settings?.filename_template ?? "%(title)s [%(id)s].%(ext)s",
        },
      });
      pushLog("下载命令已完成"); onSettingsChange();
    } catch (e) { setMsg(`下载失败：${String(e)}`); pushLog(`ERROR: ${String(e)}`); }
    finally { unlisten(); }
  };

  return (
    <div>
      <div className="card">
        <h2 className="card-title"><span className="card-step">1</span>解析链接</h2>
        <p className="card-desc">支持单个视频、播放列表和频道主页链接（YouTube / Bilibili 等 yt-dlp 支持站点）。</p>
        <div className="row">
          <input type="text" placeholder="https://www.youtube.com/watch?v=…" value={url} onChange={(e) => setUrl(e.target.value)} />
          <button className="btn" onClick={resolve} disabled={resolving}>{resolving ? "解析中…" : "解析"}</button>
          <button className="btn btn-ghost" onClick={() => loadFormats()} disabled={loadingFormats}>{loadingFormats ? "读取中…" : "刷新格式"}</button>
        </div>
        {media && (
          <div className="row" style={{ marginTop: 14 }}>
            <span className="pill ok">{media.kind}</span>
            <span style={{ fontWeight: 600 }}>{media.title}</span>
            {media.uploader && <span className="muted">· {media.uploader}</span>}
          </div>
        )}
      </div>

      <div className="card">
        <h2 className="card-title"><span className="card-step">2</span>选择画质 / 音频</h2>
        <div className="row">
          <select value={preset} onChange={(e) => setPreset(e.target.value)} style={{ maxWidth: 240 }}>
            {FORMAT_PRESETS.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
          </select>
          {preset === "manual" && (
            <input type="text" placeholder="Format ID，如 137+140" value={manualId} onChange={(e) => setManualId(e.target.value)} style={{ maxWidth: 220, flex: "0 0 auto" }} />
          )}
          <span className="pill">-f {selector}</span>
        </div>
        {formats.length > 0 ? (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>ID</th><th>扩展</th><th>分辨率</th><th>编码</th><th>大小</th><th>备注</th></tr>
              </thead>
              <tbody>
                {formats.map((f) => (
                  <tr key={f.format_id} className={manualId === f.format_id ? "sel" : ""} onClick={() => { setPreset("manual"); setManualId(f.format_id); }}>
                    <td>{f.format_id}</td><td>{f.ext}</td><td>{f.resolution ?? "-"}</td>
                    <td>{f.vcodec ?? "-"} / {f.acodec ?? "-"}</td>
                    <td>{formatBytes(f.filesize ?? f.filesize_approx)}</td><td>{f.format_note ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="muted" style={{ marginTop: 12 }}>暂无格式数据，点击「解析」或「刷新格式」获取（调用 yt-dlp -J）。</p>
        )}
      </div>

      <div className="card">
        <h2 className="card-title"><span className="card-step">3</span>开始下载</h2>
        <div className="row">
          <input type="text" placeholder="选择输出目录" value={outDir} onChange={(e) => setOutDir(e.target.value)} />
          <button className="btn btn-ghost" onClick={pickDir}>浏览…</button>
          <button className="btn btn-success" onClick={download}>开始下载</button>
        </div>
        <div className="muted" style={{ marginTop: 8 }}>
          Cookie：{settings?.default_cookie_profile ?? "未使用"} · 并发 {settings?.concurrent_fragments ?? 4}
          {settings?.proxy ? ` · 代理 ${settings.proxy}` : ""}
        </div>
        {progress && (
          <div style={{ marginTop: 16 }}>
            <div className="row" style={{ marginBottom: 8 }}>
              <span className="pill ok">{progress.status}</span>
              {progress.percent != null && <span style={{ fontWeight: 700 }}>{progress.percent.toFixed(1)}%</span>}
              {progress.speed && <span className="muted">{progress.speed}</span>}
              {progress.eta && <span className="muted">剩余 {progress.eta}</span>}
            </div>
            <div className="progress"><div style={{ width: `${progress.percent ?? 0}%` }} /></div>
          </div>
        )}
        {msg && <div className="alert alert-error" style={{ marginTop: 14, marginBottom: 0 }}>{msg}</div>}
        {log.length > 0 && <div className="log" style={{ marginTop: 14 }}>{log.join("\n")}</div>}
      </div>
    </div>
  );
}
