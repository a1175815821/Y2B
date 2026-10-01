import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS, HistoryEntry } from "../types";
import { IconFolder, IconTrash, IconRefresh, IconInfo, IconPlay } from "./icons";

export default function HistoryPanel({ active }: { active: boolean }) {
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    setBusy(true);
    try {
      const list = await invoke<HistoryEntry[]>(TAURI_COMMANDS.historyList);
      setItems(list);
      setMsg(null);
    } catch (e) {
      setMsg(`读取历史失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  // 页面常驻挂载：每次切回本页时刷新，避免看到过期列表
  useEffect(() => {
    if (active) refresh();
  }, [active]);

  const openDir = async (path: string) => {
    try {
      await invoke(TAURI_COMMANDS.openInFolder, { path });
    } catch (e) {
      setMsg(`打开失败：${String(e)}`);
    }
  };

  const remove = async (id: string) => {
    await invoke(TAURI_COMMANDS.historyRemove, { id });
    await refresh();
  };

  const clear = async () => {
    if (!confirm("清空全部下载历史？（仅删除记录，不删除文件）")) return;
    await invoke(TAURI_COMMANDS.historyClear);
    await refresh();
  };

  const okCount = items.filter((i) => i.status === "ok").length;

  return (
    <div>
      <div className="page-head">
        <h1>下载历史</h1>
        <p>每次下载完成/失败自动记录，可定位文件目录、清理记录。</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h3>
              历史记录{" "}
              <span className="pill info" style={{ marginLeft: 6 }}>
                {okCount} 成功 / {items.length} 共
              </span>
            </h3>
            <p>只删记录不删文件，最多保留 200 条</p>
          </div>
          <div style={{ marginLeft: "auto" }} className="row">
            <button
              className={`icon-btn ${busy ? "spinning" : ""}`}
              onClick={refresh}
              title="刷新历史"
            >
              <IconRefresh size={16} />
            </button>
            {items.length > 0 && (
              <button className="btn btn-ghost btn-sm" onClick={clear}>
                <IconTrash size={13} />
                清空
              </button>
            )}
          </div>
        </div>

        {items.length === 0 ? (
          <div className="empty">
            <IconPlay size={26} />
            暂无下载历史，去「新建下载」完成第一单吧。
          </div>
        ) : (
          <div className="vlist">
            {items.map((h) => (
              <div key={h.id} className="vitem" style={{ cursor: "default" }}>
                <div className="grow">
                  <div className="t">{h.title || h.url}</div>
                  <div className="s">
                    {h.created_at} · {h.format_selector} · {h.out_dir}
                  </div>
                  {h.status !== "ok" && h.detail && (
                    <div className="s" style={{ color: "var(--err)" }}>
                      {h.detail.slice(0, 200)}
                    </div>
                  )}
                </div>
                <span className={`pill ${h.status === "ok" ? "ok" : "warn"}`}>
                  {h.status === "ok" ? "成功" : "失败"}
                </span>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={() => openDir(h.out_dir)}
                  title="打开所在目录"
                >
                  <IconFolder size={13} />
                  目录
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={() => remove(h.id)}
                  title="删除这条记录"
                >
                  <IconTrash size={13} />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {msg && (
        <div className="callout error">
          <IconInfo size={16} />
          <span>{msg}</span>
        </div>
      )}
    </div>
  );
}
