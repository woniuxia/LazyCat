[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$TaskPath,

  [Parameter(Mandatory = $true)]
  [ValidateSet("read-only-analysis", "implementation")]
  [string]$TaskType,

  [ValidateSet("xhigh", "high", "medium", "low")]
  [string]$ReasoningEffort = "xhigh",

  [string]$DowngradeReason = "none",

  [string]$RepoRoot,

  [Parameter(Mandatory = $true)]
  [string]$ResultPath,

  [string]$EventLogPath,

  [string]$StderrLogPath,

  [ValidateRange(1, 86400)]
  [int]$TimeoutSeconds = 3600,

  [ValidateRange(1, 86400)]
  [int]$HeartbeatSeconds = 60,

  [ValidateRange(0, 10)]
  [int]$MaxResumeAttempts = 1,

  [ValidateRange(0, 86400)]
  [int]$RetryDelaySeconds = 15,

  [string]$CodexCommand = "codex",

  [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"
$model = "gpt-5.6-luna"
$requiredSections = @(
  "Task Type",
  "Goal",
  "Context",
  "Confirmed Decisions",
  "Executable Design",
  "Non-goals",
  "Allowed Scope",
  "Allowed Changed Paths",
  "Forbidden Scope",
  "Steps",
  "Acceptance Criteria",
  "Validation",
  "Stop Conditions",
  "Output Requirements"
)
$resultProperties = @(
  "taskType",
  "reasoningEffort",
  "status",
  "summary",
  "scopeInspected",
  "changedFiles",
  "commands",
  "validation",
  "findings",
  "uninspectedScope",
  "scopeDeviations",
  "remainingIssues"
)

function Resolve-ExistingPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  if (!(Test-Path -LiteralPath $Path)) {
    throw "$Label does not exist: $Path"
  }

  return (Resolve-Path -LiteralPath $Path).Path
}

function Get-TaskSection {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Content,

    [Parameter(Mandatory = $true)]
    [string]$Section
  )

  $escapedSection = [Regex]::Escape($Section)
  $pattern = "(?ms)^##[ \t]+$escapedSection[ \t]*\r?\n(?<body>.*?)(?=^##[ \t]+|\z)"
  $match = [Regex]::Match($Content, $pattern)
  if (!$match.Success -or [string]::IsNullOrWhiteSpace($match.Groups["body"].Value)) {
    throw "Task packet section is missing or empty: $Section"
  }

  return $match.Groups["body"].Value.Trim()
}

function ConvertTo-RepositoryPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  if ([string]::IsNullOrWhiteSpace($Path) -or [IO.Path]::IsPathRooted($Path) -or $Path.IndexOfAny(@('*', '?')) -ge 0) {
    throw "$Label must be an exact repository-relative file path: '$Path'"
  }

  $rootValue = $Root.TrimEnd('\', '/')
  $rootPrefix = $rootValue + [IO.Path]::DirectorySeparatorChar
  $fullPath = [IO.Path]::GetFullPath((Join-Path $rootValue $Path))
  if (!$fullPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "$Label escapes the repository: '$Path'"
  }

  $relativePath = $fullPath.Substring($rootPrefix.Length).Replace('\', '/')
  if ([string]::IsNullOrWhiteSpace($relativePath) -or $relativePath.EndsWith('/')) {
    throw "$Label must name a file: '$Path'"
  }
  return $relativePath
}

function Get-AllowedChangedPaths {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Content,

    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string]$Type
  )

  $body = Get-TaskSection -Content $Content -Section "Allowed Changed Paths"
  if (!$body.TrimStart().StartsWith('[')) {
    throw "Allowed Changed Paths must be a JSON array."
  }

  try {
    $parsedValues = ConvertFrom-Json -InputObject $body -ErrorAction Stop
    $values = @($parsedValues)
  } catch {
    throw "Allowed Changed Paths is not valid JSON: $($_.Exception.Message)"
  }

  $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
  foreach ($value in $values) {
    if (!($value -is [string])) {
      throw "Allowed Changed Paths entries must be strings."
    }
    $normalized = ConvertTo-RepositoryPath -Path $value -Root $Root -Label "Allowed changed path"
    if (!$paths.Add($normalized)) {
      throw "Allowed Changed Paths contains a duplicate: $normalized"
    }
  }

  if ($Type -eq "read-only-analysis" -and $paths.Count -ne 0) {
    throw "Read-only task packets must use an empty Allowed Changed Paths array."
  }
  Write-Output -NoEnumerate $paths
}

function Get-GitValue {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string[]]$Arguments,

    [Parameter(Mandatory = $true)]
    [string]$Action
  )

  $output = @(& git -C $Root @Arguments)
  if ($LASTEXITCODE -ne 0) {
    throw "$Action failed with exit code $LASTEXITCODE"
  }

  return $output -join "`n"
}

function Assert-NoReparsePoint {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  $current = Get-Item -LiteralPath $Path -Force
  while ($null -ne $current) {
    if (($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "$Label cannot use a reparse-point path: $($current.FullName)"
    }
    $current = $current.Parent
  }
}

function Test-PathInsideRoot {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Root
  )

  $rootValue = $Root.TrimEnd('\', '/')
  $rootPrefix = $rootValue + [IO.Path]::DirectorySeparatorChar
  return $Path.Equals($rootValue, [StringComparison]::OrdinalIgnoreCase) -or
    $Path.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)
}

function Resolve-NewExternalFilePath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  $fullPath = [IO.Path]::GetFullPath($Path)
  $directory = Split-Path -Parent $fullPath
  if (!(Test-Path -LiteralPath $directory -PathType Container)) {
    throw "$Label directory does not exist: $directory"
  }

  Assert-NoReparsePoint -Path $directory -Label "$Label directory"
  $resolvedDirectory = (Resolve-Path -LiteralPath $directory).Path
  $resolvedPath = Join-Path $resolvedDirectory (Split-Path -Leaf $fullPath)
  if (Test-PathInsideRoot -Path $resolvedPath -Root $Root) {
    throw "$Label must be outside the repository: $resolvedPath"
  }
  if (Test-Path -LiteralPath $resolvedPath) {
    throw "$Label already exists: $resolvedPath"
  }

  return $resolvedPath
}

function Assert-ExternalTaskPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Root
  )

  if (Test-PathInsideRoot -Path $Path -Root $Root) {
    throw "Task packet must be outside the repository for execution: $Path"
  }

  $item = Get-Item -LiteralPath $Path -Force
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Task packet cannot be a reparse point: $Path"
  }
  Assert-NoReparsePoint -Path $item.Directory.FullName -Label "Task packet directory"
}

function Get-GitLines {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string[]]$Arguments,

    [Parameter(Mandatory = $true)]
    [string]$Action
  )

  $output = @(& git -C $Root @Arguments)
  if ($LASTEXITCODE -ne 0) {
    throw "$Action failed with exit code $LASTEXITCODE"
  }
  return @($output | Where-Object { ![string]::IsNullOrWhiteSpace($_) })
}

