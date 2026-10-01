/** 下载完成系统通知（尽力而为，失败静默） */
export function notifyDownload(title: string, body: string) {
  try {
    if (!("Notification" in window)) return;
    if (Notification.permission === "granted") {
      new Notification(title, { body });
    } else if (Notification.permission === "default") {
      Notification.requestPermission().then((p) => {
        if (p === "granted") new Notification(title, { body });
      });
    }
  } catch {
    /* WebView 不支持时忽略，界面内 log 已有记录 */
  }
}

const LINK_RE = /https?:\/\/[^\s"'<>）】]+/i;

function looksLikeMediaLink(u: string): boolean {
  return /youtube\.com|youtu\.be|bilibili\.com|b23\.tv|youku\.com|iqiyi\.com|v\.qq\.com|douyin\.com|tiktok\.com|twitch\.tv|nicovideo\.jp/i.test(
    u
  );
}

/** 从剪贴板读第一条疑似视频链接，无权限/无链接时返回 null */
export async function readClipboardLink(): Promise<string | null> {
  try {
    if (!navigator.clipboard?.readText) return null;
    const text = await navigator.clipboard.readText();
    if (!text) return null;
    const m = text.match(LINK_RE);
    if (m && looksLikeMediaLink(m[0])) return m[0];
    return null;
  } catch {
    return null;
  }
}
