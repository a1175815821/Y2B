import "./styles.css";
import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS, YtdlpStatus, AppSettings } from "./types";
import SingleDownload from "./components/SingleDownload";
import CreatorBatch from "./components/CreatorBatch";
import CookieManager from "./components/CookieManager";
import SettingsPanel from "./components/SettingsPanel";

type Tab = "single" | "creator" | "cookies" | "settings";

export default function App() {
  const [tab, setTab] = useState<Tab>("single");
  const [ytdlp, setYtdlp] = useState<YtdlpStatus | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const refreshEnv = async () => {
    try {
      const [s, st] = await Promise.all([
        invoke<YtdlpStatus>(TAURI_COMMANDS.ytdlpStatus),
        invoke<AppSettings>(TAURI_COMMANDS.getSettings),
      ]);
      setYtdlp(s);
      setSettings(st);
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    }
  };

  useEffect(() => {
    refreshEnv();
  }, []);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo">Y2B</span>
          <span className="subtitle">创作者视频下载器 · Windows</span>
        </div>
        <div className="env">
          <span className={`pill ${ytdlp?.ready ? "ok" : "warn"}`}>
            yt-dlp: {ytdlp ? (ytdlp.version ?? ytdlp.source) : "检测中…"}
          </span>
          <button className="ghost" onClick={refreshEnv}>
            刷新
          </button>
        </div>
      </header>

      {bootError && <div className="error">后端连接失败：{bootError}</div>}
      {ytdlp && !ytdlp.ready && (
        <div className="warnbar">
          未检测到内置 yt-dlp，请前往「设置 / 更新」点击「下载内置 yt-dlp」。
          <button onClick={() => setTab("settings")}>前往设置</button>
        </div>
      )}

      <nav className="tabs">
        <button className={tab === "single" ? "active" : ""} onClick={() => setTab("single")}>
          单视频 / 链接下载
        </button>
        <button className={tab === "creator" ? "active" : ""} onClick={() => setTab("creator")}>
          创作者批量
        </button>
        <button className={tab === "cookies" ? "active" : ""} onClick={() => setTab("cookies")}>
          Cookie 管理
        </button>
        <button className={tab === "settings" ? "active" : ""} onClick={() => setTab("settings")}>
          设置 / 更新
        </button>
      </nav>

      <main className="main">
        {tab === "single" && <SingleDownload settings={settings} onSettingsChange={refreshEnv} />}
        {tab === "creator" && <CreatorBatch settings={settings} />}
        {tab === "cookies" && <CookieManager onChange={refreshEnv} />}
        {tab === "settings" && (
          <SettingsPanel settings={settings} ytdlp={ytdlp} onChange={refreshEnv} />
        )}
      </main>
    </div>
  );
}
