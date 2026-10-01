import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  TAURI_COMMANDS,
  AppSettings,
  YtdlpStatus,
  FfmpegStatus,
  YtdlpUpdateInfo,
  FORMAT_PRESETS,
} from "../types";
import { IconGear, IconRefresh, IconFolder, IconInfo, IconCheck } from "./icons";

function KV({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <div className="kv-row">
      <span className="k">{k}</span>
      <span className={`v ${mono ? "mono" : ""}`} title={v}>{v}</span>
    </div>
  );
}

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
  const [ffmpeg, setFfmpeg] = useState<FfmpegStatus | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [msgKind, setMsgKind] = useState<"info" | "error" | "success">("info");
  const [update, setUpdate] = useState<YtdlpUpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [working, setWorking] = useState(false);

  if (settings && !form) setForm(settings);

  useEffect(() => {
    invoke<FfmpegStatus>(TAURI_COMMANDS.ffmpegStatus)
      .then(setFfmpeg)
      .catch(() => setFfmpeg({ path: null, ready: false, source: "missing" }));
  }, []);

  const say = (kind: "info" | "error" | "success", text: string) => {
    setMsgKind(kind);
    setMsg(text);
  };

  const set = (k: keyof AppSettings, v: unknown) =>
    setForm((p) => (p ? { ...p, [k]: v } : p));

  const save = async () => {
    if (!form) return;
    await invoke(TAURI_COMMANDS.saveSettings, { settings: form });
    say("success", "下载偏好已保存。");
    onChange();
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && form) setForm({ ...form, out_dir: dir });
  };

  const ensureYtdlp = async () => {
    setWorking(true);
    say("info", "正在下载内置 yt-dlp（约 18MB，请稍候）…");
    try {
      const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.ensureYtdlp);
      say("success", `yt-dlp 就绪：${s.version}。`);
      onChange();
    } catch (e) {
      say("error", `失败：${String(e)}`);
    } finally {
      setWorking(false);
    }
  };

  const checkUpdate = async () => {
    setChecking(true);
    try {
      const info = await invoke<YtdlpUpdateInfo>(TAURI_COMMANDS.checkYtdlpUpdate);
      setUpdate(info);
      say(
        info.need_update ? "info" : "success",
        info.need_update
          ? `发现新版 yt-dlp：${info.current} → ${info.latest}，可一键更新。`
          : `yt-dlp 已是最新（${info.current}）。`
      );
    } catch (e) {
      say("error", `检查失败：${String(e)}`);
    } finally {
      setChecking(false);
    }
  };

  const doUpdate = async () => {
    setWorking(true);
    say("info", "正在更新 yt-dlp…");
    try {
      const s = await invoke<YtdlpStatus>(TAURI_COMMANDS.updateYtdlp);
      say("success", `已更新到 ${s.version}。`);
      setUpdate(null);
      onChange();
    } catch (e) {
      say("error", `更新失败：${String(e)}`);
    } finally {
      setWorking(false);
    }
  };

  const ensureFfmpeg = async () => {
    setWorking(true);
    say("info", "正在下载内置 ffmpeg（约 200MB，含解压，请耐心等待）…");
    try {
      const s = await invoke<FfmpegStatus>(TAURI_COMMANDS.ensureFfmpeg);
      setFfmpeg(s);
      say("success", "ffmpeg 已就绪，合并与转码功能可用。");
      onChange();
    } catch (e) {
      say("error", `失败：${String(e)}`);
    } finally {
      setWorking(false);
    }
  };

  if (!form) return <div className="muted">加载设置中…</div>;

  return (
    <div>
      <div className="page-head">
        <h1>设置与更新</h1>
        <p>管理内置组件版本、下载偏好与应用升级。</p>
      </div>

      <div className="grid2">
        {/* —— yt-dlp —— */}
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-head">
            <div className="t-ico">
              <IconRefresh size={17} />
            </div>
            <div>
              <h3>内置 yt-dlp</h3>
              <p>视频解析与下载的核心引擎</p>
            </div>
            <span className={`pill ${ytdlp?.ready ? "ok" : "warn"}`} style={{ marginLeft: "auto" }}>
              {ytdlp?.ready ? `就绪 ${ytdlp.version}` : "未就绪"}
            </span>
          </div>
          <KV k="来源" v={ytdlp?.source ?? "未知（bundled / downloaded / system）"} mono />
          <KV k="路径" v={ytdlp?.path ?? "未找到可执行文件"} mono />
          {update?.need_update && (
            <KV k="最新版" v={`${update.latest}（发布于 ${update.published_at ?? "未知"}）`} mono />
          )}
          <div className="row mt12">
            <button className="btn btn-ghost btn-sm" onClick={ensureYtdlp} disabled={working}>
              下载 / 修复
            </button>
            <button className="btn btn-ghost btn-sm" onClick={checkUpdate} disabled={checking}>
              {checking ? "检查中…" : "检查更新"}
            </button>
            {update?.need_update && (
              <button className="btn btn-primary btn-sm" onClick={doUpdate} disabled={working}>
                更新到 {update.latest}
              </button>
            )}
          </div>
        </div>

        {/* —— ffmpeg —— */}
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-head">
            <div className="t-ico">
              <IconCheck size={17} />
            </div>
            <div>
              <h3>内置 ffmpeg</h3>
              <p>音画合并与 mp3 / m4a 转码依赖</p>
            </div>
            <span className={`pill ${ffmpeg?.ready ? "ok" : "warn"}`} style={{ marginLeft: "auto" }}>
              {ffmpeg?.ready ? "就绪" : "未就绪"}
            </span>
          </div>
          <KV k="来源" v={ffmpeg?.source ?? "检测中…"} mono />
          <KV k="路径" v={ffmpeg?.path ?? "未找到可执行文件"} mono />
          {!ffmpeg?.ready && (
            <div className="row mt12">
              <button className="btn btn-ghost btn-sm" onClick={ensureFfmpeg} disabled={working}>
                下载内置 ffmpeg
              </button>
            </div>
          )}
        </div>
      </div>

      {/* —— App 更新 —— */}
      <div className="card">
        <div className="card-head">
          <div className="t-ico">
            <IconGear size={17} />
          </div>
          <div>
            <h3>本应用更新</h3>
            <p>当前版本 v0.1.0 · 基于 tauri-plugin-updater</p>
          </div>
          <div style={{ marginLeft: "auto" }}>
            <CheckAppUpdateButton say={say} proxy={form.proxy ?? null} />
          </div>
        </div>
        <div className="hint">
          正式发版需先配置签名公钥与 latest.json 更新地址（见 README「App 自更新配置」），否则检查会提示未配置。
        </div>
      </div>

      {/* —— 下载偏好 —— */}
      <div className="card">
        <div className="card-head">
          <div className="t-ico">
            <IconFolder size={17} />
          </div>
          <div>
            <h3>下载偏好</h3>
            <p>新建下载与批量任务的默认值</p>
          </div>
        </div>
        <div className="grid2">
          <div className="field">
            <span className="label">默认输出目录</span>
            <div className="row" style={{ flexWrap: "nowrap" }}>
              <input
                className="input grow"
                value={form.out_dir ?? ""}
                onChange={(e) => set("out_dir", e.target.value || null)}
                placeholder="如 D:\Videos"
              />
              <button className="btn btn-ghost btn-sm" onClick={pickDir}>选择…</button>
            </div>
          </div>
          <div className="field">
            <span className="label">文件名模板</span>
            <input
              className="input mono"
              value={form.filename_template}
              onChange={(e) => set("filename_template", e.target.value)}
            />
          </div>
          <div className="field">
            <span className="label">默认画质</span>
            <select
              className="select"
              value={form.default_format}
              onChange={(e) => set("default_format", e.target.value)}
            >
              {FORMAT_PRESETS.filter((p) => p.value !== "manual").map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <span className="label">并发片段（1–16）</span>
            <input
              className="input"
              type="number"
              min={1}
              max={16}
              value={form.concurrent_fragments}
              onChange={(e) => set("concurrent_fragments", Number(e.target.value) || 4)}
            />
          </div>
        </div>
        <div className="field mt12">
          <span className="label">代理（可选）</span>
          <input
            className="input"
            placeholder="http://127.0.0.1:7890"
            value={form.proxy ?? ""}
            onChange={(e) => set("proxy", e.target.value || null)}
          />
        </div>
        <div className="row mt16">
          <button className="btn btn-primary" onClick={save}>保存偏好</button>
        </div>
      </div>

      {msg && (
        <div className={`callout ${msgKind}`}>
          <IconInfo size={16} />
          <span>{msg}</span>
        </div>
      )}
    </div>
  );
}