function Get-WorkspaceSnapshot {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root
  )

  $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
  $trackedChanges = Get-GitLines -Root $Root -Arguments @(
    "-c", "core.quotepath=false", "-c", "core.safecrlf=false", "diff", "--name-only", "--no-renames", "HEAD", "--"
  ) -Action "List tracked worktree changes"
  $untrackedChanges = Get-GitLines -Root $Root -Arguments @(
    "-c", "core.quotepath=false", "ls-files", "--others", "--exclude-standard", "--"
  ) -Action "List untracked worktree changes"

  foreach ($path in @($trackedChanges) + @($untrackedChanges)) {
    $null = $paths.Add((ConvertTo-RepositoryPath -Path $path -Root $Root -Label "Git changed path"))
  }

  $snapshot = @{}
  foreach ($path in $paths) {
    $fullPath = Join-Path $Root $path.Replace('/', [IO.Path]::DirectorySeparatorChar)
    $worktreeState = "missing"
    if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
      $hash = (Get-FileHash -LiteralPath $fullPath -Algorithm SHA256).Hash
      $worktreeState = "file:$hash"
    } elseif (Test-Path -LiteralPath $fullPath -PathType Container) {
      $worktreeState = "directory"
    }
    $indexState = Get-GitValue -Root $Root -Arguments @("ls-files", "-s", "--", $path) -Action "Read index state for $path"
    $statusState = Get-GitValue -Root $Root -Arguments @(
      "-c", "core.quotepath=false", "status", "--porcelain=v1", "--untracked-files=all", "--", $path
    ) -Action "Read status state for $path"
    $snapshot[$path] = "$statusState|$indexState|$worktreeState"
  }
  return $snapshot
}

function Compare-WorkspaceSnapshots {
  param(
    [Parameter(Mandatory = $true)]
    [hashtable]$Before,

    [Parameter(Mandatory = $true)]
    [hashtable]$After
  )

  $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
  foreach ($path in $Before.Keys) { $null = $paths.Add($path) }
  foreach ($path in $After.Keys) { $null = $paths.Add($path) }

  $changed = [Collections.Generic.List[string]]::new()
  foreach ($path in $paths) {
    $beforeValue = if ($Before.ContainsKey($path)) { $Before[$path] } else { $null }
    $afterValue = if ($After.ContainsKey($path)) { $After[$path] } else { $null }
    if ($beforeValue -cne $afterValue) {
      $changed.Add($path)
    }
  }
  return @($changed | Sort-Object)
}

function Assert-StringArray {
  param(
    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [object]$Value,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  if ($null -eq $Value -or !($Value -is [Array])) {
    throw "Executor result $Label must be an array."
  }
  foreach ($entry in $Value) {
    if (!($entry -is [string])) {
      throw "Executor result $Label entries must be strings."
    }
  }
}

function Assert-ObjectArray {
  param(
    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [object]$Value,

    [Parameter(Mandatory = $true)]
    [string]$Label,

    [Parameter(Mandatory = $true)]
    [string[]]$RequiredProperties
  )

  if ($null -eq $Value -or !($Value -is [Array])) {
    throw "Executor result $Label must be an array."
  }
  foreach ($entry in $Value) {
    if ($null -eq $entry -or !($entry -is [PSCustomObject])) {
      throw "Executor result $Label entries must be objects."
    }
    $names = @($entry.PSObject.Properties.Name)
    foreach ($property in $RequiredProperties) {
      if ($names -notcontains $property) {
        throw "Executor result $Label entry is missing property: $property"
      }
    }
    foreach ($name in $names) {
      if ($RequiredProperties -notcontains $name) {
        throw "Executor result $Label entry has unexpected property: $name"
      }
    }
  }
}

function Assert-ExecutorResult {
  param(
    [Parameter(Mandatory = $true)]
    [object]$Result,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedTaskType,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedEffort
  )

  $names = @($Result.PSObject.Properties.Name)
  foreach ($property in $resultProperties) {
    if ($names -notcontains $property) {
      throw "Executor result is missing property: $property"
    }
  }
  foreach ($name in $names) {
    if ($resultProperties -notcontains $name) {
      throw "Executor result has unexpected property: $name"
    }
  }

  if ($Result.taskType -ne $ExpectedTaskType) {
    throw "Executor result taskType mismatch. Expected '$ExpectedTaskType', got '$($Result.taskType)'."
  }
  if ($Result.reasoningEffort -ne $ExpectedEffort) {
    throw "Executor result reasoningEffort mismatch. Expected '$ExpectedEffort', got '$($Result.reasoningEffort)'."
  }
  if (@("success", "failed", "blocked") -notcontains $Result.status) {
    throw "Executor result status is invalid: '$($Result.status)'."
  }
  if (!($Result.summary -is [string])) {
    throw "Executor result summary must be a string."
  }

  foreach ($property in @("scopeInspected", "changedFiles", "uninspectedScope", "scopeDeviations", "remainingIssues")) {
    Assert-StringArray -Value $Result.$property -Label $property
  }
  Assert-ObjectArray -Value $Result.commands -Label "commands" -RequiredProperties @("command", "exitCode", "purpose")
  Assert-ObjectArray -Value $Result.validation -Label "validation" -RequiredProperties @("name", "status", "evidence")
  Assert-ObjectArray -Value $Result.findings -Label "findings" -RequiredProperties @("severity", "summary", "evidence")

  foreach ($entry in $Result.commands) {
    if (!($entry.command -is [string]) -or !($entry.purpose -is [string]) -or !($entry.exitCode -is [ValueType])) {
      throw "Executor result commands entry has invalid value types."
    }
  }
  foreach ($entry in $Result.validation) {
    if (@("passed", "failed", "not-run") -notcontains $entry.status) {
      throw "Executor result validation status is invalid: '$($entry.status)'."
    }
  }
  foreach ($entry in $Result.findings) {
    if (@("blocker", "high", "medium", "low", "info") -notcontains $entry.severity) {
      throw "Executor result finding severity is invalid: '$($entry.severity)'."
    }
  }
}

function Get-WriterMutexName {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root
  )

  $sha256 = [Security.Cryptography.SHA256]::Create()
  try {
    $bytes = [Text.Encoding]::UTF8.GetBytes($Root.ToUpperInvariant())
    $hash = ([BitConverter]::ToString($sha256.ComputeHash($bytes))).Replace('-', '')
    return "Local\LazyCat.SolLuna.Writer.$($hash.Substring(0, 32))"
  } finally {
    $sha256.Dispose()
  }
}

function Acquire-WriterMutex {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root
  )

  $mutex = [Threading.Mutex]::new($false, (Get-WriterMutexName -Root $Root))
  try {
    if (!$mutex.WaitOne(0)) {
      $mutex.Dispose()
      throw "Another implementation Luna is already running for repository: $Root"
    }
  } catch [Threading.AbandonedMutexException] {
    # An abandoned mutex is acquired by this caller.
  }
  return $mutex
}

