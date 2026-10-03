# Y2B — Windows 创作者视频下载器

![license](https://img.shields.io/badge/license-MIT-green)
![platform](https://img.shields.io/badge/platform-Windows-blue)
![version](https://img.shields.io/badge/version-0.2.2-orange)

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

到 [Releases](../../releases) 下载 `Y2B_0.2.2_x64-setup.exe`，一路下一步即可。
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

### 18+ / 高清 403？点一下就行（yt-dlp #17542）

这是 YouTube 的 PO-Token 验证：高清格式要求 GVS PO Token。
Y2B 内置了一键方案：「设置 → YouTube 年龄限制 / PO 服务」点「一键安装并启动」，
应用自动下载 provider 插件 + 预编译 PO 服务 + Node portable（共约 60MB，一次性），
并在后台拉起 `127.0.0.1:4416` 服务。服务就绪后解析/下载自动走 `mweb` 拿高清；
服务异常时自动回退默认客户端。18+ 视频仍需 Cookie 用**已登录成人账号**。

技术细节：PO Token 按视频 ID 绑定、数小时过期，由本地服务逐个现算；
HTTP 服务不可用时自动降级走脚本直调（慢一些）。组件来源：

- 插件 + 服务：[bgutil-ytdlp-pot-provider](https://github.com/Brainicism/bgutil-ytdlp-pot-provider)
 （GPL-3.0，预编译包附带 LICENSE）+ [BgUtils](https://github.com/LuanRT/BgUtils)
- Node portable：https://nodejs.org（MIT）

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

## App 自更新（已启用）

应用内「设置 → 检查应用更新」走 GitHub Releases 的 `latest.json`，
签名公钥已写入 `src-tauri/tauri.conf.json`。

发新版流程：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "$env:USERPROFILE\.tauri\Y2B.key"
npm run tauri build -- --bundles nsis
# 产物：*.exe 安装包 + *.nsis.zip + *.nsis.zip.sig + latest.json
# 四个文件一起传到新 Release，用户即可在线升级
```

私钥在 `%USERPROFILE%\.tauri\Y2B.key`，**丢了就签不了新版，务必备份**。
公钥更换需同步改 `tauri.conf.json` 并重发版。

## 技术栈

- 前端：React 18 + Vite + TypeScript
- 后端：Rust（Tauri v2 command：resolve / list_formats / start_download / cookies / settings / pot_status …）
- 引擎：yt-dlp（内置 exe）+ ffmpeg（内置 exe）
- 更新：yt-dlp 走 GitHub releases 对比；App 自更新走签名 `latest.json`（见上节）

## 免责声明

本工具仅供学习与个人备份已获授权的内容。请遵守 YouTube
服务条款与当地法律法规，不要下载无权保存的视频。年龄限制内容请确保你已成年。

## 开源协议

[MIT](LICENSE)
