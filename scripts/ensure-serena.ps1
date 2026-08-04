[CmdletBinding()]
param(
    [int]$Port = 9121,
    [int]$StartupTimeoutSeconds = 15
)

$ErrorActionPreference = 'Stop'

$listenAddress = '127.0.0.1'
$projectPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$serenaPath = Join-Path $env:USERPROFILE '.local\bin\serena.exe'
$endpoint = 'http://{0}:{1}/mcp' -f $listenAddress, $Port

if (-not (Test-Path -LiteralPath $serenaPath -PathType Leaf)) {
    throw "Serena executable was not found: $serenaPath"
}

function Get-SerenaListener {
    Get-NetTCPConnection `
        -State Listen `
        -LocalAddress $listenAddress `
        -LocalPort $Port `
        -ErrorAction SilentlyContinue |
        Select-Object -First 1
}

function Test-ExpectedSerena {
    param(
        [Parameter(Mandatory = $true)]
        $Listener
    )

    $owner = Get-CimInstance Win32_Process `
        -Filter "ProcessId = $($Listener.OwningProcess)"

    if (-not $owner) {
        return $false
    }

    $commandLine = [string]$owner.CommandLine
    $portPattern = '--port(?:=|\s+)' + [regex]::Escape([string]$Port)
    $projectPattern = [regex]::Escape($projectPath.TrimEnd('\'))

    return (
        ($commandLine -match 'serena\.exe') -and
        ($commandLine -match 'start-mcp-server') -and
        ($commandLine -match $portPattern) -and
        ($commandLine -match $projectPattern)
    )
}

$mutex = [System.Threading.Mutex]::new($false, "Local\LazyCat.Serena.$Port")
$lockAcquired = $false

try {
    $lockAcquired = $mutex.WaitOne([TimeSpan]::FromSeconds(30))
    if (-not $lockAcquired) {
        throw 'Could not acquire the Serena startup lock'
    }

    $listener = Get-SerenaListener

    if ($listener) {
        if (-not (Test-ExpectedSerena $listener)) {
            throw "Port $Port is already occupied by another process"
        }

        Write-Output "[Serena] Reusing existing MCP server: $endpoint"
        return
    }

    Write-Output "[Serena] Starting MCP server for project: $projectPath"

    $process = Start-Process `
        -FilePath $serenaPath `
        -ArgumentList @(
            'start-mcp-server',
            '--transport', 'streamable-http',
            '--host', $listenAddress,
            '--port', $Port,
            '--context', 'codex',
            '--project', $projectPath,
            '--enable-web-dashboard', 'false',
            '--open-web-dashboard', 'false'
        ) `
        -WindowStyle Hidden `
        -PassThru

    $deadline = [DateTime]::UtcNow.AddSeconds($StartupTimeoutSeconds)

    do {
        Start-Sleep -Milliseconds 250

        if ($process.HasExited) {
            throw "Serena exited before listening on port $Port (code $($process.ExitCode))"
        }

        $listener = Get-SerenaListener
    } while (-not $listener -and [DateTime]::UtcNow -lt $deadline)

    if (-not $listener) {
        throw "Serena did not start listening on port $Port within $StartupTimeoutSeconds seconds"
    }

    if (-not (Test-ExpectedSerena $listener)) {
        throw "Port $Port was taken by another process while Serena was starting"
    }

    Write-Output "[Serena] MCP server ready: $endpoint"
}
finally {
    if ($lockAcquired) {
        $mutex.ReleaseMutex()
    }

    $mutex.Dispose()
}
