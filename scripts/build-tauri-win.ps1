$ErrorActionPreference = "Stop"

function Resolve-VsDevCmd {
  $candidates = @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
  )

  foreach ($path in $candidates) {
    if (Test-Path $path) {
      return $path
    }
  }
  return $null
}

function Resolve-Kernel32Lib {
  $windowsKitsLib = "C:\Program Files (x86)\Windows Kits\10\Lib"
  if (!(Test-Path $windowsKitsLib)) {
    return $null
  }

  $versions = Get-ChildItem $windowsKitsLib -Directory | Sort-Object Name -Descending
  foreach ($version in $versions) {
    $candidate = Join-Path $version.FullName "um\x64\kernel32.lib"
    if (Test-Path $candidate) {
      return $candidate
    }
  }
  return $null
}

if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Rust is not installed or not in PATH. Install rustup first."
}

if (!(Get-Command perl -ErrorAction SilentlyContinue)) {
  throw "Perl is required by openssl-sys (vendored OpenSSL). Install Strawberry Perl and reopen terminal."
}

$vsDevCmd = Resolve-VsDevCmd
if (-not $vsDevCmd) {
  throw "VsDevCmd.bat not found. Install Visual Studio 2022 (Community or BuildTools) with C++ desktop workload."
}

$kernel32 = Resolve-Kernel32Lib
if (-not $kernel32) {
  throw "Windows SDK not found (kernel32.lib missing). In Visual Studio Installer, add 'Windows 10/11 SDK' under C++ desktop workload."
}

Write-Host "Using VsDevCmd: $vsDevCmd"
Write-Host "Found kernel32.lib: $kernel32"
Write-Host "Building renderer in PowerShell context..."
pnpm --filter @lazycat/desktop build:web
if ($LASTEXITCODE -ne 0) {
  throw "Renderer build failed with exit code $LASTEXITCODE"
}

Write-Host "Running Tauri build in VS developer environment..."
$cmd = "`"$vsDevCmd`" -arch=x64 && pnpm --filter @lazycat/desktop build:tauri"
cmd /c $cmd
if ($LASTEXITCODE -ne 0) {
  throw "Tauri build failed with exit code $LASTEXITCODE"
}

Write-Host "Tauri build completed."
