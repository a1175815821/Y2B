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

function Bump-Version($file, $pattern) {
  if (-not (Test-Path -LiteralPath $file)) { Fail ('missing file: ' + $file) }
  $text = Get-Content -LiteralPath $file -Raw -Encoding UTF8
  $new = $text -replace $pattern, ('$1' + $Version + '$2')
  if ($new -ceq $text) { Fail ('version pattern not matched in ' + $file) }
  Set-Content -LiteralPath $file -Value $new -Encoding UTF8
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
Bump-Version 'src-tauri/Cargo.toml' '(^version\s*=\s*")[^"]+(")'
Bump-Version 'src-tauri/tauri.conf.json' '("version":\s*")[^"]+(")'
Bump-Version 'src/App.tsx' '(setAppVersion\(")[^"]+(")'
Bump-Version 'src/components/SettingsPanel.tsx' '(setAppVersion\(")[^"]+(")'
Bump-Version 'README.md' '(version-)[\d.]+(-orange)'

$readme = 'README.md'
$t = Get-Content -LiteralPath $readme -Raw -Encoding UTF8
$t = $t -replace 'Y2B_[\d.]+_x64-setup\.exe', ('Y2B_' + $Version + '_x64-setup.exe')
Set-Content -LiteralPath $readme -Value $t -Encoding UTF8

npx tsc --noEmit
if ($LASTEXITCODE -ne 0) { Fail 'tsc failed' }

# ---------- 2. commit + tag + push ----------
Step ('commit and push tag ' + $Tag)
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src/App.tsx src/components/SettingsPanel.tsx README.md
git commit -m ('chore: bump ' + $Version + ' for release')
git tag $Tag
git push origin master
git push origin $Tag

# ---------- 3. signing password (asked once, masked) ----------
Step 'signed build (password asked once)'
$sec = Read-Host 'signing key password' -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
$plain = [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr)
[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
Remove-Variable sec

$env:PATH += ';C:\Program Files (x86)\NSIS'
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = $keyPath
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plain
$plain = $null

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

# ---------- 4. collect artifacts + publish Release ----------
# Real layout for this project (nsis target): setup exe + exe.sig + latest.json.
# latest.json points windows-x86_64.url at the setup exe, updater verifies it
# against the pubkey in tauri.conf.json. Scope by version: old version files
# from previous builds stay on disk but are never uploaded.
Step ('publish GitHub Release ' + $Tag)
$bundleDir = Join-Path $RepoRoot 'src-tauri\target\release\bundle'
$setupName = 'Y2B_' + $Version + '_x64-setup.exe'
$setupSig = $setupName + '.sig'
$wanted = @($setupName, $setupSig, 'latest.json')
$artifacts = @(Get-ChildItem -LiteralPath $bundleDir -Recurse -File |
  Where-Object { $wanted -contains $_.Name } |
  Select-Object -ExpandProperty FullName | Sort-Object -Unique)
Write-Host ($artifacts -join [Environment]::NewLine)
foreach ($w in $wanted) {
  if (-not ($artifacts | Where-Object { $_ -like ('*' + $w) })) {
    Fail ('missing artifact: ' + $w + ' (signing probably failed, wrong key password?)')
  }
}

if ($Notes -ne '') {
  gh release create $Tag --title $Tag --notes $Notes -- $artifacts
} else {
  gh release create $Tag --title $Tag --generate-notes -- $artifacts
}

Write-Host ''
Write-Host ('done, please verify: https://github.com/a1175815821/Y2B/releases/tag/' + $Tag) -ForegroundColor Green
