import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  TAURI_COMMANDS,
  AppSettings,
  ResolvedMedia,
  VideoEntry,
  DownloadProgress,
  FORMAT_PRESETS,
} from "../types";

export default function CreatorBatch({ settings }: { settings: AppSettings | null }) {
  const [channelUrl, setChannelUrl] = useState("");
  const [scanning, setScanning] = useState(false);
  const [media, setMedia] = useState<ResolvedMedia | null>(null);
  const [checked, setChecked] = useState<Record<string, boolean>>({});
  const [preset, setPreset] = useState("best720");
  const [outDir, setOutDir] = useState("");
  const [running, setRunning] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [msg, setMsg] = useState<string | null>(null);

  const pushLog = (s: string) => setLog((p) => [...p.slice(-299), s]);

  const scan = async () => {
    if (!channelUrl.trim()) return;
    setScanning(true);
    setMsg(null);
    try {
      const r = await invoke<ResolvedMedia>(TAURI_COMMANDS.resolveUrl, {
        url: channelUrl.trim(),
        maxEntries: 100,
      });
      setMedia(r);
      const init: Record<string, boolean> = {};
      for (const e of r.entries_preview) init[e.id] = true;
      setChecked(init);
      pushLog(`扫描到 ${r.video_count ?? r.entries_preview.length} 个视频（显示前 ${r.entries_preview.length} 条）`);
    } catch (e) {
      setMsg(`扫描失败：${String(e)}`);
    } finally {
      setScanning(false);
    }
  };

  const toggle = (id: string) => setChecked((p) => ({ ...p, [id]: !p[id] }));
  const all = (v: boolean) => {
    const n: Record<string, boolean> = {};
    for (const e of media?.entries_preview ?? []) n[e.id] = v;
    setChecked(n);
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") setOutDir(dir);
  };

  const selected: VideoEntry[] = (media?.entries_preview ?? []).filter((e) => checked[e.id]);

  const batchDownload = async () => {
    const dir = (outDir || settings?.out_dir || "").trim();
    if (selected.length === 0 || !dir) {
      setMsg("请先勾选视频并选择输出目录");
      return;
    }
    setRunning(true);
    setMsg(null);
    const unlisten = await listen<DownloadProgress>("download-progress", (ev) => {
      if (ev.payload.line) pushLog(`[${ev.payload.task_id}] ${ev.payload.line}`);
    });
    try {
      for (let i = 0; i < selected.length; i++) {
        const v = selected[i];
        pushLog(`(${i + 1}/${selected.length}) 开始：${v.title} ${v.url}`);
        try {
          await invoke(TAURI_COMMANDS.startDownload, {
            request: {
              url: v.url,
              format_selector: preset,
              out_dir: dir,
              cookie_profile: settings?.default_cookie_profile ?? null,
              concurrent_fragments: settings?.concurrent_fragments ?? 4,
              proxy: settings?.proxy ?? null,
              filename_template:
                settings?.filename_template ?? "%(title)s [%(id)s].%(ext)s",
              task_label: v.id,
            },
          });
          pushLog(`(${i + 1}/${selected.length}) 完成`);
        } catch (e) {
          pushLog(`(${i + 1}/${selected.length}) 失败：${String(e)}`);
        }
      }
    } finally {
      unlisten();
      setRunning(false);
    }
  };

  return (
    <div>
      <div className="card">
        <h3>创作者主页 / 频道扫描</h3>
        <div className="row">
          <input
            type="text"
            placeholder="https://www.youtube.com/@handle / /channel/… / /playlist?list=…"
            value={channelUrl}
            onChange={(e) => setChannelUrl(e.target.value)}
          />
          <button onClick={scan} disabled={scanning}>
            {scanning ? "扫描中…" : "扫描视频列表"}
          </button>
        </div>
        {media && (
          <div className="muted" style={{ marginTop: 8 }}>
            {media.title} {media.uploader ? `· ${media.uploader}` : ""} · 共{" "}
            {media.video_count ?? media.entries_preview.length} 个
            {media.truncated ? "（仅显示前 100 条）" : ""}
          </div>
        )}
      </div>

      {media && (
        <div className="card">
          <div className="row">
            <strong>勾选要下载的视频（{selected.length}/{media.entries_preview.length}）</strong>
            <button className="ghost" onClick={() => all(true)}>
              全选
            </button>
            <button className="ghost" onClick={() => all(false)}>
              全不选
            </button>
          </div>
          <div className="row" style={{ marginTop: 10 }}>
            <select value={preset} onChange={(e) => setPreset(e.target.value)}>
              {FORMAT_PRESETS.filter((p) => p.value !== "manual").map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
            <input
              type="text"
              placeholder="输出目录"
              value={outDir || settings?.out_dir || ""}
              onChange={(e) => setOutDir(e.target.value)}
              style={{ maxWidth: 320 }}
            />
            <button className="ghost" onClick={pickDir}>
              选择…
            </button>
            <button onClick={batchDownload} disabled={running}>
              {running ? "下载中…" : `批量下载 (${selected.length})`}
            </button>
          </div>
          <div className="vlist">
            {media.entries_preview.map((v) => (
              <label key={v.id} className="vitem">
                <input type="checkbox" checked={!!checked[v.id]} onChange={() => toggle(v.id)} />
                {v.thumbnail && <img src={v.thumbnail} alt="" />}
                <div>
                  <div className="t">{v.title}</div>
                  <div className="muted">
                    {v.duration != null ? `${Math.round(v.duration / 60)} 分钟 · ` : ""}
                    {v.upload_date ?? ""} {v.view_count != null ? `· ${v.view_count} 播放` : ""}
                  </div>
                </div>
              </label>
            ))}
          </div>
        </div>
      )}

      {msg && <div className="error">{msg}</div>}
      {log.length > 0 && <div className="log">{log.join("\n")}</div>}
    </div>
  );
}
