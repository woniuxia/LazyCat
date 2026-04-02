param(
  [Parameter(Mandatory = $true)]
  [string]$ScreenDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$resolvedScreenDir = if (Test-Path -LiteralPath $ScreenDir) {
  (Resolve-Path -LiteralPath $ScreenDir).Path
} else {
  [System.IO.Path]::GetFullPath($ScreenDir)
}

$pidFile = Join-Path $resolvedScreenDir ".server.pid"
$logFile = Join-Path $resolvedScreenDir ".server.log"
$errLogFile = Join-Path $resolvedScreenDir ".server.err.log"

if (-not (Test-Path -LiteralPath $pidFile)) {
  @{ status = "not_running" } | ConvertTo-Json -Compress
  exit 0
}

$pidValue = (Get-Content -LiteralPath $pidFile -Encoding UTF8 | Select-Object -First 1).Trim()
if ($pidValue) {
  Stop-Process -Id ([int]$pidValue) -Force -ErrorAction SilentlyContinue
}

Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $logFile -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $errLogFile -Force -ErrorAction SilentlyContinue

$tempRoot = [System.IO.Path]::GetFullPath($env:TEMP).TrimEnd("\")
if ($resolvedScreenDir.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
  Remove-Item -LiteralPath $resolvedScreenDir -Recurse -Force -ErrorAction SilentlyContinue
}

@{ status = "stopped" } | ConvertTo-Json -Compress
