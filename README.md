# Y2B — Windows 创作者视频下载器

![license](https://img.shields.io/badge/license-MIT-green)
![platform](https://img.shields.io/badge/platform-Windows-blue)
![version](https://img.shields.io/badge/version-0.1.0-orange)

Tauri v2 + React + Rust 桌面应用，调用**内置 yt-dlp** 下载 YouTube 指定创作者 / 单视频：
完整格式列表自选、音频提取、Cookie（Netscape）管理、PO-Token / 播放器客户端设置（18+ 高清绕过）、
yt-dlp 与 App 双更新检查。

<img src="assets/icon.png" width="120" alt="Y2B icon">

## 功能

| 需求 | 实现 |
|---|---|
| Windows 原生窗口 | Tauri v2 + React，中文暗色 UI |
| 指定创作者批量下 | 频道 / @handle / playlist 扫描（`--flat-playlist`），勾选批量下载 |
| 单视频下载 | 粘贴链接解析，快捷预设（最佳 / 2160p / 1080p / mp3 …）或点选完整格式列表 |
| 可选画质、音频 | `yt-dlp -J` 全格式列表（过滤 storyboard，按画质排序），点击行即锁定 Format ID |
| Cookie 管理 | 命名配置，导入 Netscape `cookies.txt`，设默认 / 删除 / 校验 |
| 18+ / 高清绕过 | 播放器客户端切换（mweb / web_creator …）+ PO-Token 插件目录 + 手动 Token，见下 |
| 内置 yt-dlp | `resources/yt-dlp.exe` 随包打包；缺失时自动下载；设置页可一键更新 |
| 内置 ffmpeg | 音画合并与 mp3/m4a 转码；缺失时一键下载 |
| 断点续传 | `--continue` + `--no-overwrites`，中断重跑自动续传 |
| 中文报错 | 30+ 规则把 yt-dlp 英文报错翻译成一句话中文提示 |

## 安装

到 [Releases](../../releases) 下载 `Y2B_0.1.0_x64-setup.exe`，一路下一步即可。
安装包已内置 yt-dlp + ffmpeg，开箱即用。

> 首次启动 Windows Defender SmartScreen 可能会拦截（个人签名缺失属正常现象），
> 点“更多信息 → 仍要运行”即可。

## 使用

1. **Cookie（推荐先配）**：浏览器装扩展导出 Netscape 格式 `cookies.txt`
   （如 Get cookies.txt LOCALLY，导出 YouTube 域）→「Cookie 管理」→ 填写配置名 → 导入 → 设为默认。
   之后所有解析 / 下载自动带 `--cookies`，可过登录 / 年龄限制。
2. **单视频**：「新建下载」粘贴链接 → 解析 → 选画质预设或点格式表行 → 选输出目录 → 开始下载。
3. **创作者批量**：「创作者批量」粘贴频道 / @handle / 播放列表链接 → 扫描 → 勾选 → 批量下载。
4. **代理**：公司网用户在「设置」填 `http://127.0.0.1:7890`，更新检查同样走该代理。

### 18+ 视频只剩 360p / 403？（yt-dlp #17542）

这是 YouTube 的 PO-Token 验证：年龄限制视频的高清格式要求 GVS PO Token，
三步绕过：

1. 把 [bgutil-ytdlp-pot-provider](https://github.com/Brainicism/bgutil-ytdlp-pot-provider)
   （备用 [yt-dlp-getpot-wpc](https://github.com/coletdjnz/yt-dlp-getpot-wpc)）
   克隆到「设置」页显示的插件目录（`%APPDATA%/com.y2b.downloader/yt-dlp-plugins`）；
2. 「设置 → YouTube 年龄限制 / PO-Token」把客户端切到 `mweb`；
3. Cookie 用**已登录成人账号**并设为默认 → 保存后重新解析。

插件模式下「手动 PO-Token」留空即可（Token 绑定单个视频 ID，插件会自动逐个刷）。

## 从源码构建

```powershell
npm install
npm run tauri dev      # 开发模式
npm run tauri build    # 打包（需安装 NSIS，产物在 src-tauri/target/release/bundle/）
```

打包前准备（如需更新内置二进制版本，直接替换这两个文件）：

- `src-tauri/resources/yt-dlp.exe` ← https://github.com/yt-dlp/yt-dlp/releases
- `src-tauri/resources/ffmpeg.exe` ← https://github.com/yt-dlp/FFmpeg-Builds/releases

后端查找顺序：`resources/` → 应用数据目录 → 系统 PATH。
`cargo test` 跑后端单测（格式排序、extractor-args 构造、报错翻译规则）。

## 技术栈

- 前端：React 18 + Vite + TypeScript
- 后端：Rust（Tauri v2 command：resolve / list_formats / start_download / cookies / settings / pot_status …）
- 引擎：yt-dlp（内置 exe）+ ffmpeg（内置 exe）
- 更新：yt-dlp 走 GitHub releases 对比；App 自更新预留了 `tauri-plugin-updater`
  （发版时需配签名公钥 + latest.json，见「设置」页提示）

## 免责声明

本工具仅供学习与个人备份已获授权的内容。请遵守 YouTube
服务条款与当地法律法规，不要下载无权保存的视频。年龄限制内容请确保你已成年。

## 开源协议

[MIT](LICENSE)
