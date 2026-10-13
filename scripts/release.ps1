#Requires -Version 5.1
# Y2B one-click release: bump version -> commit -> tag -> push -> signed build -> public GitHub Release.
# The only manual step is entering the signing key password once (masked input,
# kept in process memory for this build only, never echoed, never logged, never written to disk).
# Usage: powershell -ExecutionPolicy Bypass -File scripts/release.ps1 -Version 0.2.3
param(
  [Parameter(Mandatory = $true)]
  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$Version,

  [string]$Notes = ""
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $RepoRoot
$Tag = 'v' + $Version

function Fail($msg) { Write-Host $msg -ForegroundColor Red; exit 1 }
function Step($msg) { Write-Host ''; Write-Host ('== ' + $msg + ' ==') -ForegroundColor Cyan }

$repoSlug = (gh repo view --json nameWithOwner --jq '.nameWithOwner').Trim()

# 本机 git 的 credential helper 是 `!gh auth git-credential`（shell 型，需要 sh），
# 部分环境 spawn 不了 sh → `fatal: could not read Username`。失败时退回一次性令牌 URL：
# 令牌只在进程内使用，不写盘、不改 remote.origin.url。
function Invoke-GitPush($ref) {
  git push origin $ref
  if ($LASTEXITCODE -eq 0) { return }
  Write-Host ('git push ' + $ref + ' failed, retrying with a one-off token URL') -ForegroundColor Yellow
  $tok = (& gh auth token).Trim()
  $url = 'https://x-access-token:' + $tok + '@github.com/' + $repoSlug + '.git'
  git -c credential.helper= -c "credential.https://github.com.helper=" push $url $ref
  if ($LASTEXITCODE -ne 0) { Fail ('git push failed: ' + $ref) }
  $tok = $null
  [GC]::Collect()
}

function Invoke-GitFetch {
  git fetch origin --tags
  if ($LASTEXITCODE -eq 0) { return }
  $tok = (& gh auth token).Trim()
  $url = 'https://x-access-token:' + $tok + '@github.com/' + $repoSlug + '.git'
  git -c credential.helper= -c "credential.https://github.com.helper=" fetch $url --tags
  $tok = $null
  [GC]::Collect()
}

function Bump-Version($file, $pattern) {
  if (-not (Test-Path -LiteralPath $file)) { Fail ('missing file: ' + $file) }
  $text = Get-Content -LiteralPath $file -Raw -Encoding UTF8
  # 必须用 ${1}/${2}：$1 直接拼接版本号会被 .NET 当成「第 10 个捕获组」，
  # 组不存在时整段按字面量输出，版本号会变成 "$10.2.3" 这种脏值
  $new = $text -replace $pattern, ('${1}' + $Version + '${2}')
  if ($new -ceq $text) { Fail ('version pattern not matched in ' + $file) }
  # 必须写成「无 BOM UTF-8」：Set-Content -Encoding UTF8 会加 BOM，
  # 而 Cargo.toml / tauri.conf.json 带 BOM 会被 cargo 与 JSON 解析器拒绝
  [IO.File]::WriteAllText((Join-Path $RepoRoot $file), $new, [Text.UTF8Encoding]::new($false))
}

# ---------- 0. preflight ----------
Step 'preflight'
foreach ($c in @('git', 'npm', 'gh')) {
  if (-not (Get-Command $c -ErrorAction SilentlyContinue)) { Fail ('missing tool: ' + $c) }
}
$nsis = 'C:\Program Files (x86)\NSIS\makensis.exe'
if (-not (Test-Path -LiteralPath $nsis)) { Fail ('NSIS not found: ' + $nsis) }
$keyPath = Join-Path $env:USERPROFILE '.tauri\Y2B.key'
if (-not (Test-Path -LiteralPath $keyPath)) { Fail ('signing key not found: ' + $keyPath) }
$dirty = git status --porcelain | Where-Object { $_ -notmatch '^\?\?' }
if ($dirty) { Fail 'working tree has uncommitted changes, commit or stash first' }
git rev-parse --verify --quiet ('refs/tags/' + $Tag) | Out-Null
if ($LASTEXITCODE -eq 0) { Fail ('tag already exists: ' + $Tag) }

# ---------- 1. bump version ----------
Step ('bump version to ' + $Version)
Bump-Version 'package.json' '("version":\s*")[^"]+(")'
# (?m) 必须加：.NET 的 ^ 默认只匹配整个字符串开头，而 Cargo.toml 以 [package] 开头，
# 没有 Multiline 时这条永远匹配不到，脚本会 Fail 在 Cargo.toml 上
Bump-Version 'src-tauri/Cargo.toml' '(?m)(^version\s*=\s*")[^"]+(")'
Bump-Version 'src-tauri/tauri.conf.json' '("version":\s*")[^"]+(")'
Bump-Version 'src/App.tsx' '(setAppVersion\(")[^"]+(")'
Bump-Version 'src/components/SettingsPanel.tsx' '(setAppVersion\(")[^"]+(")'
Bump-Version 'README.md' '(version-)[\d.]+(-orange)'

$readme = 'README.md'
$t = Get-Content -LiteralPath $readme -Raw -Encoding UTF8
$t = $t -replace 'Y2B_[\d.]+_x64-setup\.exe', ('Y2B_' + $Version + '_x64-setup.exe')
[IO.File]::WriteAllText((Join-Path $RepoRoot $readme), $t, [Text.UTF8Encoding]::new($false))

npx tsc --noEmit
if ($LASTEXITCODE -ne 0) { Fail 'tsc failed' }

# ---------- 2. signed build ----------
# 构建必须先于 commit/tag/push：一旦构建失败（版本号非法、签名缺失…），
# 远端就不会留下一个「版本号已改但没发版」的脏提交与 tag。
# The Y2B signing key has NO password: TAURI_SIGNING_PRIVATE_KEY_PASSWORD stays
# empty (just press Enter anywhere it is asked, including CI secret fields).
# If the key is ever replaced with an encrypted one, ask again via Read-Host.
Step 'signed build (key has no password)'
$env:PATH += ';C:\Program Files (x86)\NSIS'
# NOTE: the bundler only honors the inline key content var; _PATH alone makes
# the build fail with "no private key" even though `signer sign` accepts it.
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -LiteralPath $keyPath -Raw -Encoding ASCII
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''

try {
  # drop stale updater files so we never upload a previous version manifest
  $bundleDir = Join-Path $RepoRoot 'src-tauri\target\release\bundle'
  Get-ChildItem -LiteralPath $bundleDir -Recurse -File -Include 'latest.json' -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

  npm run tauri build -- --bundles nsis
  if ($LASTEXITCODE -ne 0) { Fail 'tauri build failed' }
}
finally {
  # password only lives during the build, cleared right after
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $null
  [GC]::Collect()
}

# ---------- 3. commit + tag + push（构建成功后才推远端） ----------
Step ('commit and push tag ' + $Tag)
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src/App.tsx src/components/SettingsPanel.tsx README.md CHANGELOG.md
git commit -m ('chore: bump ' + $Version + ' for release')
git tag $Tag
Invoke-GitPush 'master'
Invoke-GitPush $Tag
# 令牌 URL 推送不会更新 refs/remotes/origin，补一次 fetch 让本地远端引用跟上
Invoke-GitFetch

# ---------- 4. generate latest.json + publish Release ----------
# tauri build only emits setup exe + exe.sig; latest.json is NOT generated by
# the build and must be written by us following the updater schema:
# version/notes/pub_date + platforms.windows-x86_64{signature, url}.
# The updater polls .../releases/latest/download/latest.json and verifies the
# downloaded exe against the pubkey in tauri.conf.json.
Step ('publish GitHub Release ' + $Tag)
$bundleDir = Join-Path $RepoRoot 'src-tauri\target\release\bundle'
$nsisDir = Join-Path $bundleDir 'nsis'
$setupName = 'Y2B_' + $Version + '_x64-setup.exe'
$setupPath = Join-Path $nsisDir $setupName
$sigPath = $setupPath + '.sig'
$latestPath = Join-Path $nsisDir 'latest.json'
if (-not (Test-Path -LiteralPath $setupPath)) { Fail ('missing: ' + $setupName) }
if (-not (Test-Path -LiteralPath $sigPath)) { Fail ('missing sig (signing failed?): ' + $setupName + '.sig') }

$sigText = (Get-Content -LiteralPath $sigPath -Raw -Encoding ASCII).Trim()
$pubDate = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
$dlUrl = 'https://github.com/' + $repoSlug + '/releases/download/' + $Tag + '/' + $setupName
$manifestNotes = if ($Notes -ne '') { $Notes } else { 'Y2B ' + $Version + ' - see GitHub Releases for changelog' }
$manifest = [ordered]@{
  version = $Version
  notes = $manifestNotes
  pub_date = $pubDate
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = $sigText
      url = $dlUrl
    }
  }
}
$json = $manifest | ConvertTo-Json -Depth 6
# no-BOM UTF8: Set-Content -Encoding UTF8 would prepend a BOM that breaks parsers
[IO.File]::WriteAllText($latestPath, $json, [Text.UTF8Encoding]::new($false))

$artifacts = @($setupPath, $sigPath, $latestPath)
Write-Host ($artifacts -join [Environment]::NewLine)

if ($Notes -ne '') {
  gh release create $Tag --title $Tag --notes $Notes -- $artifacts
} else {
  gh release create $Tag --title $Tag --generate-notes -- $artifacts
}

Write-Host ''
Write-Host ('done, please verify: https://github.com/a1175815821/Y2B/releases/tag/' + $Tag) -ForegroundColor Green
