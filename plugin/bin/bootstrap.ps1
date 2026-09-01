# Locates a plugin backend: explicit path, cached download, verified GitHub release, or cargo build.
#
#   bootstrap.ps1 <binary-name> <ENV_PREFIX>
#
# Reads <PREFIX>_{BIN,TARGET,VERSION,REPO,SRC,BUILD}. WEZPLUG_* are passed through
# to the binary untouched, so this script never needs to know what they mean.
param(
  [Parameter(Mandatory = $true)][string]$Name,
  [Parameter(Mandatory = $true)][string]$Prefix
)
$ErrorActionPreference = "Stop"

if ($Prefix -notmatch '^[A-Z][A-Z0-9_]*$') { Write-Host "invalid prefix"; exit 1 }
function EnvOf($suffix) { [Environment]::GetEnvironmentVariable("${Prefix}_$suffix") }

$data = Join-Path $env:LOCALAPPDATA $Name
$target = if (EnvOf TARGET) { EnvOf TARGET } else { "x86_64-pc-windows-msvc" }
$version = if (EnvOf VERSION) { EnvOf VERSION } else { "dev" }
$repo = EnvOf REPO
$src = EnvOf SRC
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

if ($target -notmatch '^[A-Za-z0-9._-]+$' -or $version -notmatch '^[A-Za-z0-9._-]+$') {
  Write-Host "invalid ${Prefix}_TARGET or ${Prefix}_VERSION"; exit 1
}

$explicit = EnvOf BIN
if ($explicit -and (Test-Path $explicit)) { & $explicit; exit $LASTEXITCODE }

$bin = Join-Path $data "bin\$Name-$target-$version.exe"
if (Test-Path $bin) { & $bin; exit $LASTEXITCODE }
New-Item -ItemType Directory -Force -Path (Split-Path $bin) | Out-Null

if ($version -ne "dev" -and $repo) {
  $base = "https://github.com/$repo/releases/download/v$version"
  $tmp = "$bin.$PID.tmp"
  Write-Host "downloading $base/$Name-$target.exe"
  try {
    Invoke-WebRequest -Uri "$base/$Name-$target.exe" -OutFile $tmp
    $sums = Invoke-WebRequest -Uri "$base/SHA256SUMS" | Select-Object -ExpandProperty Content
    $expected = ($sums -split "`n" | Where-Object { $_ -match " $Name-$target.exe$" }) -replace ' .*', ''
    $actual = (Get-FileHash -Algorithm SHA256 $tmp).Hash.ToLower()
    if ($expected -and $expected.Trim() -eq $actual) {
      Move-Item -Force $tmp $bin
      & $bin; exit $LASTEXITCODE
    }
    Write-Host "checksum mismatch"
  } catch { Write-Host "download failed" }
  Remove-Item -Force -ErrorAction SilentlyContinue $tmp
}

if ((EnvOf BUILD) -ne "0" -and $src -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Write-Host "building backend"
  cargo build --release --manifest-path (Join-Path $src "Cargo.toml") --target-dir (Join-Path $data "target")
  if ($LASTEXITCODE -eq 0) {
    Copy-Item (Join-Path $data "target\release\$Name.exe") $bin
    & $bin; exit $LASTEXITCODE
  }
  Write-Host "build failed"
}

Write-Host "backend not found`ninstall cargo or set backend.path, then press Enter to retry"
Read-Host | Out-Null
& $PSCommandPath $Name $Prefix; exit $LASTEXITCODE
