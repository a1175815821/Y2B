//! yt-dlp 原文报错 → 中文一句话提示
//!
//! 用法：拿到 stderr（或其拼接）后调 [`friendly_yt_dlp_error`]，
//! 未命中任何规则时回退到最后一条 ERROR 行（去前缀、截断），
//! 保证前端永远只展示一句能看懂的话。完整原文仍保留在日志面板里。

/// 将 yt-dlp 的英文报错翻译为中文一句话
pub fn friendly_yt_dlp_error(raw: &str) -> String {
    let text = raw.replace('\r', "\n");
    let lower = text.to_lowercase();

    // 按优先级匹配，命中即返回
    const RULES: &[(&[&str], &str)] = &[
        (
            &["not a bot", "sign in to confirm"],
            "YouTube 判定当前环境为机器人：请导入登录后的 Cookie 并设为默认，或更换网络/代理后重试",
        ),
        (
            &["confirm your age", "age-gated", "age restricted"],
            "该视频有年龄限制：请导入已登录的 Cookie 后重试",
        ),
        (
            &["private video"],
            "这是私享视频：当前 Cookie 账号没有观看权限，请换有权限的账号",
        ),
        (
            &["has been deleted", "has been removed"],
            "该视频已被删除",
        ),
        (
            &["account associated with this video has been terminated", "account has been terminated"],
            "上传该视频的账号已被封禁，视频无法访问",
        ),
        (
            &["this video is unavailable"],
            "该视频不可用：可能已删除、设为私享或所在地区不可看",
        ),
        (
            &["not available in your country", "blocked in your country", "not made this video available in your country"],
            "该视频有地区限制：请使用视频所在地区的代理后重试",
        ),
        (
            &["join this channel", "members-only", "purchase this", "paid content"],
            "这是会员专享/付费视频：当前账号无观看权限",
        ),
        (
            &["copyright"],
            "该视频因版权原因无法访问",
        ),
        (
            &["this live event has ended"],
            "该直播已结束，无回放可下",
        ),
        (
            &["premieres in", "live event will begin", "premiere will begin"],
            "首播/直播尚未开始，开播后再试",
        ),
        (
            &["playlist does not exist", "playlist not found"],
            "该播放列表不存在，请检查链接",
        ),
        (
            &["this playlist is private", "private playlist"],
            "该播放列表是私享的，当前账号无权限",
        ),
        (
            &["requested format is not available", "requested format not available", "no video formats found"],
            "所选画质没有对应格式：请换一档画质，或改用「最佳画质」",
        ),
        (
            &["unsupported url", "is not a valid url"],
            "不支持的链接：请确认是有效的视频/频道/播放列表地址",
        ),
        (
            &["unable to extract"],
            "页面解析失败：可能是 YouTube 改版或网络异常，先到「设置」更新内置 yt-dlp 再试",
        ),
        (
            &["http error 429", "too many requests", "status code: 429"],
            "请求太频繁被限流（429）：等几分钟后再试",
        ),
        (
            &["http error 403", "status code: 403", "forbidden"],
            "访问被拒绝（403）：IP 或环境受限，尝试更换代理或导入 Cookie",
        ),
        (
            &["http error 404", "status code: 404"],
            "内容不存在（404）：请检查链接是否正确",
        ),
        (
            &["http error 5"],
            "对方服务器出错（5xx）：稍后重试即可",
        ),
        (
            &["no space left on device"],
            "磁盘空间不足：请清理后更换输出目录",
        ),
        (
            &["permission denied"],
            "没有写入权限：请换一个有权限的输出目录",
        ),
        (
            &["ffmpeg", "ffprobe"],
            "缺少 ffmpeg 或转码失败：请到「设置」确认内置 ffmpeg 就绪",
        ),
        (
            &["postprocessing", "post-process"],
            "下载完成但合并/转码失败：多为 ffmpeg 问题，请确认 ffmpeg 就绪后重试",
        ),
        (
            &["proxy", "failed to parse proxy"],
            "代理连接失败：请检查「设置」里的代理地址是否正确",
        ),
        (
            &["temporary failure in name resolution", "nodename nor servname", "name or service not known"],
            "DNS 解析失败：请检查网络连接和代理设置",
        ),
        (
            &["failed to establish a new connection", "connection timed out", "connection reset", "network is unreachable", "timed out", "connection aborted", "max retries exceeded"],
            "网络连接失败：请检查网络或代理，稍后重试",
        ),
        (
            &["ssl", "certificate verify failed", "certificate_verify_failed"],
            "网络证书异常：如使用了抓包/代理软件请检查其设置，或更换网络",
        ),
        (
            &["login required", "sign in to download", "use --cookies", "cookies-from-browser"],
            "需要登录后才能访问：请导入 Cookie 并设为默认",
        ),
        (
            &["invalid cookie", "could not open cookie", "error parsing cookie"],
            "Cookie 文件有问题：请重新导出 Netscape 格式的 cookies.txt 再导入",
        ),
        (
            &["fragment", "chunk"],
            "分片下载失败：多为网络波动，重试一般可恢复",
        ),
        (
            &["file is larger than", "file is too large"],
            "文件超出大小限制：换低一档画质试试",
        ),
    ];

    for (keys, msg) in RULES {
        if keys.iter().any(|k| lower.contains(k)) {
            return msg.to_string();
        }
    }

    // 未命中：取最后一条 ERROR 行，去掉 "ERROR: [youtube] id:" 前缀并截断
    let mut fallback: Option<String> = None;
    for line in text.lines().rev() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // 跳过纯进度/下载行
        if t.starts_with("[download]") && t.contains('%') {
            continue;
        }
        fallback = Some(t.to_string());
        if t.to_lowercase().contains("error") {
            break;
        }
    }
    match fallback {
        None => "操作失败，但没有返回具体原因，请重试".to_string(),
        Some(line) => {
            let mut s = line;
            if let Some(idx) = s.to_lowercase().find("error:") {
                s = s[idx + 6..].trim().to_string();
            }
            // 去掉 "[youtube] BaW_jenozKc: " 这类前缀
            if s.starts_with('[') {
                if let Some(idx) = s.find("]:") {
                    s = s[idx + 2..].trim().to_string();
                } else if let Some(idx) = s.find(']') {
                    s = s[idx + 1..].trim().to_string();
                }
            }
            // 去掉末尾的英文帮助链接，保持简短
            if let Some(idx) = s.find("See https://") {
                s = s[..idx].trim().to_string();
            }
            if s.is_empty() {
                return "操作失败，请重试".to_string();
            }
            let short: String = s.chars().take(120).collect();
            format!("操作失败：{short}")
        }
    }
}

