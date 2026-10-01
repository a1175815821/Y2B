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
import { IconCollection, IconFolder, IconPlay, IconSearch, IconCheck } from "./icons";

export default function CreatorBatch({ settings }: { settings: AppSettings | null }) {
  const [channelUrl, setChannelUrl] = useState("");
  const [scanning, setScanning] = useState(false);
  const [media, setMedia] = useState<ResolvedMedia | null>(null);
  const [checked, setChecked] = useState<Record<string, boolean>>({});
  const [q, setQ] = useState("");
  const [preset, setPreset] = useState("best720");
  const [outDir, setOutDir] = useState("");
  const [running, setRunning] = useState(false);
  const [done, setDone] = useState(0);
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
      setDone(0);
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

  const query = q.trim().toLowerCase();
  const visible = (media?.entries_preview ?? []).filter((e) =>
    query ? (e.title ?? "").toLowerCase().includes(query) : true
  );
  const selected: VideoEntry[] = (media?.entries_preview ?? []).filter((e) => checked[e.id]);

  const cancel = async () => {
    try {
      await invoke(TAURI_COMMANDS.cancelDownload);
      pushLog("已发送取消请求，当前任务完成后将停止…");
    } catch (e) {
      pushLog(`取消失败：${String(e)}`);
    }
  };

  const batchDownload = async () => {
    if (running) return;
    const dir = (outDir || settings?.out_dir || "").trim();
    if (selected.length === 0 || !dir) {
      setMsg("请先勾选视频并选择输出目录");
      return;
    }
    setRunning(true);
    setMsg(null);
    setDone(0);
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
              filename_template: settings?.filename_template ?? "%(title)s [%(id)s].%(ext)s",
              task_label: v.id,
            },
          });
          pushLog(`(${i + 1}/${selected.length}) 完成`);
        } catch (e) {
          const msg = String(e);
          pushLog(`(${i + 1}/${selected.length}) 失败：${msg}`);
          // 用户主动取消：直接中断整个批量，而不是继续下一个（下一个会重置取消标志）
          if (msg.includes("已取消")) {
            pushLog("批量已取消");
            break;
          }
        }
        setDone(i + 1);
      }
    } finally {
      unlisten();
      setRunning(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1>创作者批量</h1>
        <p>输入频道 / @handle / 播放列表主页，扫描视频列表后勾选批量下载。</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="t-ico">
            <IconCollection size={17} />
          </div>
          <div>
            <h3>扫描创作者主页</h3>
            <p>yt-dlp --flat-playlist 快速列出视频（默认前 100 条）</p>
          </div>
        </div>
        <div className="row">
          <div className="field grow">
            <input
              className="input"
              placeholder="https://www.youtube.com/@handle　/　/channel/…　/　/playlist?list=…"
              value={channelUrl}
              onChange={(e) => setChannelUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && scan()}
            />
          </div>
          <button className="btn btn-primary" onClick={scan} disabled={scanning}>
            <IconSearch size={15} />
            {scanning ? "扫描中…" : "扫描视频列表"}
          </button>
        </div>
        {media && (
          <div className="panel-soft mt16">
            <div className="media-hero">
              {media.thumbnail && <img src={media.thumbnail} alt="" />}
              <div className="grow">
                <div className="t">{media.title}</div>
                <div className="m">
                  {media.uploader ?? ""} · 共 {media.video_count ?? media.entries_preview.length} 个
                  {media.truncated ? "（仅显示前 100 条）" : ""}
                </div>
                <div className="stat-row">
                  <div className="stat">
                    <div className="k">扫描到</div>
                    <div className="v">{media.entries_preview.length}</div>
                  </div>
                  <div className="stat">
                    <div className="k">已勾选</div>
                    <div className="v">{selected.length}</div>
                  </div>
                  <div className="stat">
                    <div className="k">已完成</div>
                    <div className="v">
                      {done}/{selected.length}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {media && (
        <div className="card">
          <div className="card-head">
            <div className="t-ico">
              <IconCheck size={17} />
            </div>
            <div>
              <h3>
                勾选视频 <span className="pill info" style={{ marginLeft: 6 }}>{selected.length}/{media.entries_preview.length}</span>
              </h3>
              <p>点击整行即可勾选 / 取消</p>
            </div>
            <div style={{ marginLeft: "auto" }} className="row">
              <input
                className="input"
                style={{ width: 170, height: 34 }}
                placeholder="筛选标题…"
                value={q}
                onChange={(e) => setQ(e.target.value)}
              />
              <button className="btn btn-ghost btn-sm" onClick={() => all(true)}>全选</button>
              <button className="btn btn-ghost btn-sm" onClick={() => all(false)}>全不选</button>
            </div>
          </div>

          <div className="vlist">
            {visible.map((v) => (
              <label key={v.id} className="vitem">
                <input type="checkbox" className="check" checked={!!checked[v.id]} onChange={() => toggle(v.id)} />
                {v.thumbnail && <img src={v.thumbnail} alt="" />}
                <div className="grow">
                  <div className="t">{v.title}</div>
                  <div className="s">
                    {v.duration != null ? `${Math.round(v.duration / 60)} 分钟 · ` : ""}
                    {v.upload_date ?? ""} {v.view_count != null ? `· ${v.view_count} 播放` : ""}
                  </div>
                </div>
              </label>
            ))}
            {visible.length === 0 && <div className="empty">没有匹配的视频，换个关键词试试。</div>}
          </div>

          <div className="actionbar">
            <select className="select" value={preset} onChange={(e) => setPreset(e.target.value)} style={{ width: 210 }}>
              {FORMAT_PRESETS.filter((p) => p.value !== "manual").map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
            <input
              className="input grow"
              placeholder="输出目录"
              value={outDir || settings?.out_dir || ""}
              onChange={(e) => setOutDir(e.target.value)}
            />
            <button className="btn btn-ghost" onClick={pickDir} disabled={running}>
              <IconFolder size={15} />
              选择…
            </button>
            {running ? (
              <button className="btn btn-danger" onClick={cancel}>
                取消 ({done}/{selected.length})
              </button>
            ) : (
              <button className="btn btn-primary" onClick={batchDownload} disabled={scanning}>
                <IconPlay size={15} />
                {`批量下载 (${selected.length})`}
              </button>
            )}
          </div>
        </div>
      )}

      {msg && (
        <div className="callout error">{msg}</div>
      )}
      {log.length > 0 && <div className="log mt16">{log.join("\n")}</div>}
    </div>
  );
}