function CheckAppUpdateButton({
  say,
  proxy,
}: {
  say: (k: "info" | "error" | "success", s: string) => void;
  proxy: string | null;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <button
      className="btn btn-ghost btn-sm"
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        try {
          const { check } = await import("@tauri-apps/plugin-updater");
          // 设置页代理同样用于更新检查，解决公司网/代理用户永远检查失败的问题
          const u = await check(
            proxy?.trim() ? { proxy: proxy.trim(), timeout: 20000 } : { timeout: 20000 }
          );
          if (!u) {
            say("success", "应用已是最新。");
          } else {
            say("info", `发现新版本 ${u.version}，开始下载安装…`);
            await u.downloadAndInstall();
            const { relaunch } = await import("@tauri-apps/plugin-process");
            await relaunch();
          }
        } catch (e) {
          const msg = String(e);
          // 未发版配置时给明确指引，而不是一串底层报错
          if (msg.includes("YOUR_NAME") || msg.includes("pubkey") || msg.includes("url")) {
            say("info", "应用自更新尚未配置（需发版时填签名公钥与 latest.json 地址），不影响 yt-dlp 下载功能。");
          } else {
            say("error", `应用更新检查失败：${msg}（公司网请先在下方设置代理）`);
          }
        } finally {
          setBusy(false);
        }
      }}
    >
      {busy ? "检查中…" : "检查应用更新"}
    </button>
  );
}
