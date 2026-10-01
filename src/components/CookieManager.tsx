import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { TAURI_COMMANDS, CookieProfile } from "../types";

export default function CookieManager({ onChange }: { onChange: () => void }) {
  const [profiles, setProfiles] = useState<CookieProfile[]>([]);
  const [name, setName] = useState("youtube-main");
  const [msg, setMsg] = useState<string | null>(null);
  const [tone, setTone] = useState<"info" | "success" | "error">("info");

  const refresh = async () => {
    try { setProfiles(await invoke<CookieProfile[]>(TAURI_COMMANDS.cookieList)); }
    catch (e) { setMsg(String(e)); setTone("error"); }
  };
  useEffect(() => { refresh(); }, []);

  const doImport = async () => {
    setMsg(null);
    const profile = name.trim();
    if (!profile) { setMsg("请先填写配置名（英文/数字/下划线）"); setTone("error"); return; }
    const file = await open({ multiple: false, filters: [{ name: "Netscape cookies.txt", extensions: ["txt"] }] });
    if (typeof file !== "string") return;
    try { await invoke(TAURI_COMMANDS.cookieImport, { name: profile, srcPath: file }); setMsg(`已导入 Cookie：${profile}`); setTone("success"); await refresh(); onChange(); }
    catch (e) { setMsg(`导入失败：${String(e)}`); setTone("error"); }
  };

  const remove = async (n: string) => {
    if (!confirm(`删除 Cookie 配置 ${n}？`)) return;
    await invoke(TAURI_COMMANDS.cookieRemove, { name: n }); await refresh(); onChange();
  };

  const setDefault = async (n: string | null) => {
    await invoke(TAURI_COMMANDS.cookieSetDefault, { name: n }); await refresh(); onChange();
  };

  const validate = async (n: string) => {
    setMsg("校验中（调用 yt-dlp --cookies 探测）…"); setTone("info");
    try { setMsg(await invoke<string>(TAURI_COMMANDS.cookieValidate, { name: n })); setTone("success"); }
    catch (e) { setMsg(`校验失败：${String(e)}`); setTone("error"); }
  };

  return (
    <div>
      <div className="card">
        <h2 className="card-title">导入 cookies.txt</h2>
        <p className="card-desc">从浏览器扩展（Get cookies.txt LOCALLY 等）导出目标站点的 Netscape 格式文件，导入为命名配置后即可用于下载。</p>
        <div className="row">
          <input type="text" placeholder="配置名，如 youtube-main" value={name} onChange={(e) => setName(e.target.value)} style={{ maxWidth: 260, flex: "0 0 auto" }} />
          <button className="btn" onClick={doImport}>选择文件并导入</button>
        </div>
      </div>

      <div className="card">
        <h2 className="card-title">已保存的配置 <span className="pill">{profiles.length}</span></h2>
        {profiles.length === 0 && <p className="muted">暂无配置，请先导入。</p>}
        {profiles.map((p) => (
          <div key={p.name} className="row" style={{ justifyContent: "space-between", padding: "10px 0", borderBottom: "1px solid var(--border)" }}>
            <div className="row">
              <strong>{p.name}</strong>
              {p.is_default && <span className="pill ok">默认</span>}
              <span className="muted">{p.entry_count} 条 · {p.updated_at ?? ""}</span>
            </div>
            <div className="row">
              {!p.is_default
                ? <button className="btn btn-ghost btn-sm" onClick={() => setDefault(p.name)}>设为默认</button>
                : <button className="btn btn-ghost btn-sm" onClick={() => setDefault(null)}>取消默认</button>}
              <button className="btn btn-ghost btn-sm" onClick={() => validate(p.name)}>校验</button>
              <button className="btn btn-danger btn-sm" onClick={() => remove(p.name)}>删除</button>
            </div>
          </div>
        ))}
      </div>

      {msg && <div className={`alert alert-${tone}`}>{msg}</div>}
    </div>
  );
}
