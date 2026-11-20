<#
.SYNOPSIS
  Captures real Snapora UI screenshots into docs/screenshots for the README.
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root "docs\screenshots"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class DocCapture {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
  [DllImport("user32.dll")] public static extern void keybd_event(byte a, byte b, uint c, UIntPtr d);
}
'@

$script:mainHandle = $null

function Restore-Snapora($proc) {
  $rootEl = [System.Windows.Automation.AutomationElement]::RootElement
  $c1 = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $proc.Id)
  $wins = $rootEl.FindAll([System.Windows.Automation.TreeScope]::Children, $c1)
  foreach ($w in $wins) {
    if ($w.Current.Name -eq 'Snapora') {
      $h = [IntPtr]$w.Current.NativeWindowHandle
      [DocCapture]::ShowWindow($h, 9) | Out-Null   # SW_RESTORE
      [DocCapture]::SetForegroundWindow($h) | Out-Null
      $script:mainHandle = $h
      return
    }
  }
  $script:mainHandle = $proc.MainWindowHandle
}

function Save-Window([string]$name, $proc) {
  $h = if ($script:mainHandle) { $script:mainHandle } else { $proc.MainWindowHandle }
  [DocCapture]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 900
  $r = New-Object DocCapture+RECT
  [DocCapture]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
  $path = Join-Path $outDir "$name.png"
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Host "  saved $name.png ($w x $h)"
}

function Click-Element([string]$name) {
  $rootEl = [System.Windows.Automation.AutomationElement]::RootElement
  $c1 = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, "Snapora")
  $win = $rootEl.FindFirst([System.Windows.Automation.TreeScope]::Children, $c1)
  $c2 = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
  $el = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $c2)
  if (-not $el) {
    $all = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    $el = $all | Where-Object {
      $_.Current.Name -like "$name*" -and
      $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button
    } | Select-Object -First 1
  }
  if ($el) { $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); return $true }
  return $false
}

function Click-Retry([string]$name, [int]$tries = 5) {
  for ($i = 0; $i -lt $tries; $i++) {
    if (Click-Element $name) { return $true }
    Start-Sleep -Milliseconds 500
  }
  return $false
}

function Send-Key([byte]$vk) {
  [DocCapture]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 70
  [DocCapture]::keybd_event($vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 70
  [DocCapture]::keybd_event($vk, 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 70
  [DocCapture]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
}

Get-Process snapora -ErrorAction SilentlyContinue | Stop-Process -Force
$p = Start-Process -FilePath (Join-Path $root "release\Snapora.exe") -PassThru
Write-Host "Waiting for Snapora..."; Start-Sleep -Seconds 7
# Minimise clutter so the gallery shows only Snapora.
(New-Object -ComObject Shell.Application).MinimizeAll()
Start-Sleep -Seconds 1
Restore-Snapora $p
Start-Sleep -Seconds 1

Write-Host "Capturing home..."; Save-Window "home" $p

# Let Brave (showing the new GitHub repo) be the captured background for a clean editor shot.
$brave = Get-Process brave -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if ($brave) { [DocCapture]::SetForegroundWindow($brave.MainWindowHandle) | Out-Null; Start-Sleep -Seconds 1 }
Write-Host "Triggering capture for editor screenshot..."; Send-Key 0x2C; Start-Sleep -Seconds 4
Save-Window "editor" $p

Write-Host "Saving the demo capture so history shows a thumbnail..."; Click-Retry "Save" | Out-Null; Start-Sleep -Seconds 2
Write-Host "Navigating to history..."; Click-Retry "Cancel and close" | Out-Null; Start-Sleep -Milliseconds 1200
Write-Host ("  history nav clicked=" + (Click-Retry "Capture history")); Start-Sleep -Seconds 2
Save-Window "history" $p

Write-Host "Navigating to settings..."; Write-Host ("  settings nav clicked=" + (Click-Retry "Settings")); Start-Sleep -Seconds 2
Save-Window "settings" $p

Stop-Process -Id $p.Id -Force
Write-Host "Done: $outDir"