function ConvertTo-ProcessArgument {
  param(
    [AllowEmptyString()]
    [string]$Argument
  )

  if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') {
    return $Argument
  }

  $builder = [Text.StringBuilder]::new()
  $null = $builder.Append('"')
  $backslashes = 0
  foreach ($character in $Argument.ToCharArray()) {
    if ($character -eq '\') {
      $backslashes++
      continue
    }
    if ($character -eq '"') {
      $null = $builder.Append(('\' * (($backslashes * 2) + 1)))
      $null = $builder.Append('"')
      $backslashes = 0
      continue
    }
    if ($backslashes -gt 0) {
      $null = $builder.Append(('\' * $backslashes))
      $backslashes = 0
    }
    $null = $builder.Append($character)
  }
  if ($backslashes -gt 0) {
    $null = $builder.Append(('\' * ($backslashes * 2)))
  }
  $null = $builder.Append('"')
  return $builder.ToString()
}

function Start-CodexProcess {
  param(
    [Parameter(Mandatory = $true)]
    [System.Management.Automation.CommandInfo]$Command,

    [Parameter(Mandatory = $true)]
    [string]$RunnerPath,

    [Parameter(Mandatory = $true)]
    [string]$ArgumentsPath,

    [Parameter(Mandatory = $true)]
    [string]$ExitCodePath,

    [Parameter(Mandatory = $true)]
    [string]$PromptPath,

    [Parameter(Mandatory = $true)]
    [string]$StdoutPath,

    [Parameter(Mandatory = $true)]
    [string]$StderrPath
  )

  if (@(
      [Management.Automation.CommandTypes]::Application,
      [Management.Automation.CommandTypes]::ExternalScript
    ) -notcontains $Command.CommandType) {
    throw "CodexCommand must resolve to an application or external script: $($Command.CommandType)"
  }

  $filePath = (Get-Process -Id $PID).Path
  $processArguments = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $RunnerPath,
    "-CommandPath", $Command.Source,
    "-ArgumentsPath", $ArgumentsPath,
    "-ExitCodePath", $ExitCodePath
  )
  $quotedArguments = @($processArguments | ForEach-Object { ConvertTo-ProcessArgument -Argument $_ })
  $startParameters = @{
    FilePath = $filePath
    ArgumentList = $quotedArguments
    RedirectStandardInput = $PromptPath
    RedirectStandardOutput = $StdoutPath
    RedirectStandardError = $StderrPath
    PassThru = $true
  }
  if ($env:OS -eq "Windows_NT") {
    $startParameters.WindowStyle = "Hidden"
  }
  return Start-Process @startParameters
}

function Stop-ProcessTree {
  param(
    [Parameter(Mandatory = $true)]
    [Diagnostics.Process]$Process
  )

  if ($Process.HasExited) {
    return
  }
  if ($env:OS -eq "Windows_NT") {
    & taskkill.exe /PID $Process.Id /T /F 2>$null | Out-Null
  }
  if (!$Process.HasExited) {
    Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
  }
  $null = $Process.WaitForExit(5000)
}

function Get-AttemptPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$CanonicalPath,

    [Parameter(Mandatory = $true)]
    [int]$AttemptNumber
  )

  if ($AttemptNumber -eq 1) {
    return $CanonicalPath
  }

  $directory = Split-Path -Parent $CanonicalPath
  $extension = [IO.Path]::GetExtension($CanonicalPath)
  $stem = [IO.Path]::GetFileNameWithoutExtension($CanonicalPath)
  return Join-Path $directory ("{0}.attempt-{1}{2}" -f $stem, $AttemptNumber, $extension)
}

function Get-UsageTokenValue {
  param(
    [AllowNull()]
    [object]$Usage,

    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  if ($null -eq $Usage) {
    return [int64]0
  }
  $value = if ($Usage -is [System.Collections.IDictionary]) {
    $Usage[$Name]
  } else {
    $property = $Usage.PSObject.Properties[$Name]
    if ($null -eq $property) { $null } else { $property.Value }
  }
  $parsed = [int64]0
  if ($null -ne $value -and [int64]::TryParse([string]$value, [ref]$parsed)) {
    return $parsed
  }
  return [int64]0
}

function ConvertTo-UsageRecord {
  param([AllowNull()][object]$Usage)

  if ($null -eq $Usage) {
    return $null
  }
  return [ordered]@{
    input_tokens = Get-UsageTokenValue -Usage $Usage -Name "input_tokens"
    cached_input_tokens = Get-UsageTokenValue -Usage $Usage -Name "cached_input_tokens"
    output_tokens = Get-UsageTokenValue -Usage $Usage -Name "output_tokens"
    reasoning_output_tokens = Get-UsageTokenValue -Usage $Usage -Name "reasoning_output_tokens"
  }
}

function Add-UsageRecords {
  param(
    [AllowNull()][object]$Left,
    [AllowNull()][object]$Right
  )

  if ($null -eq $Left -and $null -eq $Right) {
    return $null
  }
  return [ordered]@{
    input_tokens = (Get-UsageTokenValue -Usage $Left -Name "input_tokens") + (Get-UsageTokenValue -Usage $Right -Name "input_tokens")
    cached_input_tokens = (Get-UsageTokenValue -Usage $Left -Name "cached_input_tokens") + (Get-UsageTokenValue -Usage $Right -Name "cached_input_tokens")
    output_tokens = (Get-UsageTokenValue -Usage $Left -Name "output_tokens") + (Get-UsageTokenValue -Usage $Right -Name "output_tokens")
    reasoning_output_tokens = (Get-UsageTokenValue -Usage $Left -Name "reasoning_output_tokens") + (Get-UsageTokenValue -Usage $Right -Name "reasoning_output_tokens")
  }
}

function Get-AttemptEventMetadata {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [ref]$ObservedLength
  )

  $latestEventUtc = $null
  $hasNewContent = $false
  $readError = $null
  $eventBytes = [int64]0
  if (Test-Path -LiteralPath $Path -PathType Leaf) {
    try {
      $item = Get-Item -LiteralPath $Path -Force
      $eventBytes = [int64]$item.Length
      if ($eventBytes -ne $ObservedLength.Value) {
        $hasNewContent = $true
        $latestEventUtc = $item.LastWriteTimeUtc
        $ObservedLength.Value = $eventBytes
      }
    } catch {
      $readError = $_.Exception.Message
    }
  }

  return [PSCustomObject]@{
    LatestEventUtc = $latestEventUtc
    HasNewContent = $hasNewContent
    EventBytes = $eventBytes
    ReadError = $readError
  }
}

function Get-ThreadIdFromEventHead {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [int]$MaxLines = 64,
    [int64]$MaxBytes = 65536
  )

  if (!(Test-Path -LiteralPath $Path -PathType Leaf)) {
    return $null
  }
  $stream = $null
  $reader = $null
  try {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8, $true, 4096, $false)
    $lineCount = 0
    while (!$reader.EndOfStream -and $lineCount -lt $MaxLines -and $stream.Position -le $MaxBytes) {
      $lineCount++
      $line = $reader.ReadLine()
      try {
        $event = $line | ConvertFrom-Json -ErrorAction Stop
        if ($event.type -eq "thread.started" -and $event.thread_id -is [string] -and ![string]::IsNullOrWhiteSpace($event.thread_id)) {
          return $event.thread_id
        }
      } catch {
        # The last line may still be in flight; retry the bounded head scan after growth.
      }
    }
  } catch {
    return $null
  } finally {
    if ($null -ne $reader) { $reader.Dispose() }
    elseif ($null -ne $stream) { $stream.Dispose() }
  }
  return $null
}

function Get-CompletedEventSummary {
  param([Parameter(Mandatory = $true)][string]$Path)

  $threadId = $null
  $usage = $null
  $commandCount = 0
  $failedCommandCount = 0
  $eventBytes = [int64]0
  $latestEventUtc = $null
  $readError = $null
  $stream = $null
  $reader = $null
  if (Test-Path -LiteralPath $Path -PathType Leaf) {
    try {
      $item = Get-Item -LiteralPath $Path -Force
      $eventBytes = [int64]$item.Length
      $latestEventUtc = $item.LastWriteTimeUtc
      $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
      $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8, $true, 4096, $false)
      while (!$reader.EndOfStream) {
        $line = $reader.ReadLine()
        try {
          $event = $line | ConvertFrom-Json -ErrorAction Stop
          if ($event.type -eq "thread.started" -and $event.thread_id -is [string] -and ![string]::IsNullOrWhiteSpace($event.thread_id)) {
            $threadId = $event.thread_id
          }
          if ($event.type -eq "turn.completed" -and $null -ne $event.usage) {
            $usage = Add-UsageRecords -Left $usage -Right (ConvertTo-UsageRecord -Usage $event.usage)
          }
          if ($event.type -eq "item.completed" -and $null -ne $event.item -and $event.item.type -eq "command_execution") {
            $commandCount++
            $exitCodeProperty = $event.item.PSObject.Properties["exit_code"]
            $statusProperty = $event.item.PSObject.Properties["status"]
            if (($null -ne $exitCodeProperty -and [int]$exitCodeProperty.Value -ne 0) -or ($null -ne $statusProperty -and $statusProperty.Value -eq "failed")) {
              $failedCommandCount++
            }
          }
        } catch {
          # Ignore non-JSON diagnostics and a final partial line; stderr remains authoritative.
        }
      }
    } catch {
      $readError = $_.Exception.Message
    } finally {
      if ($null -ne $reader) { $reader.Dispose() }
      elseif ($null -ne $stream) { $stream.Dispose() }
    }
  }

  return [PSCustomObject]@{
    ThreadId = $threadId
    Usage = $usage
    CommandCount = $commandCount
    FailedCommandCount = $failedCommandCount
    EventBytes = $eventBytes
    LatestEventUtc = $latestEventUtc
    ReadError = $readError
  }
}

