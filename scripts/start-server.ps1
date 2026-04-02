param(
  [string]$ProjectDir = "",
  [string]$BindHost = "127.0.0.1",
  [string]$UrlHost = "",
  [int]$Port = 0,
  [switch]$Foreground,
  [switch]$Background
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-JsonError {
  param([string]$Message)
  @{ error = $Message } | ConvertTo-Json -Compress
}

function Resolve-BindAddress {
  param([string]$HostName)

  switch ($HostName) {
    "127.0.0.1" { return [System.Net.IPAddress]::Loopback }
    "localhost" { return [System.Net.IPAddress]::Loopback }
    "0.0.0.0" { return [System.Net.IPAddress]::Any }
    default {
      try {
        return [System.Net.IPAddress]::Parse($HostName)
      } catch {
        return [System.Net.IPAddress]::Any
      }
    }
  }
}

function Get-FreeTcpPort {
  param([string]$HostName)

  $listener = [System.Net.Sockets.TcpListener]::new((Resolve-BindAddress $HostName), 0)
  try {
    $listener.Start()
    return ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
  } finally {
    $listener.Stop()
  }
}

$codexHome = if ($env:CODEX_HOME) {
  $env:CODEX_HOME
} else {
  Join-Path $HOME ".codex"
}

$skillScriptsDir = Join-Path $codexHome "skills\brainstorming\scripts"
$serverScript = Join-Path $skillScriptsDir "server.js"

if (-not (Test-Path -LiteralPath $serverScript)) {
  Write-Output (Write-JsonError "Brainstorm server script not found: $serverScript")
  exit 1
}

if ([string]::IsNullOrWhiteSpace($UrlHost)) {
  if ($BindHost -eq "127.0.0.1" -or $BindHost -eq "localhost") {
    $UrlHost = "localhost"
  } else {
    $UrlHost = $BindHost
  }
}

$sessionId = "{0}-{1}" -f $PID, [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()

if ([string]::IsNullOrWhiteSpace($ProjectDir)) {
  $screenDir = Join-Path $env:TEMP ("brainstorm-{0}" -f $sessionId)
} else {
  $projectRoot = (Resolve-Path -LiteralPath $ProjectDir).Path
  $screenDir = Join-Path $projectRoot (".superpowers\brainstorm\{0}" -f $sessionId)
}

New-Item -ItemType Directory -Path $screenDir -Force | Out-Null

$pidFile = Join-Path $screenDir ".server.pid"
$infoFile = Join-Path $screenDir ".server-info"
$logFile = Join-Path $screenDir ".server.log"
$errLogFile = Join-Path $screenDir ".server.err.log"

$envBackup = @{
  BRAINSTORM_DIR = $env:BRAINSTORM_DIR
  BRAINSTORM_HOST = $env:BRAINSTORM_HOST
  BRAINSTORM_URL_HOST = $env:BRAINSTORM_URL_HOST
  BRAINSTORM_PORT = $env:BRAINSTORM_PORT
  BRAINSTORM_OWNER_PID = $env:BRAINSTORM_OWNER_PID
}

try {
  $env:BRAINSTORM_DIR = $screenDir
  $env:BRAINSTORM_HOST = $BindHost
  $env:BRAINSTORM_URL_HOST = $UrlHost
  Remove-Item Env:BRAINSTORM_OWNER_PID -ErrorAction SilentlyContinue

  $selectedPort = if ($Port -gt 0) { $Port } else { Get-FreeTcpPort $BindHost }
  $env:BRAINSTORM_PORT = [string]$selectedPort

  if ($Foreground.IsPresent -and -not $Background.IsPresent) {
    Set-Content -LiteralPath $pidFile -Value $PID -Encoding UTF8
    & node $serverScript
    exit $LASTEXITCODE
  }

  for ($attempt = 0; $attempt -lt 5; $attempt++) {
    if ($attempt -gt 0 -and $Port -le 0) {
      $selectedPort = Get-FreeTcpPort $BindHost
      $env:BRAINSTORM_PORT = [string]$selectedPort
    }

    Remove-Item -LiteralPath $infoFile -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $logFile -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $errLogFile -Force -ErrorAction SilentlyContinue

    $process = Start-Process `
      -FilePath "node" `
      -ArgumentList @($serverScript) `
      -WorkingDirectory $skillScriptsDir `
      -WindowStyle Hidden `
      -RedirectStandardOutput $logFile `
      -RedirectStandardError $errLogFile `
      -PassThru

    Set-Content -LiteralPath $pidFile -Value $process.Id -Encoding UTF8

    for ($i = 0; $i -lt 50; $i++) {
      if (Test-Path -LiteralPath $infoFile) {
        Get-Content -LiteralPath $infoFile -Encoding UTF8
        exit 0
      }

      $process.Refresh()
      if ($process.HasExited) {
        $stderr = if (Test-Path -LiteralPath $errLogFile) {
          (Get-Content -LiteralPath $errLogFile -Raw -Encoding UTF8).Trim()
        } else {
          ""
        }
        $canRetry = $Port -le 0 -and $stderr -match "EACCES: permission denied"
        if (-not $canRetry -or $attempt -eq 4) {
          $message = if ([string]::IsNullOrWhiteSpace($stderr)) {
            "Server exited before writing .server-info"
          } else {
            "Server exited before writing .server-info: $stderr"
          }
          Write-Output (Write-JsonError $message)
          exit 1
        }
        break
      }

      Start-Sleep -Milliseconds 100
    }

    if ($process.HasExited) {
      continue
    }
  }

  Write-Output (Write-JsonError "Server failed to start within 5 seconds")
  exit 1
} finally {
  foreach ($entry in $envBackup.GetEnumerator()) {
    if ($null -eq $entry.Value) {
      Remove-Item ("Env:{0}" -f $entry.Key) -ErrorAction SilentlyContinue
    } else {
      Set-Item ("Env:{0}" -f $entry.Key) -Value $entry.Value
    }
  }
}
