<#
.SYNOPSIS
  Builds the Snapora standalone EXE and MSI installer on Windows.

.DESCRIPTION
  Prerequisites: Rust (x86_64-pc-windows-gnu or MSVC), Node 20+, WebView2 runtime.
  With the GNU toolchain, a w64devkit installation is used for windres/gcc.
  Set $W64DEVKIT_BIN if yours lives somewhere other than the default.

  Outputs (copied into .\release):
    Snapora.exe          standalone application
    WebView2Loader.dll   required next to the EXE
    Snapora_x64.msi      x64 MSI installer
#>
param(
  [string]$W64DevkitBin = "C:\Users\Opu\AppData\Local\Temp\w64devkit\w64devkit\bin"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (Test-Path $W64DevkitBin) {
  $env:PATH = "$W64DevkitBin;$env:PATH"
  Write-Host "Using w64devkit from $W64DevkitBin"
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) {
  $env:PATH = "$cargoBin;$env:PATH"
  Write-Host "Using cargo from $cargoBin"
}

$npm = "C:\Program Files\nodejs\npm.cmd"
if (-not (Test-Path $npm)) { $npm = "npm.cmd" }

Write-Host "`n[1/4] Building frontend..."
& $npm run build
if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }

Write-Host "`n[2/4] Forcing embedded-frontend refresh..."
(Get-Item "src-tauri\src\lib.rs").LastWriteTime = Get-Date

Write-Host "`n[3/4] Building release binary + MSI (this can take several minutes)..."
& $npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }

Write-Host "`n[4/4] Copying artifacts..."
$release = Join-Path $root "release"
New-Item -ItemType Directory -Path $release -Force | Out-Null
Copy-Item "src-tauri\target\release\snapora.exe" "$release\Snapora.exe" -Force
Copy-Item "src-tauri\target\release\WebView2Loader.dll" "$release\WebView2Loader.dll" -Force
Copy-Item "src-tauri\target\release\bundle\msi\Snapora_1.0.1_x64_en-US.msi" "$release\Snapora_x64.msi" -Force

Write-Host "`nDone. Deliverables in $release"
Get-ChildItem $release | Select-Object Name, @{ n = "MB"; e = { [math]::Round($_.Length / 1MB, 2) } }
