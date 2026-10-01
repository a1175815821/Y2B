import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  TAURI_COMMANDS,
  AppSettings,
  YtdlpStatus,
  YtdlpUpdateInfo,
  FORMAT_PRESETS,
} from "../types";

export default function SettingsPanel({
  settings,
  ytdlp,
  onChange,
}: {
  settings: AppSettings | null;
  ytdlp: YtdlpStatus | null;
  onChange: () => void;
}) {
  const [form, setForm] = useState<AppSettings | null>(settings);
  const [msg, setMsg] = useState<string | null>(null);
  const [update, setUpdate] = useState<YtdlpUpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);

  // settings 异步到达后同步一次
  if (settings && !form) setForm(settings);

  const set = (k: keyof AppSettings, v: unknown) =>
    setForm((p) => (p ? { ...p, [k]: v } : p));

  const save = async () => {
    if (!form) return;
    await invoke(TAURI_COMMANDS.saveSettings, { settings: form });
    setMsg("设置已保存");
    onChange();
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && form) setForm({ ...form, out_dir: dir });
  };

  const ensureYtdlp = async () => {
    setMsg("正在下载内置 yt-dlp…");
    try {
      const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.ensureYtdlp);
      setMsg(`yt-dlp 就绪：${s.version} (${s.path})`);
      onChange();
    } catch (e) {
      setMsg(`失败：${String(e)}`);
    }
  };

  const checkUpdate = async () => {
    setChecking(true);
    setMsg(null);
    try {
      const info = await invoke<YtdlpUpdateInfo>(TAURI_COMMANDS.checkYtdlpUpdate);
      setUpdate(info);
      setMsg(
        info.need_update
          ? `发现新版 yt-dlp：${info.current} → ${info.latest}`
          : `yt-dlp 已是最新（${info.current}）`
      );
    } catch (e) {
      setMsg(`检查失败：${String(e)}`);
    } finally {
      setChecking(false);
    }
  };

  const doUpdate = async () => {
    setMsg("正在更新 yt-dlp…");
    try {
      const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.updateYtdlp);
      setMsg(`已更新到 ${s.version}`);
      onChange();
    } catch (e) {
      setMsg(`更新失败：${String(e)}`);
    }
  };

  if (!form) return <div className="muted">加载设置中…</div>;

  return (
    <div>
      <div className="grid2">
        <div className="card">
          <h3>内置 yt-dlp</h3>
          <div className="muted">
            状态：{ytdlp?.ready ? `就绪 ${ytdlp.version}` : "未就绪"} · 来源：
            {ytdlp?.source}
          </div>
          <div className="muted" style={{ wordBreak: "break-all" }}>
            {ytdlp?.path ?? "未找到可执行文件"}
          </div>
          <div className="row" style={{ marginTop: 10 }}>
            <button onClick={ensureYtdlp}>下载 / 修复内置 yt-dlp</button>
            <button className="ghost" onClick={checkUpdate} disabled={checking}>
              {checking ? "检查中…" : "检查 yt-dlp 更新"}
            </button>
            {update?.need_update && <button onClick={doUpdate}>更新到 {update.latest}</button>}
          </div>
          <div className="muted" style={{ marginTop: 8 }}>
            内置策略：优先使用 resources/yt-dlp.exe（随 App
            打包），缺失时下载到应用数据目录并记录版本；检查更新走 GitHub
            yt-dlp/yt-dlp releases latest。
          </div>
        </div>

        <div className="card">
          <h3>本 App 更新</h3>
          <div className="muted">当前版本：0.1.0</div>
          <div className="muted">
            已集成 tauri-plugin-updater，打包时配置公钥 + releases
            更新地址后，「检查 App 更新」即可提示升级（需签名，见 README 打包章节）。
          </div>
          <div className="row" style={{ marginTop: 10 }}>
            <CheckAppUpdateButton setMsg={setMsg} />
          </div>
        </div>
      </div>

      <div className="card">
        <h3>下载设置</h3>
        <div className="row">
          <span style={{ width: 140 }}>默认输出目录</span>
          <input
            type="text"
            value={form.out_dir ?? ""}
            onChange={(e) => set("out_dir", e.target.value || null)}
          />
          <button className="ghost" onClick={pickDir}>
            选择…
          </button>
        </div>
        <div className="row">
          <span style={{ width: 140 }}>默认画质</span>
          <select
            value={form.default_format}
            onChange={(e) => set("default_format", e.target.value)}
          >
            {FORMAT_PRESETS.filter((p) => p.value !== "manual").map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
          <span style={{ width: 140 }}>并发片段</span>
          <input
            type="number"
            min={1}
            max={16}
            value={form.concurrent_fragments}
            onChange={(e) => set("concurrent_fragments", Number(e.target.value) || 4)}
            style={{ width: 80 }}
          />
        </div>
        <div className="row">
          <span style={{ width: 140 }}>代理（可选）</span>
          <input
            type="text"
            placeholder="http://127.0.0.1:7890"
            value={form.proxy ?? ""}
            onChange={(e) => set("proxy", e.target.value || null)}
          />
        </div>
        <div className="row">
          <span style={{ width: 140 }}>文件名模板</span>
          <input
            type="text"
            value={form.filename_template}
            onChange={(e) => set("filename_template", e.target.value)}
          />
        </div>
        <div className="row">
          <button onClick={save}>保存设置</button>
        </div>
      </div>

      {msg && <div className="card muted">{msg}</div>}
    </div>
  );
}

function CheckAppUpdateButton({ setMsg }: { setMsg: (s: string) => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <button
      className="ghost"
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        try {
          const { check } = await import("@tauri-apps/plugin-updater");
          const u = await check();
          if (!u) {
            setMsg("App 已是最新（或未配置更新地址）");
          } else {
            setMsg(`发现 App 新版本 ${u.version}，开始下载安装…`);
            await u.downloadAndInstall();
            const { relaunch } = await import("@tauri-apps/plugin-process");
            await relaunch();
          }
        } catch (e) {
          setMsg(`App 更新检查失败：${String(e)}（需先配置公钥与更新地址，见 README）`);
        } finally {
          setBusy(false);
        }
      }}
    >
      {busy ? "检查中…" : "检查 App 更新"}
    </button>
  );
}
