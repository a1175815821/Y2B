import "./styles.css";
import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS, YtdlpStatus, AppSettings } from "./types";
import SingleDownload from "./components/SingleDownload";
import CreatorBatch from "./components/CreatorBatch";
import CookieManager from "./components/CookieManager";
import SettingsPanel from "./components/SettingsPanel";

type Tab = "single" | "creator" | "cookies" | "settings";

const NAV: { id: Tab; label: string; group: string; icon: React.ReactNode }[] = [
  {
    id: "single", label: "单视频下载", group: "下载",
    icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3v12"/><path d="m7 10 5 5 5-5"/><path d="M5 21h14"/></svg>,
  },
  {
    id: "creator", label: "创作者批量", group: "下载",
    icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="m10 9 5 3-5 3z"/></svg>,
  },
  {
    id: "cookies", label: "Cookie 管理", group: "配置",
    icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9"/><path d="M8.5 10h.01M15.5 10h.01M9 15c1.5 1 4.5 1 6 0"/></svg>,
  },
  {
    id: "settings", label: "设置 / 更新", group: "配置",
    icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h0a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h0a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v0a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>,
  },
];

const PAGE_META: Record<Tab, { title: string; sub: string }> = {
  single:  { title: "单视频下载", sub: "粘贴链接，选择画质 / 音频格式，一键下载" },
  creator: { title: "创作者批量下载", sub: "扫描频道 / 播放列表，勾选后批量保存" },
  cookies: { title: "Cookie 管理", sub: "导入 Netscape cookies.txt，管理多个登录配置" },
  settings: { title: "设置与更新", sub: "下载偏好、内置 yt-dlp / ffmpeg 与版本更新" },
};

export default function App() {
  const [tab, setTab] = useState<Tab>("single");
  const [ytdlp, setYtdlp] = useState<YtdlpStatus | null>(null);
  const [ffmpegReady, setFfmpegReady] = useState<boolean | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const refreshEnv = async () => {
    try {
      const [s, st, ff] = await Promise.all([
        invoke<YtdlpStatus>(TAURI_COMMANDS.ytdlpStatus),
        invoke<AppSettings>(TAURI_COMMANDS.getSettings),
        invoke<{ ready: boolean }>(TAURI_COMMANDS.ffmpegStatus),
      ]);
      setYtdlp(s);
      setSettings(st);
      setFfmpegReady(ff.ready);
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    }
  };

  useEffect(() => { refreshEnv(); }, []);

  const groups = Array.from(new Set(NAV.map((n) => n.group)));

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-logo">Y2B</div>
          <div>
            <div className="brand-name">Y2B</div>
            <div className="brand-sub">视频下载器</div>
          </div>
        </div>

        <nav>
          {groups.map((g) => (
            <div key={g} style={{ marginBottom: 18 }}>
              <div className="nav-group-label">{g}</div>
              {NAV.filter((n) => n.group === g).map((n) => (
                <button
                  key={n.id}
                  className={`nav-item ${tab === n.id ? "active" : ""}`}
                  onClick={() => setTab(n.id)}
                >
                  {n.icon}
                  <span>{n.label}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-env">
          <div className="env-row">
            <span className={`dot ${ytdlp?.ready ? "ok" : "warn"}`} />
            <span className="env-text">yt-dlp {ytdlp?.ready ? (ytdlp.version ?? "就绪") : "未就绪"}</span>
          </div>
          <div className="env-row">
            <span className={`dot ${ffmpegReady ? "ok" : "warn"}`} />
            <span className="env-text">ffmpeg {ffmpegReady ? "已集成" : "缺失"}</span>
          </div>
        </div>
      </aside>

      <main className="main">
        <div className="page-head">
          <div>
            <h1 className="page-title">{PAGE_META[tab].title}</h1>
            <div className="page-sub">{PAGE_META[tab].sub}</div>
          </div>
          <button className="btn btn-ghost btn-sm" onClick={refreshEnv}>刷新环境</button>
        </div>

        {bootError && <div className="alert alert-error">后端连接失败：{bootError}</div>}
        {ytdlp && !ytdlp.ready && (
          <div className="alert alert-warn">
            未检测到内置 yt-dlp，请前往「设置 / 更新」完成初始化。
            <button className="btn btn-ghost btn-sm" onClick={() => setTab("settings")}>前往设置</button>
          </div>
        )}

        {tab === "single" && <SingleDownload settings={settings} onSettingsChange={refreshEnv} />}
        {tab === "creator" && <CreatorBatch settings={settings} />}
        {tab === "cookies" && <CookieManager onChange={refreshEnv} />}
        {tab === "settings" && <SettingsPanel settings={settings} ytdlp={ytdlp} onChange={refreshEnv} />}
      </main>
    </div>
  );
}