function Write-Heartbeat {
  param(
    [Parameter(Mandatory = $true)]
    [DateTime]$ExecutionStartedUtc,

    [Parameter(Mandatory = $true)]
    [int]$AttemptNumber,

    [Parameter(Mandatory = $true)]
    [string]$AttemptKind,

    [Parameter(Mandatory = $true)]
    [string]$ProcessState,

    [string]$SessionId,

    [DateTime]$LatestEventUtc
  )

  $elapsedSeconds = [math]::Floor(([DateTime]::UtcNow - $ExecutionStartedUtc).TotalSeconds)
  $sessionText = if ([string]::IsNullOrWhiteSpace($SessionId)) { "pending" } else { $SessionId }
  $eventAge = if ($LatestEventUtc.Year -eq 1) {
    "pending"
  } else {
    [math]::Max(0, [math]::Floor(([DateTime]::UtcNow - $LatestEventUtc).TotalSeconds)).ToString([Globalization.CultureInfo]::InvariantCulture) + "s"
  }
  Write-Host ("HEARTBEAT elapsed={0}s attempt={1} kind={2} process={3} session={4} latest-event-age={5}" -f $elapsedSeconds, $AttemptNumber, $AttemptKind, $ProcessState, $sessionText, $eventAge)
}

function Write-StateManifest {
  param(
    [Parameter(Mandatory = $true)]
    [System.Collections.IDictionary]$State,

    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  $State.updatedUtc = [DateTime]::UtcNow.ToString("o", [Globalization.CultureInfo]::InvariantCulture)
  $temporaryPath = "{0}.tmp-{1}-{2}" -f $Path, $PID, [Guid]::NewGuid().ToString('N')
  $backupPath = "{0}.bak-{1}-{2}" -f $Path, $PID, [Guid]::NewGuid().ToString('N')
  try {
    $json = ConvertTo-Json -InputObject $State -Depth 12 -Compress
    [IO.File]::WriteAllText($temporaryPath, $json, [Text.UTF8Encoding]::new($false))
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
      [IO.File]::Replace($temporaryPath, $Path, $backupPath)
    } else {
      [IO.File]::Move($temporaryPath, $Path)
    }
  } catch {
    throw "State manifest update failed at ${Path}: $($_.Exception.Message)"
  } finally {
    if (Test-Path -LiteralPath $temporaryPath) {
      Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $backupPath) {
      Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
    }
  }
}

function Assert-RetrySafety {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string]$BaselineHead,

    [Parameter(Mandatory = $true)]
    [hashtable]$BaselineSnapshot,

    [Parameter(Mandatory = $true)]
    [string]$Type,

    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [Collections.Generic.HashSet[string]]$AllowedChangedPaths,

    [Parameter(Mandatory = $true)]
    [bool]$SessionAvailable
  )

  $currentHead = Get-GitValue -Root $Root -Arguments @("rev-parse", "HEAD") -Action "Read retry HEAD"
  if ($currentHead -ne $BaselineHead) {
    throw "Retry blocked because HEAD moved. Baseline: $BaselineHead; current: $currentHead."
  }
  $currentSnapshot = Get-WorkspaceSnapshot -Root $Root
  $delta = @(Compare-WorkspaceSnapshots -Before $BaselineSnapshot -After $currentSnapshot)
  if ($Type -eq "read-only-analysis" -and $delta.Count -gt 0) {
    throw "Retry blocked because the read-only workspace changed: $($delta -join ', ')"
  }
  if ($Type -eq "implementation") {
    if (!$SessionAvailable -and $delta.Count -gt 0) {
      throw "Retry blocked because an implementation changed the workspace before a session ID was captured: $($delta -join ', ')"
    }
    $outsideScope = @($delta | Where-Object { !$AllowedChangedPaths.Contains($_) })
    if ($outsideScope.Count -gt 0) {
      throw "Retry blocked because current changes are outside Allowed Changed Paths: $($outsideScope -join ', ')"
    }
  }
  return $delta
}

function Merge-AttemptLogs {
  param(
    [Parameter(Mandatory = $true)]
    [string]$CanonicalEventPath,

    [Parameter(Mandatory = $true)]
    [string]$CanonicalStderrPath,

    [Parameter(Mandatory = $true)]
    [string]$AttemptEventPath,

    [Parameter(Mandatory = $true)]
    [string]$AttemptStderrPath,

    [Parameter(Mandatory = $true)]
    [int]$AttemptNumber
  )

  if ($AttemptNumber -eq 1) {
    return
  }
  $utf8 = [Text.UTF8Encoding]::new($false)
  try {
    if (Test-Path -LiteralPath $AttemptEventPath -PathType Leaf) {
      if (!(Test-Path -LiteralPath $CanonicalEventPath -PathType Leaf)) {
        [IO.File]::WriteAllText($CanonicalEventPath, "", $utf8)
      }
      $eventText = [IO.File]::ReadAllText($AttemptEventPath, $utf8)
      if ($eventText.Length -gt 0) {
        [IO.File]::AppendAllText($CanonicalEventPath, $eventText, $utf8)
        if (!$eventText.EndsWith("`n")) {
          [IO.File]::AppendAllText($CanonicalEventPath, "`r`n", $utf8)
        }
      }
    }
    if (Test-Path -LiteralPath $AttemptStderrPath -PathType Leaf) {
      $stderrText = [IO.File]::ReadAllText($AttemptStderrPath, $utf8)
      $boundary = "`r`n--- Sol-Luna retry attempt $AttemptNumber stderr ---`r`n"
      [IO.File]::AppendAllText($CanonicalStderrPath, $boundary + $stderrText, $utf8)
    }
  } finally {
    if ($utf8 -is [IDisposable]) {
      $utf8.Dispose()
    }
  }
}

