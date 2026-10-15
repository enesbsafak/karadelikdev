# KaradelikDev sürüm paketi: exe'yi derler, dist\ altında zip, SHA256SUMS.txt ve latest.json üretir.
# latest.json uygulama içi güncelleyicinin okuduğu dosyadır; GitHub sürümüne zip ile birlikte yüklenmelidir.
#
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1            # derle + paketle
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -NoBuild   # mevcut exe'yi paketle
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

$name = "KaradelikDev-$version-windows-x64"
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
Remove-Item -Recurse -Force $stage, "$dist\$name.zip" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item src-tauri\target\release\karadelikdev.exe, LICENSE, README.md $stage
# Windows PowerShell 5.1 Compress-Archive girdi adlarına ters eğik çizgi yazar; tar (bsdtar) standart zip üretir
& "$env:SystemRoot\System32\tar.exe" -a -c -f "$dist\$name.zip" -C $dist $name
if ($LASTEXITCODE -ne 0) { throw 'zip oluşturulamadı' }

$zipHash = (Get-FileHash "$dist\$name.zip" -Algorithm SHA256).Hash.ToLower()
$exeHash = (Get-FileHash "$stage\karadelikdev.exe" -Algorithm SHA256).Hash.ToLower()
$utf8 = New-Object System.Text.UTF8Encoding $false
[IO.File]::WriteAllText("$dist\SHA256SUMS.txt", "$zipHash $name.zip`n$exeHash karadelikdev.exe`n", $utf8)

$summary = (Get-Content $notesFile -Encoding UTF8 | Where-Object { $_ -match '^\s*- ' } | ForEach-Object { ($_ -replace '\*\*', '' -replace '`', '').Trim() }) -join "`n"
$latest = [ordered]@{
    version    = $version
    url        = "https://github.com/enesbsafak/karadelikdev/releases/download/v$version/$name.zip"
    sha256     = $zipHash
    exe_sha256 = $exeHash
    notes      = $summary
}
[IO.File]::WriteAllText("$dist\latest.json", ($latest | ConvertTo-Json), $utf8)
Copy-Item $notesFile "$dist\RELEASE_NOTES.md"

Write-Host "Hazır: dist\$name.zip, dist\SHA256SUMS.txt, dist\latest.json"
Write-Host "Yayın: gh release create v$version dist\$name.zip dist\SHA256SUMS.txt dist\latest.json --title `"KaradelikDev $version`" --notes-file $notesFile"
