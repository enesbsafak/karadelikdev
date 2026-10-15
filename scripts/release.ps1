﻿# KaradelikDev sürüm paketi: kurulum programını (NSIS) derler, dist\ altında setup, SHA256SUMS.txt ve latest.json üretir.
# latest.json uygulama içi güncelleyicinin okuduğu dosyadır; GitHub sürümüne setup ile birlikte yüklenmelidir.
#
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1            # derle + paketle
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -NoBuild   # mevcut derlemeyi paketle
#
# Sürüm notları: docs\releases\<sürüm>.md (GitHub sürüm metni). Madde işaretli satırlar latest.json'a özet olarak girer.
param([switch]$NoBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = ([regex]::Match((Get-Content src-tauri\Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"')).Groups[1].Value
$pkg = (Get-Content package.json -Raw | ConvertFrom-Json).version
$conf = (Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).version
if ($version -ne $pkg -or $version -ne $conf) { throw "Sürüm uyuşmuyor: Cargo.toml=$version package.json=$pkg tauri.conf.json=$conf" }
$notesFile = "docs\releases\$version.md"
if (-not (Test-Path $notesFile)) { throw "Sürüm notu yok: $notesFile" }

if (-not $NoBuild) {
    npm run build
    if ($LASTEXITCODE -ne 0) { throw 'Derleme başarısız' }
}

$built = "src-tauri\target\release\bundle\nsis\KaradelikDev_${version}_x64-setup.exe"
if (-not (Test-Path $built)) { throw "Kurulum programı yok: $built" }
$name = "KaradelikDev-$version-setup.exe"
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
Get-ChildItem $dist -File | Remove-Item -Force
Copy-Item $built "$dist\$name"

$setupHash = (Get-FileHash "$dist\$name" -Algorithm SHA256).Hash.ToLower()
$exeHash = (Get-FileHash src-tauri\target\release\karadelikdev.exe -Algorithm SHA256).Hash.ToLower()
$utf8 = New-Object System.Text.UTF8Encoding $false
[IO.File]::WriteAllText("$dist\SHA256SUMS.txt", "$setupHash $name`n$exeHash karadelikdev.exe`n", $utf8)

$summary = (Get-Content $notesFile -Encoding UTF8 | Where-Object { $_ -match '^\s*- ' } | ForEach-Object { ($_ -replace '\*\*', '' -replace '`', '').Trim() }) -join "`n"
$latest = [ordered]@{
    version = $version
    url     = "https://github.com/enesbsafak/karadelikdev/releases/download/v$version/$name"
    sha256  = $setupHash
    notes   = $summary
}
[IO.File]::WriteAllText("$dist\latest.json", ($latest | ConvertTo-Json), $utf8)
Copy-Item $notesFile "$dist\RELEASE_NOTES.md"

Write-Host "Hazır: dist\$name, dist\SHA256SUMS.txt, dist\latest.json"
Write-Host "Önce main'i push edin, sonra: gh release create v$version dist\$name dist\SHA256SUMS.txt dist\latest.json --title `"KaradelikDev $version`" --notes-file $notesFile"
