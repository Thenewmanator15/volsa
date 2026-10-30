# cold.ps1 -Path FILE_OR_FOLDER -Label NAME : drops the files' cached pages, then measures one open.
param([string]$Path, [string]$Label, [string]$Pref = 'high')
$files = if (Test-Path $Path -PathType Container) { Get-ChildItem -Recurse -File $Path } else { Get-Item $Path }
$noBuffering = [System.IO.FileOptions]0x20000000
foreach ($f in $files) {
  $s = New-Object System.IO.FileStream($f.FullName, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::ReadWrite, [int]4096, $noBuffering)
  $s.Dispose()
}
$S = Join-Path $PSScriptRoot 'sample.ps1'
$M = Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path 'target\release\examples\measure_big.exe'
& $S -Exe $M -Arguments @($Path, 'nodraw') -Label $Label -Env @{WGPU_POWER_PREF=$Pref} | Select-String -Pattern 'indexed|first slice|volume|peak' | ForEach-Object { $_.Line }
