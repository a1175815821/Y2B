import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { TAURI_COMMANDS, CookieProfile } from "../types";
import { IconKey, IconCheck, IconTrash, IconInfo } from "./icons";

export default function CookieManager({ onChange }: { onChange: () => void }) {
  const [profiles, setProfiles] = useState<CookieProfile[]>([]);
  const [name, setName] = useState("youtube-main");
  const [msg, setMsg] = useState<string | null>(null);
  const [msgKind, setMsgKind] = useState<"info" | "error" | "success">("info");
  const [busy, setBusy] = useState(false);

  const say = (kind: "info" | "error" | "success", text: string) => {
    setMsgKind(kind);
    setMsg(text);
  };

  const refresh = async () => {
    try {
      const list = await invoke<CookieProfile[]>(TAURI_COMMANDS.cookieList);
      setProfiles(list);
    } catch (e) {
      say("error", String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const doImport = async () => {
    const profile = name.trim();
    if (!profile) {
      say("error", "请先填写配置名（英文 / 数字 / 下划线）");
      return;
    }
    const file = await open({
      multiple: false,
      filters: [{ name: "Netscape cookies.txt", extensions: ["txt"] }],
    });
    if (typeof file !== "string") return;
    setBusy(true);
    try {
      await invoke(TAURI_COMMANDS.cookieImport, { name: profile, srcPath: file });
      say("success", `已导入 Cookie 配置「${profile}」，可设为默认后直接用于下载。`);
      await refresh();
      onChange();
    } catch (e) {
      say("error", `导入失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const remove = async (n: string) => {
    if (!confirm(`删除 Cookie 配置「${n}」？`)) return;
    await invoke(TAURI_COMMANDS.cookieRemove, { name: n });
    await refresh();
    onChange();
  };

  const setDefault = async (n: string | null) => {
    await invoke(TAURI_COMMANDS.cookieSetDefault, { name: n });
    await refresh();
    onChange();
  };

  const validate = async (n: string) => {
    say("info", "校验中（调用 yt-dlp --cookies 轻量探测 YouTube）…");
    try {
      const ok = await invoke<string>(TAURI_COMMANDS.cookieValidate, { name: n });
      say("success", ok);
    } catch (e) {
      say("error", `校验失败：${String(e)}`);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1>Cookie 管理</h1>
        <p>导入 Netscape 格式 cookies.txt 为命名配置，下载时自动携带，可绕过登录与年龄限制。</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="t-ico">
            <IconKey size={17} />
          </div>
          <div>
            <h3>导入新的 Cookie 配置</h3>
            <p>从浏览器扩展（如 Get cookies.txt LOCALLY）导出 YouTube 域的 cookies.txt</p>
          </div>
        </div>
        <div className="steps">
          <div className="step-line">
            <div className="rail"><span className="step-num">1</span></div>
            <div className="body"><b>浏览器导出</b> — 用扩展导出 Netscape 格式的 cookies.txt 到本地。</div>
          </div>
          <div className="step-line">
            <div className="rail"><span className="step-num">2</span></div>
            <div className="body"><b>命名并导入</b> — 起个配置名（如 youtube-main），选择文件导入。</div>
          </div>
          <div className="step-line">
            <div className="rail"><span className="step-num">3</span></div>
            <div className="body"><b>设为默认</b> — 之后所有解析与下载自动携带该 Cookie，可随时切换。</div>
          </div>
        </div>
        <div className="row mt8">
          <div className="field" style={{ width: 230 }}>
            <span className="label">配置名</span>
            <input
              className="input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="youtube-main"
            />
          </div>
          <button className="btn btn-primary" style={{ alignSelf: "flex-end" }} onClick={doImport} disabled={busy}>
            选择 cookies.txt 并导入
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="t-ico">
            <IconCheck size={17} />
          </div>
          <div>
            <h3>已保存的配置 <span className="pill info" style={{ marginLeft: 6 }}>{profiles.length}</span></h3>
            <p>存储于应用数据目录 cookies/，每个配置独立管理</p>
          </div>
        </div>
        {profiles.length === 0 ? (
          <div className="empty">
            <IconKey size={26} />
            暂无 Cookie 配置，请先按上方三步导入。
          </div>
        ) : (
          <div className="grid3">
            {profiles.map((p) => (
              <div key={p.name} className="card cookie-card panel-soft" style={{ marginBottom: 0 }}>
                <div className="nm">
                  {p.name}
                  {p.is_default && <span className="pill ok">默认</span>}
                </div>
                <div className="meta">
                  {p.entry_count} 条记录 · {p.updated_at ?? "未知时间"}
                </div>
                <div className="ops">
                  {!p.is_default ? (
                    <button className="btn btn-ghost btn-sm" onClick={() => setDefault(p.name)}>
                      设为默认
                    </button>
                  ) : (
                    <button className="btn btn-ghost btn-sm" onClick={() => setDefault(null)}>
                      取消默认
                    </button>
                  )}
                  <button className="btn btn-ghost btn-sm" onClick={() => validate(p.name)}>
                    校验
                  </button>
                  <button className="btn btn-danger btn-sm" onClick={() => remove(p.name)}>
                    <IconTrash size={13} />
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
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
