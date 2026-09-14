$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

# Win32：置前台 + 取客户区，用于只截取界面本体并做空白检测
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class SpWin32 {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
}
"@

$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'test-output'
New-Item -ItemType Directory -Force $out | Out-Null
$exe = Join-Path $root 'src-tauri/target/release/soundpilot.exe'
if (!(Test-Path $exe)) { throw 'Missing executable' }

$env:SOUNDPILOT_SMOKE = '1'
$p = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
try {
  # 1) 等待原生窗口出现
  $handle = [IntPtr]::Zero
  for ($i = 0; $i -lt 45; $i++) {
    Start-Sleep -Seconds 1
    $p.Refresh()
    if ($p.HasExited) { throw "App exited during startup: $($p.ExitCode)" }
    if ($p.MainWindowHandle -ne 0) { $handle = $p.MainWindowHandle; break }
  }
  if ($handle -eq [IntPtr]::Zero) { throw 'No native window found after 45 seconds' }

  # 2) 置前台并留出 WebView2 首帧绘制时间
  #    （窗口句柄出现 ≠ 内容已绘制；CI 冷启动软件渲染较慢）
  [void][SpWin32]::ShowWindow($handle, 5)          # SW_SHOW
  [void][SpWin32]::SetForegroundWindow($handle)
  Start-Sleep -Seconds 12
  $p.Refresh()
  if ($p.HasExited) { throw "App exited after window shown: $($p.ExitCode)" }

  # 3) 只截客户区（界面本体），避免桌面背景干扰
  $rect = New-Object SpWin32+RECT
  [void][SpWin32]::GetClientRect($handle, [ref]$rect)
  $origin = New-Object SpWin32+POINT
  [void][SpWin32]::ClientToScreen($handle, [ref]$origin)
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  if ($w -le 0 -or $h -le 0) { throw "Invalid client area: ${w}x${h}" }

  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $png = Join-Path $out 'windows-startup.png'
  $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)

  # 4) 空白检测：逐点采样统计"不同颜色数"
  #    已渲染界面必然有多种颜色（文字/图标/卡片/描边）；纯白 WebView2 只有 1~2 种
  $colors = New-Object 'System.Collections.Generic.HashSet[int]'
  $pureWhite = 0
  $total = 0
  for ($y = 0; $y -lt $h; $y += 4) {
    for ($x = 0; $x -lt $w; $x += 4) {
      $c = $bmp.GetPixel($x, $y)
      [void]$colors.Add(($c.R -shl 16) -bor ($c.G -shl 8) -bor $c.B)
      if ($c.R -ge 250 -and $c.G -ge 250 -and $c.B -ge 250) { $pureWhite++ }
      $total++
    }
  }
  $g.Dispose(); $bmp.Dispose()

  $distinct = $colors.Count
  $whiteRatio = if ($total -gt 0) { [math]::Round($pureWhite / $total, 4) } else { 0 }

  @{
    windowTitle        = $p.MainWindowTitle
    windowHandle       = [long]$handle
    exeBytes           = (Get-Item $exe).Length
    alive              = !$p.HasExited
    clientWidth        = $w
    clientHeight       = $h
    distinctColours    = $distinct
    pureWhiteRatio     = $whiteRatio
    screenshot         = 'windows-startup.png'
  } | ConvertTo-Json | Set-Content (Join-Path $out 'smoke.json') -Encoding utf8

  Write-Host "client=${w}x${h} distinctColours=$distinct pureWhiteRatio=$whiteRatio"

  # 5) 判定：颜色过于单一说明界面没渲染出来（白屏）
  if ($distinct -lt 5) {
    throw "UI appears blank: only $distinct distinct colour(s) sampled in client area"
  }
} finally {
  if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }
}
