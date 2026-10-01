import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { TAURI_COMMANDS, AppSettings, YtdlpStatus, YtdlpUpdateInfo, FORMAT_PRESETS } from "../types";

export default function SettingsPanel({ settings, ytdlp, onChange }: { settings: AppSettings | null; ytdlp: YtdlpStatus | null; onChange: () => void }) {
  const [form, setForm] = useState<AppSettings | null>(settings);
  const [msg, setMsg] = useState<string | null>(null);
  const [update, setUpdate] = useState<YtdlpUpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);

  if (settings && !form) setForm(settings);
  const set = (k: keyof AppSettings, v: unknown) => setForm((p) => (p ? { ...p, [k]: v } : p));

  const save = async () => {
    if (!form) return;
    await invoke(TAURI_COMMANDS.saveSettings, { settings: form });
    setMsg("设置已保存"); onChange();
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && form) setForm({ ...form, out_dir: dir });
  };

  const ensureYtdlp = async () => {
    setMsg("正在下载内置 yt-dlp…");
    try { const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.ensureYtdlp); setMsg(`yt-dlp 就绪：${s.version}`); onChange(); }
    catch (e) { setMsg(`失败：${String(e)}`); }
  };

  const checkUpdate = async () => {
    setChecking(true); setMsg(null);
    try {
      const info = await invoke<YtdlpUpdateInfo>(TAURI_COMMANDS.checkYtdlpUpdate);
      setUpdate(info);
      setMsg(info.need_update ? `发现新版 yt-dlp：${info.current} → ${info.latest}` : `yt-dlp 已是最新（${info.current}）`);
    } catch (e) { setMsg(`检查失败：${String(e)}`); }
    finally { setChecking(false); }
  };

  const doUpdate = async () => {
    setMsg("正在更新 yt-dlp…");
    try { const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.updateYtdlp); setMsg(`已更新到 ${s.version}`); onChange(); }
    catch (e) { setMsg(`更新失败：${String(e)}`); }
  };

  if (!form) return <div className="muted">加载设置中…</div>;

  return (
    <div>
      <div className="grid2">
        <div className="card">
          <h2 className="card-title">内置 yt-dlp</h2>
          <div className="row">
            <span className={`pill ${ytdlp?.ready ? "ok" : "warn"}`}>{ytdlp?.ready ? `就绪 ${ytdlp.version}` : "未就绪"}</span>
            <span className="muted">来源：{ytdlp?.source}</span>
          </div>
          <p className="muted" style={{ wordBreak: "break-all", margin: "10px 0 0" }}>{ytdlp?.path ?? "未找到可执行文件"}</p>
          <div className="row" style={{ marginTop: 14 }}>
            <button className="btn" onClick={ensureYtdlp}>下载 / 修复</button>
            <button className="btn btn-ghost" onClick={checkUpdate} disabled={checking}>{checking ? "检查中…" : "检查更新"}</button>
            {update?.need_update && <button className="btn btn-success" onClick={doUpdate}>更新到 {update.latest}</button>}
          </div>
          <p className="muted" style={{ marginTop: 10 }}>查找顺序：随包 resources → 应用数据目录 → 系统 PATH；更新走 GitHub 官方 release。</p>
        </div>

        <div className="card">
          <h2 className="card-title">本 App 更新</h2>
          <div className="row">
            <span className="pill">当前 0.1.0</span>
            <span className="muted">tauri-plugin-updater</span>
          </div>
          <p className="muted" style={{ marginTop: 10 }}>配置公钥与 latest.json 地址后即可一键升级（见 README「App 自更新配置」）。</p>
          <div className="row" style={{ marginTop: 14 }}>
            <CheckAppUpdateButton setMsg={setMsg} />
          </div>
        </div>
      </div>

      <div className="card">
        <h2 className="card-title">下载偏好</h2>
        <div className="row">
          <span className="muted" style={{ width: 130 }}>默认输出目录</span>
          <input type="text" value={form.out_dir ?? ""} onChange={(e) => set("out_dir", e.target.value || null)} />
          <button className="btn btn-ghost btn-sm" onClick={pickDir}>浏览…</button>
        </div>
        <div className="row">
          <label className="field"><span>默认画质</span>
            <select value={form.default_format} onChange={(e) => set("default_format", e.target.value)}>
              {FORMAT_PRESETS.filter((p) => p.value !== "manual").map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
            </select>
          </label>
          <label className="field"><span>并发片段</span>
            <input type="number" min={1} max={16} value={form.concurrent_fragments} onChange={(e) => set("concurrent_fragments", Number(e.target.value) || 4)} style={{ width: 90, flex: "0 0 auto" }} />
          </label>
        </div>
        <label className="field"><span>代理（可选）</span>
          <input type="text" placeholder="http://127.0.0.1:7890" value={form.proxy ?? ""} onChange={(e) => set("proxy", e.target.value || null)} />
        </label>
        <label className="field"><span>文件名模板</span>
          <input type="text" value={form.filename_template} onChange={(e) => set("filename_template", e.target.value)} />
        </label>
        <div className="row" style={{ marginTop: 16 }}>
          <button className="btn" onClick={save}>保存设置</button>
        </div>
      </div>

      {msg && <div className="alert alert-info">{msg}</div>}
    </div>
  );
}

function CheckAppUpdateButton({ setMsg }: { setMsg: (s: string) => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <button
      className="btn btn-ghost"
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        try {
          const { check } = await import("@tauri-apps/plugin-updater");
          const u = await check();
          if (!u) setMsg("App 已是最新（或未配置更新地址）");
          else {
            setMsg(`发现 App 新版本 ${u.version}，开始下载安装…`);
            await u.downloadAndInstall();
            const { relaunch } = await import("@tauri-apps/plugin-process");
            await relaunch();
          }
        } catch (e) { setMsg(`App 更新检查失败：${String(e)}`); }
        finally { setBusy(false); }
      }}
    >
      {busy ? "检查中…" : "检查 App 更新"}
    </button>
  );
}
