<#
.SYNOPSIS
  Make a portable DarkRaw folder and zip from a release build. No installer tools needed.

.DESCRIPTION
  Copies target\release\darkraw.exe (as DarkRaw.exe), darkraw-cli.exe when it exists,
  portable.txt and the licenses into dist\DarkRaw-portable, then zips it. portable.txt next to the
  exe switches on portable mode: preferences, presets and crash-recovery files go to the
  DarkRawData folder beside the exe and nothing is written to %APPDATA% (see app_dirs.rs).

  Build first:  cargo build --release -p photocraft
  Then:         powershell -ExecutionPolicy Bypass -File packaging\windows\make-portable.ps1

  With the GNU toolchain (WinLibs) the script also warns when the exe needs a DLL that isn't part
  of Windows, which would break the copy on another PC. That check needs objdump on PATH (WinLibs
  has it).
#>
param(
  [string]$Bin = (Join-Path $PSScriptRoot '..\..\target\release'),
  [string]$Out = (Join-Path $PSScriptRoot '..\..\dist\DarkRaw-LD-portable')
)
$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..\..')

$Exe = Join-Path $Bin 'darkraw.exe'
if (-not (Test-Path $Exe)) {
  throw "darkraw.exe not found in $Bin. Run: cargo build --release -p photocraft"
}

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = Resolve-Path $Out

Copy-Item $Exe (Join-Path $Out 'DarkRaw-LD.exe')
$Cli = Join-Path $Bin 'darkraw-cli.exe'
if (Test-Path $Cli) { Copy-Item $Cli $Out }
Copy-Item (Join-Path $PSScriptRoot 'portable.txt') $Out
foreach ($f in 'LICENSE-MIT', 'LICENSE-APACHE', 'ATTRIBUTION.md') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Out }
}

# DLLs the exe loads that Windows itself doesn't ship.
$objdump = Get-Command objdump -ErrorAction SilentlyContinue
if ($objdump) {
  $dlls = & $objdump.Source -p $Exe | Select-String 'DLL Name:' | ForEach-Object { ($_ -split ':', 2)[1].Trim() }
  $extra = $dlls | Where-Object { $_ -match '^(libgcc|libstdc\+\+|libwinpthread|libgomp)' }
  if ($extra) {
    Write-Warning "DarkRaw-LD.exe needs DLLs that aren't part of Windows: $($extra -join ', '). Copy them from your WinLibs mingw64\bin folder into $Out."
  } else {
    Write-Host 'No extra DLLs needed.'
  }
} else {
  Write-Host 'objdump not found: skipped the DLL check.'
}

$Zip = "$Out.zip"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $Out -DestinationPath $Zip
Write-Host "Portable folder: $Out"
Write-Host "Portable zip:    $Zip"
