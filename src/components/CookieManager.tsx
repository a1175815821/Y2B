import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { TAURI_COMMANDS, CookieProfile } from "../types";

export default function CookieManager({ onChange }: { onChange: () => void }) {
  const [profiles, setProfiles] = useState<CookieProfile[]>([]);
  const [name, setName] = useState("youtube-main");
  const [msg, setMsg] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const list = await invoke<CookieProfile[]>(TAURI_COMMANDS.cookieList);
      setProfiles(list);
    } catch (e) {
      setMsg(String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const doImport = async () => {
    setMsg(null);
    const profile = name.trim();
    if (!profile) {
      setMsg("请先填写配置名（英文/数字/下划线）");
      return;
    }
    const file = await open({
      multiple: false,
      filters: [{ name: "Netscape cookies.txt", extensions: ["txt"] }],
    });
    if (typeof file !== "string") return;
    try {
      await invoke(TAURI_COMMANDS.cookieImport, { name: profile, srcPath: file });
      setMsg(`已导入 Cookie：${profile}`);
      await refresh();
      onChange();
    } catch (e) {
      setMsg(`导入失败：${String(e)}`);
    }
  };

  const remove = async (n: string) => {
    if (!confirm(`删除 Cookie 配置 ${n}？`)) return;
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
    setMsg("校验中（调用 yt-dlp --cookies 轻量探测）…");
    try {
      const ok = await invoke<string>(TAURI_COMMANDS.cookieValidate, { name: n });
      setMsg(ok);
    } catch (e) {
      setMsg(`校验失败：${String(e)}`);
    }
  };

  return (
    <div>
      <div className="card">
        <h3>导入 Netscape cookies.txt</h3>
        <div className="muted">
          从浏览器扩展（如 Get cookies.txt LOCALLY）导出 Netscape 格式的 cookies.txt，
          然后导入为命名配置。下载时通过 yt-dlp --cookies 参数使用，可绕过登录/年龄限制。
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          <input
            type="text"
            placeholder="配置名，如 youtube-main"
            value={name}
            onChange={(e) => setName(e.target.value)}
            style={{ maxWidth: 240 }}
          />
          <button onClick={doImport}>选择 cookies.txt 并导入</button>
        </div>
      </div>

      <div className="card">
        <h3>已保存的 Cookie 配置</h3>
        {profiles.length === 0 && <div className="muted">暂无，请先导入。</div>}
        {profiles.map((p) => (
          <div key={p.name} className="row" style={{ justifyContent: "space-between", borderBottom: "1px solid var(--line)", padding: "8px 0" }}>
            <div>
              <strong>{p.name}</strong>{" "}
              {p.is_default && <span className="pill ok">默认</span>}{" "}
              <span className="muted">
                {p.entry_count} 条 · {p.updated_at ?? ""}
              </span>
            </div>
            <div className="row">
              {!p.is_default && (
                <button className="ghost" onClick={() => setDefault(p.name)}>
                  设为默认
                </button>
              )}
              {p.is_default && (
                <button className="ghost" onClick={() => setDefault(null)}>
                  取消默认
                </button>
              )}
              <button className="ghost" onClick={() => validate(p.name)}>
                校验
              </button>
              <button className="danger" onClick={() => remove(p.name)}>
                删除
              </button>
            </div>
          </div>
        ))}
      </div>

      {msg && <div className="card muted">{msg}</div>}
    </div>
  );
}
