import "./styles.css";
import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS, YtdlpStatus, FfmpegStatus, AppSettings } from "./types";
import { IconDownload, IconCollection, IconKey, IconGear, IconRefresh, IconCheck } from "./components/icons";
import SingleDownload from "./components/SingleDownload";
import CreatorBatch from "./components/CreatorBatch";
import CookieManager from "./components/CookieManager";
import SettingsPanel from "./components/SettingsPanel";
import HistoryPanel from "./components/HistoryPanel";

type Tab = "single" | "creator" | "cookies" | "history" | "settings";

const NAV: { id: Tab; title: string; desc: string; icon: React.ReactNode }[] = [
  { id: "single", title: "新建下载", desc: "单视频 / 链接", icon: <IconDownload size={19} /> },
  { id: "creator", title: "创作者批量", desc: "频道扫描勾选", icon: <IconCollection size={19} /> },
  { id: "history", title: "下载历史", desc: "完成 / 定位", icon: <IconCheck size={19} /> },
  { id: "cookies", title: "Cookie 管理", desc: "Netscape 导入", icon: <IconKey size={19} /> },
  { id: "settings", title: "设置与更新", desc: "偏好 / 版本", icon: <IconGear size={19} /> },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("single");
  const [ytdlp, setYtdlp] = useState<YtdlpStatus | null>(null);
  const [ffmpeg, setFfmpeg] = useState<FfmpegStatus | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [spinning, setSpinning] = useState(false);

  const refreshEnv = async () => {
    setSpinning(true);
    try {
      const [y, f, s] = await Promise.all([
        invoke<YtdlpStatus>(TAURI_COMMANDS.ytdlpStatus),
        invoke<FfmpegStatus>(TAURI_COMMANDS.ffmpegStatus).catch(
          () => ({ path: null, ready: false, source: "missing" }) as FfmpegStatus
        ),
        invoke<AppSettings>(TAURI_COMMANDS.getSettings),
      ]);
      setYtdlp(y);
      setFfmpeg(f);
      setSettings(s);
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    } finally {
      setSpinning(false);
    }
  };

  useEffect(() => {
    refreshEnv();
  }, []);

  return (
    <div className="app">
      {/* ============ 侧边导航 ============ */}
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">Y2B</div>
          <div className="brand-text">
            <div className="brand-name">Y2B 下载器</div>
            <div className="brand-sub">创作者视频 · Windows</div>
          </div>
        </div>

        <div className="nav-label">功能</div>
        {NAV.map((n) => (
          <div
            key={n.id}
            className={`nav-item ${tab === n.id ? "active" : ""}`}
            onClick={() => setTab(n.id)}
          >
            {n.icon}
            <div className="nav-text">
              <div className="nav-title">{n.title}</div>
              <div className="nav-desc">{n.desc}</div>
            </div>
          </div>
        ))}

        <div className="side-foot">
          <div className="env-card">
            <div className="env-row">
              <span className={`dot ${ytdlp?.ready ? "ok" : "warn"}`} />
              yt-dlp
              <span className="ver">{ytdlp ? ytdlp.version ?? ytdlp.source : "…"}</span>
            </div>
            <div className="env-row">
              <span className={`dot ${ffmpeg?.ready ? "ok" : "warn"}`} />
              ffmpeg
              <span className="ver">{ffmpeg ? ffmpeg.source : "…"}</span>
            </div>
          </div>
          <button
            className={`icon-btn ${spinning ? "spinning" : ""}`}
            style={{ width: "100%", height: 34 }}
            onClick={refreshEnv}
            title="刷新环境状态"
          >
            <IconRefresh size={15} />
          </button>
        </div>
      </aside>

      {/* ============ 内容区 ============ */}
      <div className="content">
        {/* 页面常驻挂载、仅 CSS 显隐：切换页面不卸载，
            下载进度/日志/勾选/表单状态全部保留 */}
        <main className="page">
          <div className="page-inner">
            {bootError && (
              <div className="callout error" style={{ marginBottom: 16 }}>
                后端连接失败：{bootError}
              </div>
            )}
            {ytdlp && !ytdlp.ready && (
              <div className="callout warn" style={{ marginBottom: 16 }}>
                <span className="grow">
                  未检测到内置 yt-dlp（{ytdlp.source}），请前往「设置与更新」下载后再使用。
                </span>
                <button className="btn btn-ghost btn-sm" onClick={() => setTab("settings")}>
                  前往设置
                </button>
              </div>
            )}

            <section className={tab === "single" ? "" : "tab-hidden"}>
              <SingleDownload settings={settings} onSettingsChange={refreshEnv} />
            </section>
            <section className={tab === "creator" ? "" : "tab-hidden"}>
              <CreatorBatch settings={settings} />
            </section>
            <section className={tab === "history" ? "" : "tab-hidden"}>
              <HistoryPanel active={tab === "history"} />
            </section>
            <section className={tab === "cookies" ? "" : "tab-hidden"}>
              <CookieManager onChange={refreshEnv} />
            </section>
            <section className={tab === "settings" ? "" : "tab-hidden"}>
              <SettingsPanel settings={settings} ytdlp={ytdlp} onChange={refreshEnv} />
            </section>
          </div>
        </main>

        {/* ============ 状态栏 ============ */}
        <footer className="statusbar">
          <span className="stat-item">
            <span className={`dot ${ytdlp?.ready ? "ok" : "warn"}`} />
            yt-dlp <span className="mono">{ytdlp?.version ?? "未就绪"}</span>
          </span>
          <span className="stat-item">
            <span className={`dot ${ffmpeg?.ready ? "ok" : "warn"}`} />
            ffmpeg <span className="mono">{ffmpeg?.ready ? ffmpeg.source : "未就绪"}</span>
          </span>
          <span className="stat-item">
            Cookie <span className="mono">{settings?.default_cookie_profile ?? "未使用"}</span>
          </span>
          <div className="right">
            <span className="stat-item">{settings?.out_dir ?? "未设置输出目录"}</span>
            <span className="stat-item">Y2B v0.1.0</span>
          </div>
        </footer>
      </div>
    </div>
  );
}
