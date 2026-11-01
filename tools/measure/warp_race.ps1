# warp_race.ps1 -Study PATH [-Exe volsa.exe] [-Runs 20] [-Seconds 20]
# Starts Volsa on Windows' software graphics (VOLSA_ADAPTER=software, DirectX 12) with a scan
# opened at launch, without taking focus, and resizes its window every 120 ms while the scan
# loads: what once stopped Volsa when a worker handed the device work as the window set
# itself up. Prints one line a run, and how many runs Volsa's log says it stopped in.
# Use a made-up study (write_phantom), never a real scan.
param([string]$Study, [string]$Exe = '', [int]$Runs = 20, [int]$Seconds = 20)
. (Join-Path $PSScriptRoot 'win.ps1')
if (-not ('Resize9.W' -as [type])) {
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
namespace Resize9 { public static class W { [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags); } }
'@
}
if (-not $Exe) { $Exe = Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path 'target\release\volsa.exe' }
$log = Join-Path $env:LOCALAPPDATA 'Volsa\data\volsa.log'
function SetE($k, $v) { [Environment]::SetEnvironmentVariable($k, $v, 'Process') }
function Stops() {
  if (-not [IO.File]::Exists($log)) { return 0 }
  $fs = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite'); $r = New-Object IO.StreamReader($fs)
  $text = $r.ReadToEnd(); $r.Close()
  ([regex]::Matches($text, 'Volsa stopped at')).Count
}
$stoppedBefore = Stops
foreach ($i in 1..$Runs) {
  SetE 'VOLSA_ADAPTER' 'software'; SetE 'WGPU_BACKEND' 'dx12'
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $procId = [OpenMeasure5.Win]::StartQuietly($Exe, $Study, (Split-Path $Exe))
  SetE 'VOLSA_ADAPTER' $null; SetE 'WGPU_BACKEND' $null
  $p = [System.Diagnostics.Process]::GetProcessById($procId)
  $n = 0; $before = Stops
  while (-not $p.HasExited -and $sw.Elapsed.TotalSeconds -lt $Seconds -and (Stops) -eq $before) {
    $h = [OpenMeasure5.Win]::Find([uint32]$procId)
    if ($h -ne [IntPtr]::Zero) {
      $w = if ($n % 2) { 900 } else { 1000 }
      [void][Resize9.W]::SetWindowPos($h, [IntPtr]1, 0, 0, $w, 650, 0x0002 -bor 0x0010); $n++
    }
    Start-Sleep -Milliseconds 120
  }
  $stopped = (Stops) -gt $before
  if (-not $p.HasExited) { [void][OpenMeasure5.Win]::Close([uint32]$procId); [void]$p.WaitForExit(10000) }
  if (-not $p.HasExited) { $p.Kill() }
  "run {0}: {1} resizes in {2:N1} s, {3}" -f $i, $n, $sw.Elapsed.TotalSeconds, $(if ($stopped) { 'STOPPED' } else { 'went on' })
  Start-Sleep 2
}
"{0} of {1} runs stopped" -f ((Stops) - $stoppedBefore), $Runs