function Invoke-CodexAttempt {
  param(
    [Parameter(Mandatory = $true)]
    [System.Management.Automation.CommandInfo]$Command,

    [Parameter(Mandatory = $true)]
    [string]$RunnerContent,

    [Parameter(Mandatory = $true)]
    [string[]]$Arguments,

    [Parameter(Mandatory = $true)]
    [string]$Prompt,

    [Parameter(Mandatory = $true)]
    [int]$AttemptNumber,

    [Parameter(Mandatory = $true)]
    [string]$AttemptKind,

    [Parameter(Mandatory = $true)]
    [string]$EventPath,

    [Parameter(Mandatory = $true)]
    [string]$StderrPath,

    [Parameter(Mandatory = $true)]
    [string]$ResultPath,

    [Parameter(Mandatory = $true)]
    [DateTime]$DeadlineUtc,

    [Parameter(Mandatory = $true)]
    [DateTime]$ExecutionStartedUtc,

    [Parameter(Mandatory = $true)]
    [int]$HeartbeatSeconds,

    [Parameter(Mandatory = $true)]
    [System.Collections.IDictionary]$State,

    [Parameter(Mandatory = $true)]
    [string]$StatePath,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedTaskType,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedEffort,

    [Parameter(Mandatory = $true)]
    [string]$SchemaPath
  )

  $temporaryDirectory = Split-Path -Parent $ResultPath
  $temporaryStem = ".sol-luna-{0}-{1}-{2}" -f $PID, $AttemptNumber, [Guid]::NewGuid().ToString('N')
  $promptPath = Join-Path $temporaryDirectory "$temporaryStem.prompt.txt"
  $runnerPath = Join-Path $temporaryDirectory "$temporaryStem.runner.ps1"
  $argumentsPath = Join-Path $temporaryDirectory "$temporaryStem.arguments.json"
  $exitCodePath = Join-Path $temporaryDirectory "$temporaryStem.exit-code.txt"
  $temporaryPaths = @($promptPath, $runnerPath, $argumentsPath, $exitCodePath)
  $startedUtc = [DateTime]::UtcNow
  $attemptRecord = [ordered]@{
    attempt = $AttemptNumber
    kind = $AttemptKind
    eventPath = $EventPath
    stderrPath = $StderrPath
    resultPath = $ResultPath
    sessionId = $null
    exitCode = $null
    startedUtc = $startedUtc.ToString("o", [Globalization.CultureInfo]::InvariantCulture)
    completedUtc = $null
    durationSeconds = $null
    status = "starting"
    error = $null
    usage = $null
    commandCount = 0
    failedCommandCount = 0
    eventBytes = 0
    stderrBytes = 0
    lastEventUtc = $null
  }
  $null = $State.attempts.Add($attemptRecord)
  $State.status = "running"
  $State.currentAttempt = $AttemptNumber
  Write-StateManifest -State $State -Path $StatePath

  $process = $null
  $exitCode = $null
  $result = $null
  $resultValid = $false
  $resultError = $null
  $attemptError = $null
  $timedOut = $false
  $sessionId = if ([string]::IsNullOrWhiteSpace($State.sessionId)) { $null } else { [string]$State.sessionId }
  $usage = $null
  $observedLength = [int64]0
  $latestEventUtc = [DateTime]::MinValue

  try {
    [IO.File]::WriteAllText($promptPath, $Prompt, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($runnerPath, $RunnerContent, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($argumentsPath, (ConvertTo-Json -InputObject @($Arguments) -Compress), [Text.UTF8Encoding]::new($false))
    $process = Start-CodexProcess -Command $Command -RunnerPath $runnerPath -ArgumentsPath $argumentsPath -ExitCodePath $exitCodePath -PromptPath $promptPath -StdoutPath $EventPath -StderrPath $StderrPath
    $nextHeartbeatUtc = [DateTime]::UtcNow.AddSeconds($HeartbeatSeconds)
    while (!$process.WaitForExit(250)) {
      $eventInfo = Get-AttemptEventMetadata -Path $EventPath -ObservedLength ([ref]$observedLength)
      if ($eventInfo.HasNewContent -and $null -ne $eventInfo.LatestEventUtc) {
        $latestEventUtc = $eventInfo.LatestEventUtc
      }
      if ($eventInfo.HasNewContent -and [string]::IsNullOrWhiteSpace($sessionId)) {
        $capturedThreadId = Get-ThreadIdFromEventHead -Path $EventPath
        if (![string]::IsNullOrWhiteSpace($capturedThreadId)) {
          $attemptRecord.sessionId = $capturedThreadId
          if ($State.sessionId -ne $capturedThreadId) {
            $State.sessionId = $capturedThreadId
            Write-StateManifest -State $State -Path $StatePath
          }
          $sessionId = $capturedThreadId
        }
      }
      if ([DateTime]::UtcNow -ge $DeadlineUtc) {
        Stop-ProcessTree -Process $process
        $timedOut = $true
        $attemptError = "timeout"
        break
      }
      if ([DateTime]::UtcNow -ge $nextHeartbeatUtc) {
        Write-Heartbeat -ExecutionStartedUtc $ExecutionStartedUtc -AttemptNumber $AttemptNumber -AttemptKind $AttemptKind -ProcessState "running" -SessionId $sessionId -LatestEventUtc $latestEventUtc
        do { $nextHeartbeatUtc = $nextHeartbeatUtc.AddSeconds($HeartbeatSeconds) } while ($nextHeartbeatUtc -le [DateTime]::UtcNow)
      }
    }
    if (!$timedOut) {
      $process.WaitForExit()
      $eventInfo = Get-AttemptEventMetadata -Path $EventPath -ObservedLength ([ref]$observedLength)
      if ($eventInfo.HasNewContent -and $null -ne $eventInfo.LatestEventUtc) {
        $latestEventUtc = $eventInfo.LatestEventUtc
      }
      if ([string]::IsNullOrWhiteSpace($sessionId)) {
        $capturedThreadId = Get-ThreadIdFromEventHead -Path $EventPath
        if (![string]::IsNullOrWhiteSpace($capturedThreadId)) {
          $attemptRecord.sessionId = $capturedThreadId
          if ($State.sessionId -ne $capturedThreadId) {
            $State.sessionId = $capturedThreadId
            Write-StateManifest -State $State -Path $StatePath
          }
          $sessionId = $capturedThreadId
        }
      }
    }
    if (!$timedOut) {
      if (Test-Path -LiteralPath $exitCodePath -PathType Leaf) {
        $exitCodeText = [IO.File]::ReadAllText($exitCodePath).Trim()
        $parsedExitCode = 0
        if ([int]::TryParse($exitCodeText, [ref]$parsedExitCode)) {
          $exitCode = $parsedExitCode
        } else {
          $attemptError = "invalid runner exit code"
        }
      } else {
        $attemptError = "runner did not create an exit-code file"
      }
      if ($null -eq $attemptError -and (Test-Path -LiteralPath $ResultPath -PathType Leaf)) {
        try {
          $result = Get-Content -LiteralPath $ResultPath -Raw -Encoding UTF8 | ConvertFrom-Json -ErrorAction Stop
          Assert-ExecutorResult -Result $result -ExpectedTaskType $ExpectedTaskType -ExpectedEffort $ExpectedEffort
          $resultValid = $true
        } catch {
          $resultError = $_.Exception.Message
        }
      } elseif ($null -eq $attemptError) {
        $resultError = "result file is missing"
      }
    }
  } catch {
    $attemptError = $_.Exception.Message
    if ($null -ne $process -and !$process.HasExited) {
      Stop-ProcessTree -Process $process
    }
  } finally {
    if ($null -ne $process) {
      try { $process.Dispose() } catch { }
    }
    foreach ($temporaryPath in $temporaryPaths) {
      if (Test-Path -LiteralPath $temporaryPath) {
        Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
      }
    }
  }

  $eventSummary = Get-CompletedEventSummary -Path $EventPath
  if (![string]::IsNullOrWhiteSpace($eventSummary.ThreadId)) {
    $sessionId = $eventSummary.ThreadId
    $State.sessionId = $eventSummary.ThreadId
  }
  $usage = $eventSummary.Usage
  if ($null -ne $eventSummary.LatestEventUtc) {
    $latestEventUtc = $eventSummary.LatestEventUtc
  }
  $stderrBytes = if (Test-Path -LiteralPath $StderrPath -PathType Leaf) {
    [int64](Get-Item -LiteralPath $StderrPath -Force).Length
  } else {
    [int64]0
  }
  $completedUtc = [DateTime]::UtcNow
  $attemptRecord.exitCode = $exitCode
  $attemptRecord.completedUtc = $completedUtc.ToString("o", [Globalization.CultureInfo]::InvariantCulture)
  $attemptRecord.durationSeconds = [math]::Round(($completedUtc - $startedUtc).TotalSeconds, 3)
  $attemptRecord.sessionId = $sessionId
  $attemptRecord.status = if ($timedOut) { "timed-out" } elseif ($null -ne $attemptError) { "failed" } elseif ($resultValid) { "completed" } else { "invalid-result" }
  $attemptRecord.error = if ($timedOut) { "timeout" } elseif ($null -ne $attemptError) { "process-failure" } elseif (!$resultValid) { "invalid-result" } else { $null }
  $attemptRecord.usage = $usage
  $attemptRecord.commandCount = $eventSummary.CommandCount
  $attemptRecord.failedCommandCount = $eventSummary.FailedCommandCount
  $attemptRecord.eventBytes = $eventSummary.EventBytes
  $attemptRecord.stderrBytes = $stderrBytes
  $attemptRecord.lastEventUtc = if ($null -eq $eventSummary.LatestEventUtc) { $null } else { $eventSummary.LatestEventUtc.ToString("o", [Globalization.CultureInfo]::InvariantCulture) }
  if ($null -ne $State.sessionId -and [string]::IsNullOrWhiteSpace($attemptRecord.sessionId)) {
    $attemptRecord.sessionId = $State.sessionId
  }
  Write-StateManifest -State $State -Path $StatePath

  return [PSCustomObject]@{
    Attempt = $AttemptNumber
    Kind = $AttemptKind
    EventPath = $EventPath
    StderrPath = $StderrPath
    ResultPath = $ResultPath
    ExitCode = $exitCode
    TimedOut = $timedOut
    Result = $result
    ResultValid = $resultValid
    ResultError = $resultError
    AttemptError = $attemptError
    SessionId = $sessionId
    Usage = $usage
    DurationSeconds = $attemptRecord.durationSeconds
    CommandCount = $attemptRecord.commandCount
    FailedCommandCount = $attemptRecord.failedCommandCount
    EventBytes = $attemptRecord.eventBytes
    StderrBytes = $attemptRecord.stderrBytes
    LastEventUtc = $attemptRecord.lastEventUtc
  }
}

if ($ReasoningEffort -ne "xhigh") {
  if ([string]::IsNullOrWhiteSpace($DowngradeReason) -or $DowngradeReason -eq "none") {
    throw "DowngradeReason is required when ReasoningEffort is lower than xhigh."
  }
} elseif ($DowngradeReason -ne "none") {
  throw "DowngradeReason must be 'none' when ReasoningEffort is xhigh."
}

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
  $RepoRoot = Join-Path $PSScriptRoot "..\..\..\.."
}

$resolvedRepoRoot = Resolve-ExistingPath -Path $RepoRoot -Label "Repository root"
$resolvedTaskPath = Resolve-ExistingPath -Path $TaskPath -Label "Task packet"
$schemaPath = Resolve-ExistingPath -Path (Join-Path $PSScriptRoot "..\references\executor-result.schema.json") -Label "Executor result schema"
if (!(Test-Path -LiteralPath (Join-Path $resolvedRepoRoot ".git"))) {
  throw "Repository root is not a Git worktree: $resolvedRepoRoot"
}

$null = Get-Command git -ErrorAction Stop
if ($env:OS -eq "Windows_NT" -and $CodexCommand -eq "codex") {
  $codexCommandInfo = Get-Command "codex.cmd" -ErrorAction Stop
} else {
  $codexCommandInfo = Get-Command $CodexCommand -ErrorAction Stop
}
$resultFullPath = Resolve-NewExternalFilePath -Path $ResultPath -Root $resolvedRepoRoot -Label "Result path"
$eventCandidate = if ([string]::IsNullOrWhiteSpace($EventLogPath)) {
  [IO.Path]::ChangeExtension($resultFullPath, ".events.jsonl")
} else {
  $EventLogPath
}
$stderrCandidate = if ([string]::IsNullOrWhiteSpace($StderrLogPath)) {
  [IO.Path]::ChangeExtension($resultFullPath, ".stderr.log")
} else {
  $StderrLogPath
}
$eventLogFullPath = Resolve-NewExternalFilePath -Path $eventCandidate -Root $resolvedRepoRoot -Label "Event log path"
$stderrLogFullPath = Resolve-NewExternalFilePath -Path $stderrCandidate -Root $resolvedRepoRoot -Label "Stderr log path"
$stateCandidate = [IO.Path]::ChangeExtension($resultFullPath, ".state.json")
$stateFullPath = Resolve-NewExternalFilePath -Path $stateCandidate -Root $resolvedRepoRoot -Label "State manifest path"

$outputPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($path in @($resultFullPath, $eventLogFullPath, $stderrLogFullPath, $stateFullPath)) {
  if (!$outputPaths.Add($path)) {
    throw "Result, event, and stderr paths must be distinct."
  }
}

$taskContent = Get-Content -LiteralPath $resolvedTaskPath -Raw -Encoding UTF8
foreach ($section in $requiredSections) {
  $null = Get-TaskSection -Content $taskContent -Section $section
}

$packetTaskType = Get-TaskSection -Content $taskContent -Section "Task Type"
$packetTaskType = $packetTaskType.Trim('`', ' ', "`r", "`n")
if ($packetTaskType -ne $TaskType) {
  throw "Task type mismatch. Packet: '$packetTaskType'; invocation: '$TaskType'."
}
$allowedChangedPaths = Get-AllowedChangedPaths -Content $taskContent -Root $resolvedRepoRoot -Type $TaskType
if (!$ValidateOnly) {
  Assert-ExternalTaskPath -Path $resolvedTaskPath -Root $resolvedRepoRoot
}
$sandbox = if ($TaskType -eq "read-only-analysis") { "read-only" } else { "workspace-write" }
if ($ValidateOnly) {
  Write-Host "Task packet is valid."
  Write-Host "Model: $model"
  Write-Host "Reasoning effort: $ReasoningEffort"
  Write-Host "Sandbox: $sandbox"
  Write-Host "Timeout seconds: $TimeoutSeconds"
  Write-Host "Heartbeat seconds: $HeartbeatSeconds"
  Write-Host "Max resume attempts: $MaxResumeAttempts"
  Write-Host "Retry delay seconds: $RetryDelaySeconds"
  Write-Host "Repository: $resolvedRepoRoot"
  Write-Host "Result: $resultFullPath"
  Write-Host "Event log: $eventLogFullPath"
  Write-Host "Stderr log: $stderrLogFullPath"
  Write-Host "State manifest: $stateFullPath"
  exit 0
}

$writerMutex = $null
$ownsWriterMutex = $false
$executionErrors = [Collections.Generic.List[string]]::new()
$finalHead = "unavailable"
$actualChangedPaths = @()
$result = $null
$usage = $null
$state = $null

try {
  if ($TaskType -eq "implementation") {
    $writerMutex = Acquire-WriterMutex -Root $resolvedRepoRoot
    $ownsWriterMutex = $true
  }

  $baselineHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read baseline HEAD"
  $baselineSnapshot = Get-WorkspaceSnapshot -Root $resolvedRepoRoot
  $executionStartedUtc = [DateTime]::UtcNow
  $deadlineUtc = $executionStartedUtc.AddSeconds($TimeoutSeconds)
  $state = [ordered]@{
    version = 2
    status = "starting"
    taskType = $TaskType
    model = $model
    effort = $ReasoningEffort
    repository = $resolvedRepoRoot
    baselineHead = $baselineHead
    sessionId = $null
    currentAttempt = 0
    maxRetries = $MaxResumeAttempts
    canonicalOutputPaths = [ordered]@{
      result = $resultFullPath
      event = $eventLogFullPath
      stderr = $stderrLogFullPath
    }
    attempts = [Collections.ArrayList]::new()
    usage = $null
    durationSeconds = 0
    commandCount = 0
    failedCommandCount = 0
    eventBytes = 0
    stderrBytes = 0
    lastEventUtc = $null
    lastError = $null
    updatedUtc = $null
  }
  Write-StateManifest -State $state -Path $stateFullPath

  $prompt = @"
You are the bounded Luna execution agent in a Sol-led workflow.

Execution contract:
- Model requested by the host: $model
- Reasoning effort requested by the host: $ReasoningEffort
- Downgrade reason: $DowngradeReason
- Task type: $TaskType
- Git baseline HEAD: $baselineHead

Read and follow the applicable repository instructions already provided by Codex. Treat the task packet as frozen. Do not recursively scan unrelated worktrees for instruction files. Do not add requirements, redesign confirmed decisions, broaden scope, commit, amend, reset, restore, rebase, package, publish, or start a product UI unless the packet explicitly authorizes that exact action. Preserve unrelated dirty-worktree changes.

You are already the Luna executor. Do not invoke the Sol-Luna wrapper, start another Codex agent, or delegate this packet again.

Use exact target paths and focused ranges. Batch independent read-only commands when practical, limit individual tool output, and do not repeatedly read complete large files. Do not broadly scan dependency repositories. Run targeted validation before broader checks, and run a full build at most once at the end when the packet requires it.

If required information is missing, instructions conflict, the worktree changes unexpectedly, or the task reaches a new product or architecture decision, stop and return status "blocked" with evidence. Never report success when a required validation failed or was not run.

Execute the packet's frozen `Executable Design` as written. Make only explicitly delegated mechanical choices; do not select an approach, fill design gaps, or redesign confirmed decisions. If the repository facts do not match the design, or implementation would require changing or completing it, return status "blocked" with evidence.

Do not emit progress or interim agent messages. Use tools as needed, then emit exactly one final JSON object that matches the provided output schema. In commands, retain changed-state, validation, and failure-relevant commands; omit routine successful exploration unless it supports a finding. Echo taskType and reasoningEffort exactly as supplied by the host.

<task_packet>
$taskContent
</task_packet>
"@

  $runnerContent = @(
    'param(',
    '  [Parameter(Mandatory = $true)]',
    '  [string]$CommandPath,',
    '',
    '  [Parameter(Mandatory = $true)]',
    '  [string]$ArgumentsPath,',
    '',
    '  [Parameter(Mandatory = $true)]',
    '  [string]$ExitCodePath',
    ')',
    '',
    '$ErrorActionPreference = "Stop"',
    '$commandExitCode = 1',
    'try {',
    '  $parsedArguments = Get-Content -LiteralPath $ArgumentsPath -Raw -Encoding UTF8 | ConvertFrom-Json',
    '  $commandArguments = @($parsedArguments)',
    '  $command = Get-Command $CommandPath -ErrorAction Stop',
    '  $global:LASTEXITCODE = $null',
    '  if ($command.CommandType -eq [Management.Automation.CommandTypes]::ExternalScript) {',
    '    & $command.Source @commandArguments',
    '  } elseif ($command.CommandType -eq [Management.Automation.CommandTypes]::Application) {',
    '    & $command.Source @commandArguments',
    '  } else {',
    '    throw "Command must resolve to an application or external script: $($command.CommandType)"',
    '  }',
    '  $commandExitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }',
    '} catch {',
    '  [Console]::Error.WriteLine($_.Exception.Message)',
    '  $commandExitCode = 1',
    '} finally {',
    '  [IO.File]::WriteAllText($ExitCodePath, $commandExitCode.ToString([Globalization.CultureInfo]::InvariantCulture), [Text.UTF8Encoding]::new($false))',
    '}',
    'exit $commandExitCode'
  ) -join "`n"

  $attemptNumber = 1
  while ($true) {
    $hasSession = ![string]::IsNullOrWhiteSpace([string]$state.sessionId)
    $attemptKind = if ($attemptNumber -eq 1) { "initial" } elseif ($hasSession) { "resume" } else { "fresh-initial" }
    $attemptResultPath = Get-AttemptPath -CanonicalPath $resultFullPath -AttemptNumber $attemptNumber
    $attemptEventPath = Get-AttemptPath -CanonicalPath $eventLogFullPath -AttemptNumber $attemptNumber
    $attemptStderrPath = Get-AttemptPath -CanonicalPath $stderrLogFullPath -AttemptNumber $attemptNumber
    if ($attemptNumber -gt 1) {
      $attemptResultPath = Resolve-NewExternalFilePath -Path $attemptResultPath -Root $resolvedRepoRoot -Label "Attempt $attemptNumber result path"
      $attemptEventPath = Resolve-NewExternalFilePath -Path $attemptEventPath -Root $resolvedRepoRoot -Label "Attempt $attemptNumber event log path"
      $attemptStderrPath = Resolve-NewExternalFilePath -Path $attemptStderrPath -Root $resolvedRepoRoot -Label "Attempt $attemptNumber stderr log path"
    }

    $attemptArguments = if ($attemptNumber -eq 1 -or !$hasSession) {
      @(
        "exec",
        "-C", $resolvedRepoRoot,
        "-m", $model,
        "-c", "model_reasoning_effort=`"$ReasoningEffort`"",
        "-s", $sandbox,
        "--output-schema", $schemaPath,
        "--json",
        "-o", $attemptResultPath,
        "-"
      )
    } else {
      @(
        "exec",
        "resume", $state.sessionId,
        "-m", $model,
        "-c", "model_reasoning_effort=`"$ReasoningEffort`"",
        "--output-schema", $schemaPath,
        "--json",
        "-o", $attemptResultPath,
        "-"
      )
    }
    if ($attemptNumber -gt 1 -and $hasSession) {
      $attemptPrompt = "Continue the frozen task from the previous attempt. Preserve all repository changes already made and complete the task packet. Return exactly one final JSON result."
    } else {
      $attemptPrompt = $prompt
    }

    $outcome = Invoke-CodexAttempt `
      -Command $codexCommandInfo `
      -RunnerContent $runnerContent `
      -Arguments $attemptArguments `
      -Prompt $attemptPrompt `
      -AttemptNumber $attemptNumber `
      -AttemptKind $attemptKind `
      -EventPath $attemptEventPath `
      -StderrPath $attemptStderrPath `
      -ResultPath $attemptResultPath `
      -DeadlineUtc $deadlineUtc `
      -ExecutionStartedUtc $executionStartedUtc `
      -HeartbeatSeconds $HeartbeatSeconds `
      -State $state `
      -StatePath $stateFullPath `
      -ExpectedTaskType $TaskType `
      -ExpectedEffort $ReasoningEffort `
      -SchemaPath $schemaPath

    Merge-AttemptLogs `
      -CanonicalEventPath $eventLogFullPath `
      -CanonicalStderrPath $stderrLogFullPath `
      -AttemptEventPath $attemptEventPath `
      -AttemptStderrPath $attemptStderrPath `
      -AttemptNumber $attemptNumber

    $usage = Add-UsageRecords -Left $usage -Right $outcome.Usage
    $state.usage = $usage
    $state.commandCount = [int]$state.commandCount + [int]$outcome.CommandCount
    $state.failedCommandCount = [int]$state.failedCommandCount + [int]$outcome.FailedCommandCount
    $state.eventBytes = [int64]$state.eventBytes + [int64]$outcome.EventBytes
    $state.stderrBytes = [int64]$state.stderrBytes + [int64]$outcome.StderrBytes
    if (![string]::IsNullOrWhiteSpace($outcome.LastEventUtc)) {
      $state.lastEventUtc = $outcome.LastEventUtc
    }
    Write-StateManifest -State $state -Path $stateFullPath

    if ($outcome.ResultValid) {
      $result = $outcome.Result
      if ($outcome.ExitCode -ne 0) {
        $executionErrors.Add("Luna executor returned a valid result with exit code $($outcome.ExitCode).")
      }
      break
    }

    if ($outcome.TimedOut) {
      $executionErrors.Add("Luna executor timed out after $TimeoutSeconds seconds.")
      $state.status = "timed-out"
      $state.lastError = "Attempt $attemptNumber reached the hard timeout."
      break
    }

    $invalidResultDetail = if ($null -ne $outcome.ResultError) { $outcome.ResultError } else { "no valid structured result" }
    if ($null -eq $outcome.ExitCode -or $outcome.ExitCode -eq 0) {
      $executionErrors.Add("Luna executor result is not valid JSON or did not complete: $invalidResultDetail")
      $state.status = "failed"
      $state.lastError = "Attempt $attemptNumber ended without a valid structured result and was not retryable."
      break
    }

    $state.lastError = "Attempt $attemptNumber ended with a nonzero exit without a valid structured result."
    if ($attemptNumber -gt $MaxResumeAttempts) {
      $executionErrors.Add("Luna executor failed with exit code $($outcome.ExitCode) and exhausted retry attempts.")
      $state.status = "failed"
      break
    }

    try {
      $null = Assert-RetrySafety `
        -Root $resolvedRepoRoot `
        -BaselineHead $baselineHead `
        -BaselineSnapshot $baselineSnapshot `
        -Type $TaskType `
        -AllowedChangedPaths $allowedChangedPaths `
        -SessionAvailable (![string]::IsNullOrWhiteSpace([string]$state.sessionId))
    } catch {
      $executionErrors.Add($_.Exception.Message)
      $state.status = "blocked"
      $state.lastError = "Retry safety checks blocked continuation after attempt $attemptNumber."
      break
    }

    if ([DateTime]::UtcNow -ge $deadlineUtc) {
      $executionErrors.Add("Luna executor reached the hard deadline before retry attempt $($attemptNumber + 1).")
      $state.status = "timed-out"
      break
    }
    $remainingSeconds = ($deadlineUtc - [DateTime]::UtcNow).TotalSeconds
    if ($RetryDelaySeconds -gt $remainingSeconds) {
      $executionErrors.Add("Luna executor cannot honor the retry delay before the hard deadline.")
      $state.status = "timed-out"
      break
    }

    $state.status = "retry-waiting"
    $state.currentAttempt = $attemptNumber
    Write-StateManifest -State $state -Path $stateFullPath
    if ($RetryDelaySeconds -gt 0) {
      Start-Sleep -Seconds $RetryDelaySeconds
    }
    if ([DateTime]::UtcNow -ge $deadlineUtc) {
      $executionErrors.Add("Luna executor reached the hard deadline during retry delay.")
      $state.status = "timed-out"
      break
    }
    $attemptNumber++
  }

  try {
    $finalHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read final HEAD"
    $finalSnapshot = Get-WorkspaceSnapshot -Root $resolvedRepoRoot
    $actualChangedPaths = Compare-WorkspaceSnapshots -Before $baselineSnapshot -After $finalSnapshot
  } catch {
    $executionErrors.Add("Capture final Git state failed: $($_.Exception.Message)")
  }

  if ($finalHead -ne "unavailable" -and $finalHead -ne $baselineHead) {
    $executionErrors.Add("Luna executor created or changed a commit. Baseline: $baselineHead; final: $finalHead.")
  }
  if ($TaskType -eq "read-only-analysis" -and $actualChangedPaths.Count -gt 0) {
    $executionErrors.Add("Read-only Luna task changed Git-visible paths: $($actualChangedPaths -join ', ')")
  }
  if ($TaskType -eq "implementation") {
    $outsideScope = @($actualChangedPaths | Where-Object { !$allowedChangedPaths.Contains($_) })
    if ($outsideScope.Count -gt 0) {
      $executionErrors.Add("Implementation changed paths outside Allowed Changed Paths: $($outsideScope -join ', ')")
    }
  }

  if ($null -ne $result) {
    $reportedPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    try {
      foreach ($path in @($result.changedFiles)) {
        $normalized = ConvertTo-RepositoryPath -Path $path -Root $resolvedRepoRoot -Label "Executor changedFiles path"
        if (!$reportedPaths.Add($normalized)) {
          throw "Executor changedFiles contains a duplicate: $normalized"
        }
      }
      $missingReports = @($actualChangedPaths | Where-Object { !$reportedPaths.Contains($_) })
      $falseReports = @($reportedPaths | Where-Object { $actualChangedPaths -notcontains $_ })
      if ($missingReports.Count -gt 0 -or $falseReports.Count -gt 0) {
        $executionErrors.Add("Executor changedFiles does not match actual changes. Missing: $($missingReports -join ', '); unexpected: $($falseReports -join ', ')")
      }
    } catch {
      $executionErrors.Add($_.Exception.Message)
    }

    if (@($result.validation | Where-Object { $_.status -eq "failed" }).Count -gt 0) {
      $executionErrors.Add("Luna executor reported failed validation.")
    }
    if (@($result.scopeDeviations).Count -gt 0) {
      $executionErrors.Add("Luna executor reported scope deviations.")
    }
    if ($result.status -ne "success") {
      $executionErrors.Add("Luna executor returned status '$($result.status)'.")
    }
  }

  if ($executionErrors.Count -eq 0 -and $attemptNumber -gt 1) {
    try {
      [IO.File]::Copy($attemptResultPath, $resultFullPath, $true)
    } catch {
      $executionErrors.Add("Copy successful retry result to the canonical path failed: $($_.Exception.Message)")
    }
  }

  if ($executionErrors.Count -eq 0) {
    $state.status = "succeeded"
    $state.lastError = $null
  } elseif ($state.status -notin @("timed-out", "blocked")) {
    $state.status = "failed"
    $state.lastError = $executionErrors[0]
  } elseif ([string]::IsNullOrWhiteSpace([string]$state.lastError)) {
    $state.lastError = $executionErrors[0]
  }
  $state.durationSeconds = [math]::Round(([DateTime]::UtcNow - $executionStartedUtc).TotalSeconds, 3)
  $state.eventBytes = if (Test-Path -LiteralPath $eventLogFullPath -PathType Leaf) { [int64](Get-Item -LiteralPath $eventLogFullPath -Force).Length } else { [int64]0 }
  $state.stderrBytes = if (Test-Path -LiteralPath $stderrLogFullPath -PathType Leaf) { [int64](Get-Item -LiteralPath $stderrLogFullPath -Force).Length } else { [int64]0 }
  Write-StateManifest -State $state -Path $stateFullPath

  $actualSummary = if ($actualChangedPaths.Count -eq 0) { "<none>" } else { $actualChangedPaths -join ", " }
  if ($executionErrors.Count -gt 0) {
    $details = $executionErrors -join " | "
    throw "$details Final HEAD: $finalHead. Actual changed paths: $actualSummary. Event log: $eventLogFullPath. Stderr log: $stderrLogFullPath. State manifest: $stateFullPath."
  }

  $compactSummary = [ordered]@{
    status = $result.status
    taskType = $TaskType
    model = $model
    effort = $ReasoningEffort
    attemptCount = $state.attempts.Count
    durationSeconds = $state.durationSeconds
    usage = $state.usage
    commandCount = $state.commandCount
    failedCommandCount = $state.failedCommandCount
    actualChangedPathCount = $actualChangedPaths.Count
    paths = [ordered]@{
      result = $resultFullPath
      eventLog = $eventLogFullPath
      stderrLog = $stderrLogFullPath
      state = $stateFullPath
    }
  }
  $compactJson = ConvertTo-Json -InputObject $compactSummary -Depth 5 -Compress
  if ([Text.Encoding]::UTF8.GetByteCount($compactJson) -ge 4096) {
    throw "Compact success summary exceeded 4096 bytes. Result: $resultFullPath. Event log: $eventLogFullPath. Stderr log: $stderrLogFullPath. State manifest: $stateFullPath."
  }
  Write-Output $compactJson
} finally {
  if ($ownsWriterMutex -and $null -ne $writerMutex) {
    try { $writerMutex.ReleaseMutex() } catch { }
  }
  if ($null -ne $writerMutex) {
    $writerMutex.Dispose()
  }
}
