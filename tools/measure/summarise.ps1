# summarise.ps1 -Prefix base : medians (and ranges) over runs of each label, from runs\*.out.txt
# and runs\*.summary.txt.
param([string]$Prefix = 'base')
$R = Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path 'target\measure\runs'
function Med([double[]]$xs) {
  $s = $xs | Sort-Object; $n = $s.Count
  if ($n -eq 0) { return '' }
  $m = if ($n % 2) { $s[[int][math]::Floor($n / 2)] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 }
  if ($n -gt 1) { '{0:N0} ({1:N0}-{2:N0})' -f $m, $s[0], $s[-1] } else { '{0:N0}' -f $m }
}
function MedF([double[]]$xs) {
  $s = $xs | Sort-Object; $n = $s.Count
  if ($n -eq 0) { return '' }
  $m = if ($n % 2) { $s[[int][math]::Floor($n / 2)] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 }
  if ($n -gt 1) { '{0:N1} ({1:N1}-{2:N1})' -f $m, $s[0], $s[-1] } else { '{0:N1}' -f $m }
}
$groups = Get-ChildItem $R -Filter "$Prefix-*.out.txt" | Group-Object { $_.BaseName -replace '-\d+\.out$', '' }
foreach ($g in $groups | Sort-Object Name) {
  $first = @(); $vol = @(); $dev = @(); $peak = @(); $vr = @(); $vr4 = @(); $mip = @(); $dims = ''
  foreach ($f in $g.Group) {
    $t = Get-Content $f.FullName -Raw
    if ($t -match 'first slice\s+(\d+) ms after') { $first += [double]$Matches[1] }
    if ($t -match 'volume\s+(\d+) ms after') { $vol += [double]$Matches[1] }
    if ($t -match '(\[\d+, \d+, \d+\]) on the device, halved (\d+)') { $dims = "$($Matches[1]) h$($Matches[2])" }
    if ($t -match 'Vr 1280x800: ([\d.]+) ms') { $vr += [double]$Matches[1] }
    if ($t -match 'Vr 3840x2160: ([\d.]+) ms') { $vr4 += [double]$Matches[1] }
    if ($t -match 'Mip 1280x800: ([\d.]+) ms') { $mip += [double]$Matches[1] }
    $sum = Get-Content ($f.FullName -replace '\.out\.txt$', '.summary.txt') -Raw
    if ($sum -match 'peak private (\d+) MiB') { $peak += [double]$Matches[1] }
  }
  '{0,-26} first {1,-16} volume {2,-18} peak {3,-20} VR {4,-18} VR4K {5,-18} MIP {6,-16} {7}' -f $g.Name, (Med $first), (Med $vol), (Med $peak), (MedF $vr), (MedF $vr4), (MedF $mip), $dims
}
$apps = Get-ChildItem $R -Filter "app$Prefix-*.summary.txt", "app-*.summary.txt" -ErrorAction SilentlyContinue
$apps = Get-ChildItem $R | Where-Object { $_.Name -like "app$Prefix-*.summary.txt" -or ($Prefix -eq 'base' -and $_.Name -like 'app-*-rtx-1.summary.txt') }
foreach ($g in $apps | Group-Object { $_.BaseName -replace '-\d+\.summary$', '' -replace "^app$Prefix-", 'app-' -replace '^app-', 'app-' } | Sort-Object Name) {
  $peak = @(); $wall = @()
  foreach ($f in $g.Group) {
    $sum = Get-Content $f.FullName -Raw
    if ($sum -match 'peak private (\d+) MiB') { $peak += [double]$Matches[1] }
    if ($sum -match 'wall ([\d,]+) ms') { $wall += [double]($Matches[1] -replace ',', '') }
  }
  '{0,-26} app peak {1,-22} wall to picture {2}' -f $g.Name, (Med $peak), (Med $wall)
}
