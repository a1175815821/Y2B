# 更新日志

本文件只记录从建立之日起的版本；更早的改动见
[GitHub Releases](https://github.com/a1175815821/Y2B/releases)。

## 0.2.3

### 修复

- **ffmpeg 合并失败不再谎报成功**（P0）
  yt-dlp 与 ffmpeg 可能不在同一目录（设置页「更新 yt-dlp」会把 yt-dlp 写到应用数据目录，
  而 ffmpeg 仍是出厂内置），此前 yt-dlp 找不到 ffmpeg 时只打一行 WARNING 并**退出码 0**，
  界面弹「下载完成」、历史记「成功」，输出目录里却只有 `.fXXX.mp4` / `.fXXX.m4a` 两个碎片。
  现在：下载命令显式传 `--ffmpeg-location`，并在成功分支单独识别
  `ffmpeg is not installed` / `ffmpeg not found`，按失败处理并给出明确提示。
- **设置页「保存偏好」不再回滚 Cookie 默认配置**（P1）
  设置表单只在首帧快照一次，Cookie 页设为默认后再回设置页保存，会把
  `default_cookie_profile` 写回空。现在本页不管辖的字段每次跟随服务端最新值。
- **Cookie 校验跟随代理**（P2）
  `cookie_validate` 此前没传 `--proxy`，代理用户点「校验」必然失败；现在与解析/下载一致。
- **历史区分「跳过」**（P3）
  被 `--no-overwrites` 跳过的任务不再记成「成功」，历史页单独显示「跳过」并计入独立统计。
- **清空输出目录输入框后会回填默认目录**（P3），兑现原注释承诺。
- **PO「停止服务」兼容 IPv6 监听**（P3）：netstat 的 `TCPv6` 行此前被整行跳过。

### 改进

- PO 一键诊断：一次输出插件包 / portable node / 脚本自检 / 服务状态，定位 Token 拿不到的原因。
- 报错翻译改为按**日志末端**定性：中段顺手打的 PO WARNING 不再盖过末端真正的文件/网络死因。
- 下载文件名兼容：统一加 `--windows-filenames` + `--trim-filenames 150`。
- 全局顶部 toast：下载完成 / 跳过 / 失败 / 批量汇总即时可见（错误常驻可关）。

### 安全

- 移除前端零调用的 `shell` / `fs` / `http` / `opener` 四个插件及其 10 条无作用域权限
  （其中 `shell:allow-execute` 按插件文档是「无预置作用域的命令执行」）。
  Rust 侧的 explorer / netstat / taskkill 走 `std::process::Command`，不受影响。

### 发版脚本

- 修 `Bump-Version`：`'$1' + $Version` 会被 .NET 当成第 10 个捕获组，
  版本号被写成字面量 `$10.x.y`；改用 `${1}` / `${2}`。
- commit + tag + push 移到**构建成功之后**，构建失败不再往远端留脏提交与 tag。
- `git add` 补上 `src-tauri/Cargo.lock`，避免升版后锁文件落在提交外。