/// 带场景前缀的完整提示，如 friendly("解析失败", stderr)
pub fn friendly(context: &str, raw_stderr: &str) -> String {
    format!("{}：{}", context, friendly_yt_dlp_error(raw_stderr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bot_check_is_chinese() {
        let s = friendly_yt_dlp_error(
            "ERROR: [youtube] abc: Sign in to confirm you're not a bot. Use --cookies-from-browser or --cookies for the authentication.",
        );
        assert!(s.contains("机器人"), "got: {s}");
        assert!(!s.contains("not a bot"));
    }

    #[test]
    fn unavailable_is_chinese() {
        let s = friendly_yt_dlp_error("ERROR: [youtube] xxx: This video is unavailable");
        assert!(s.contains("不可用"), "got: {s}");
    }

    #[test]
    fn format_missing_is_chinese() {
        let s = friendly_yt_dlp_error("ERROR: Requested format is not available");
        assert!(s.contains("画质"), "got: {s}");
    }

    #[test]
    fn unknown_error_falls_back_to_trimmed_line() {
        let s = friendly_yt_dlp_error("ERROR: [youtube] xxx: Some brand new weird failure mode happened");
        assert!(s.starts_with("操作失败"), "got: {s}");
        assert!(!s.contains("[youtube]"));
        assert!(!s.contains("ERROR:"));
    }

    #[test]
    fn empty_gives_generic_message() {
        assert_eq!(
            friendly_yt_dlp_error(""),
            "操作失败，但没有返回具体原因，请重试"
        );
    }
}
