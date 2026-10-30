# repeat.ps1 -Prefix base -Runs @(2,3) [-Exe measure_big.exe] [-App voxa.exe]
# Repeats the headless measurement and the app's picture run over the standard studies.
param([string]$Prefix = 'base', [int[]]$Runs = @(2, 3), [string]$Exe = '', [string]$App = '',
      [string]$Head = '', [switch]$NoApp, [switch]$NoIntel)
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Exe) { $Exe = Join-Path $Root 'target\release\examples\measure_big.exe' }
if (-not $App) { $App = Join-Path $Root 'target\release\voxa.exe' }
if (-not $Head) { $Head = Join-Path $Root 'target\big\head300' }
$Sampler = Join-Path $PSScriptRoot 'sample.ps1'
$P = Join-Path $Root 'target\measure\pics'
New-Item -ItemType Directory -Force $P | Out-Null
$B = Join-Path $Root 'target\big'
$studies = @(
  @($Head, 'head300'),
  @("$B\ct", 'ct-folder'), @("$B\ct.zip", 'ct-zip'), @("$B\ct-stored.zip", 'ct-stored'),
  @("$B\body", 'body-folder'), @("$B\body.zip", 'body-zip'))
foreach ($i in $Runs) {
  foreach ($st in $studies) {
    if (-not (Test-Path $st[0])) { continue }
    & $Sampler -Exe $Exe -Arguments @($st[0]) -Label "$Prefix-$($st[1])-rtx-$i" -Env @{WGPU_POWER_PREF='high'} | Out-Null
    Start-Sleep 2
    if (-not $NoIntel -and $st[1] -in @('head300', 'ct-folder', 'body-folder')) {
      & $Sampler -Exe $Exe -Arguments @($st[0]) -Label "$Prefix-$($st[1])-intel-$i" -Env @{WGPU_POWER_PREF='low'} | Out-Null
      Start-Sleep 2
    }
    if (-not $NoApp -and $st[1] -in @('head300', 'ct-folder', 'body-zip')) {
      & $Sampler -Exe $App -Arguments @($st[0]) -Label "app$Prefix-$($st[1])-rtx-$i" -LimitS 120 -Env @{WGPU_POWER_PREF='high'; VOXA_CAPTURE="$P\app$Prefix-$($st[1])-$i.png"; VOXA_CAPTURE_SHOW='vr'; VOXA_CAPTURE_SIZE='1280x800'} | Out-Null
      Start-Sleep 3
    }
  }
}
Write-Host "done"
