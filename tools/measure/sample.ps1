# sample.ps1 -Exe PATH -Arguments @(...) -Label NAME [-Env @{K='V'}] [-OutDir DIR] [-LimitS 300]
# Runs one program, samples its private memory and working set every ~25 ms and its graphics
# memory (dedicated and shared, Windows' per-process GPU counters) every ~250 ms, keeps what it
# prints, and writes LABEL.samples.csv, LABEL.out.txt and one summary line to LABEL.summary.txt.
param(
  [string]$Exe, [string[]]$Arguments = @(), [string]$Label, [hashtable]$Env = @{},
  [string]$OutDir = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path 'target\measure\runs'),
  [int]$LimitS = 300
)
New-Item -ItemType Directory -Force $OutDir | Out-Null
if (-not ('BigMeasure.Mem' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace BigMeasure {
public static class Mem {
  [StructLayout(LayoutKind.Sequential)]
  public struct PMC {
    public uint cb; public uint PageFaultCount;
    public UIntPtr PeakWorkingSetSize, WorkingSetSize, QuotaPeakPagedPoolUsage, QuotaPagedPoolUsage,
      QuotaPeakNonPagedPoolUsage, QuotaNonPagedPoolUsage, PagefileUsage, PeakPagefileUsage, PrivateUsage;
  }
  [DllImport("psapi.dll", SetLastError = true)]
  static extern bool GetProcessMemoryInfo(IntPtr h, out PMC c, uint cb);
  public static PMC Get(IntPtr h) {
    PMC c; c.cb = (uint)Marshal.SizeOf(typeof(PMC));
    GetProcessMemoryInfo(h, out c, c.cb); return c;
  }
}
}
'@
}
function MiB($b) { [math]::Round([double]$b / 1MB, 0) }
$psi = New-Object System.Diagnostics.ProcessStartInfo $Exe
foreach ($a in $Arguments) { $psi.ArgumentList.Add($a) }
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.WorkingDirectory = Split-Path $Exe
foreach ($k in $Env.Keys) { $psi.Environment[$k] = $Env[$k] }
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$outTask = $p.StandardOutput.ReadToEndAsync()
$errTask = $p.StandardError.ReadToEndAsync()
$procId = $p.Id
$gpuJob = Start-ThreadJob -ArgumentList $procId, $LimitS -ScriptBlock {
  param($procId, $limit)
  $max = 0.0; $maxShared = 0.0; $n = 0
  $until = (Get-Date).AddSeconds($limit)
  while ((Get-Process -Id $procId -ErrorAction SilentlyContinue) -and (Get-Date) -lt $until) {
    $c = Get-Counter "\GPU Process Memory(pid_$($procId)_*)\Dedicated Usage", "\GPU Process Memory(pid_$($procId)_*)\Shared Usage" -ErrorAction SilentlyContinue
    if ($c) {
      $n++
      $d = ($c.CounterSamples | Where-Object { $_.Path -like '*dedicated usage' } | Measure-Object CookedValue -Sum).Sum
      $s = ($c.CounterSamples | Where-Object { $_.Path -like '*shared usage' } | Measure-Object CookedValue -Sum).Sum
      if ($d -gt $max) { $max = $d }; if ($s -gt $maxShared) { $maxShared = $s }
    }
    Start-Sleep -Milliseconds 200
  }
  [pscustomobject]@{ maxDedicated = $max; maxShared = $maxShared; samples = $n }
}
$samples = New-Object System.Collections.Generic.List[object]
$last = $null
while (-not $p.HasExited -and $sw.Elapsed.TotalSeconds -lt $LimitS) {
  $m = [BigMeasure.Mem]::Get($p.Handle)
  if ($m.cb -ne 0) { $last = $m }
  $samples.Add([pscustomobject]@{ t = [math]::Round($sw.Elapsed.TotalMilliseconds); priv = $m.PrivateUsage.ToUInt64(); ws = $m.WorkingSetSize.ToUInt64() })
  Start-Sleep -Milliseconds 25
}
if (-not $p.HasExited) { $p.Kill(); $killed = $true } else { $killed = $false }
$p.WaitForExit()
$wall = $sw.Elapsed.TotalMilliseconds
$after = [BigMeasure.Mem]::Get($p.Handle)
$g = Receive-Job $gpuJob -Wait -AutoRemoveJob
$out = $outTask.Result + $errTask.Result
$out | Set-Content -Encoding utf8 (Join-Path $OutDir "$Label.out.txt")
$samples | Export-Csv -NoTypeInformation (Join-Path $OutDir "$Label.samples.csv")
$peakPriv = [math]::Max(($samples | Measure-Object priv -Maximum).Maximum, $after.PeakPagefileUsage.ToUInt64())
$peakWs = [math]::Max(($samples | Measure-Object ws -Maximum).Maximum, $after.PeakWorkingSetSize.ToUInt64())
$summary = "{0}: exit {1}{2}, wall {3:N0} ms, cpu {4:N0} ms, peak private {5} MiB, peak working set {6} MiB, GPU dedicated max {7} MiB, shared max {8} MiB" -f $Label, $p.ExitCode, $(if ($killed) { ' (killed)' } else { '' }), $wall, $p.TotalProcessorTime.TotalMilliseconds, (MiB $peakPriv), (MiB $peakWs), (MiB $g.maxDedicated), (MiB $g.maxShared)
$summary | Set-Content -Encoding utf8 (Join-Path $OutDir "$Label.summary.txt")
Write-Host $out
Write-Host $summary
