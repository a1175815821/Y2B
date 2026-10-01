# Y2B — Windows 创作者视频下载器（Tauri + yt-dlp）

Tauri v2 + React + Rust 后端，调用**内置 yt-dlp** 下载指定创作者/单视频，
支持完整格式列表自选、音频提取、Cookie（Netscape）管理、yt-dlp 与 App 双更新检查。

## 功能对照需求

| 需求 | 实现 |
|---|---|
| Windows UI | Tauri 原生窗口 + React（中文） |
| 指定创作者视频 | 创作者批量页：频道/@handle/playlist 扫描（`--flat-playlist`），勾选批量下载；单视频页也支持单链/列表 |
| 可选画质、音频 | 预设（最佳/1080/720/480/mp3/m4a）+ 完整格式列表（`yt-dlp -J`，点击行即选 Format ID） |
| Cookie 管理 | 命名配置，导入 Netscape `cookies.txt`，设默认/删除/校验（`--cookies` 轻量探测） |
| 内置 yt-dlp | `resources/yt-dlp.exe` 随包打包；缺失时自动下载到应用数据目录；`设置/更新`可修复 |
| 检查更新 | yt-dlp：GitHub releases latest 对比+一键更新；App：`tauri-plugin-updater`（需配公钥+latest.json） |

## 目录结构

```
Y2B/
  src/                    # React 前端
    components/
      SingleDownload.tsx  # 单视频/链接下载 + 完整格式列表
      CreatorBatch.tsx     # 创作者批量扫描 + 勾选下载
      CookieManager.tsx    # Cookie 导入/管理
      SettingsPanel.tsx    # 设置 + 双更新检查
    types.ts  App.tsx  main.tsx  styles.css
  src-tauri/              # Rust 后端
    src/
      lib.rs      # command 注册
      ytdlp.rs    # 内置定位/下载/更新/ffmpeg
      media.rs    # resolve_url / list_formats
      download.rs # start_download 进度事件
      cookies.rs  # Cookie CRUD + 校验
      settings.rs # settings.json 持久化
    tauri.conf.json  capabilities/default.json
  resources/              # 打包进安装包的二进制（见下）
```

## 快速开始（开发）

```powershell
npm install
npm run tauri dev
```

首次运行到「设置 / 更新」点「下载 / 修复内置 yt-dlp」即可（存到
`%APPDATA%\com.y2b.downloader\bin\yt-dlp.exe`）。

## 内置二进制（打包前准备）

下载对应 Windows 构建（两个目录都放一份最省心）：

- `src-tauri/resources/yt-dlp.exe` ← 打包进安装包用（tauri.conf.json bundle.resources）
- `resources/yt-dlp.exe` ← 开发模式 `npm run tauri dev` 直读用
- 同理 `ffmpeg.exe`（可选，合并/转码用）← https://github.com/yt-dlp/FFmpeg-Builds/releases

后端查找顺序：`resources/` → `app_data/bin/` → 系统 PATH。

## 打包 Windows 安装包

```powershell
npm run tauri build
# 产物：src-tauri/target/release/bundle/nsis/Y2B_*_x64-setup.exe
```

图标：先准备 `src-tauri/icons/icon.ico + 32x32.png + 128x128.png`（可用
`npm run tauri icon assets/icon.png` 生成），再把 `tauri.conf.json` 的
`bundle.icon` 指向它们。当前模板为避免缺文件构建失败，`icon` 留空数组，
正式发版前请补上。

## App 自更新配置（发版必需）

1. 生成签名密钥：`npm run tauri signer generate -w ~/.tauri/Y2B.key`
2. 把公钥填入 `src-tauri/tauri.conf.json → plugins.updater.pubkey`
3. 把 `endpoints` 改成你的 GitHub Release 的 `latest.json` 地址
4. CI 打包时带上 `TAURI_SIGNING_PRIVATE_KEY` 环境变量，Release 附带
   `.nsis.zip` + `.nsis.zip.sig` + `latest.json`，前端「检查 App 更新」即可升级

## Cookie 使用

1. 浏览器装扩展导出 Netscape 格式 `cookies.txt`
   （如 Get cookies.txt LOCALLY，导出 YouTube 域）
2. 「Cookie 管理」→ 填写配置名 → 选择文件导入 → 设为默认
3. 之后所有解析/下载自动带 `--cookies`，可过登录/年龄限制

## 常见问题

- 解析 403 / 登录确认：先导入 Cookie 并设默认，再重试
- 合并失败/转码失败：确认 ffmpeg 就绪（设置页 `ffmpeg_status`）
- 公司网/代理：在设置页填代理 `http://127.0.0.1:7890`
