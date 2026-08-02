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
  return $paths
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

$outputPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($path in @($resultFullPath, $eventLogFullPath, $stderrLogFullPath)) {
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
  Write-Host "Repository: $resolvedRepoRoot"
  Write-Host "Result: $resultFullPath"
  Write-Host "Event log: $eventLogFullPath"
  Write-Host "Stderr log: $stderrLogFullPath"
  exit 0
}

$writerMutex = $null
$ownsWriterMutex = $false
if ($TaskType -eq "implementation") {
  $writerMutex = Acquire-WriterMutex -Root $resolvedRepoRoot
  $ownsWriterMutex = $true
}

$baselineHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read baseline HEAD"
$baselineSnapshot = Get-WorkspaceSnapshot -Root $resolvedRepoRoot
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

If required information is missing, instructions conflict, the worktree changes unexpectedly, or the task reaches a new product or architecture decision, stop and return status "blocked" with evidence. Never report success when a required validation failed or was not run.

Do not emit progress or interim agent messages. Use tools as needed, then emit exactly one final JSON object that matches the provided output schema. Echo taskType and reasoningEffort exactly as supplied by the host.

<task_packet>
$taskContent
</task_packet>
"@

$arguments = @(
  "exec",
  "-C", $resolvedRepoRoot,
  "-m", $model,
  "-c", "model_reasoning_effort=`"$ReasoningEffort`"",
  "-s", $sandbox,
  "--ephemeral",
  "--output-schema", $schemaPath,
  "--json",
  "-o", $resultFullPath,
  "-"
)

$temporaryStem = ".sol-luna-{0}-{1}" -f $PID, [Guid]::NewGuid().ToString('N')
$temporaryDirectory = Split-Path -Parent $resultFullPath
$promptPath = Join-Path $temporaryDirectory "$temporaryStem.prompt.txt"
$runnerPath = Join-Path $temporaryDirectory "$temporaryStem.runner.ps1"
$argumentsPath = Join-Path $temporaryDirectory "$temporaryStem.arguments.json"
$exitCodePath = Join-Path $temporaryDirectory "$temporaryStem.exit-code.txt"
$temporaryPaths = @($promptPath, $runnerPath, $argumentsPath, $exitCodePath)
$runnerContent = @'
param(
  [Parameter(Mandatory = $true)]
  [string]$CommandPath,

  [Parameter(Mandatory = $true)]
  [string]$ArgumentsPath,

  [Parameter(Mandatory = $true)]
  [string]$ExitCodePath
)

$ErrorActionPreference = "Stop"
$commandExitCode = 1
try {
  $parsedArguments = Get-Content -LiteralPath $ArgumentsPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $commandArguments = @($parsedArguments)
  $command = Get-Command $CommandPath -ErrorAction Stop
  $global:LASTEXITCODE = $null
  if ($command.CommandType -eq [Management.Automation.CommandTypes]::ExternalScript) {
    & $command.Source @commandArguments
  } elseif ($command.CommandType -eq [Management.Automation.CommandTypes]::Application) {
    & $command.Source @commandArguments
  } else {
    throw "Command must resolve to an application or external script: $($command.CommandType)"
  }
  $commandExitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
} catch {
  [Console]::Error.WriteLine($_.Exception.Message)
  $commandExitCode = 1
} finally {
  [IO.File]::WriteAllText($ExitCodePath, $commandExitCode.ToString([Globalization.CultureInfo]::InvariantCulture), [Text.UTF8Encoding]::new($false))
}
exit $commandExitCode
'@
$process = $null
$result = $null
$executionErrors = [Collections.Generic.List[string]]::new()
$finalHead = "unavailable"
$actualChangedPaths = @()
$usage = $null

try {
  [IO.File]::WriteAllText($promptPath, $prompt, [Text.UTF8Encoding]::new($false))
  [IO.File]::WriteAllText($runnerPath, $runnerContent, [Text.UTF8Encoding]::new($false))
  [IO.File]::WriteAllText($argumentsPath, (ConvertTo-Json -InputObject @($arguments) -Compress), [Text.UTF8Encoding]::new($false))
  $process = Start-CodexProcess -Command $codexCommandInfo -RunnerPath $runnerPath -ArgumentsPath $argumentsPath -ExitCodePath $exitCodePath -PromptPath $promptPath -StdoutPath $eventLogFullPath -StderrPath $stderrLogFullPath
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while (!$process.WaitForExit(250)) {
    if ([DateTime]::UtcNow -ge $deadline) {
      Stop-ProcessTree -Process $process
      throw "Luna executor timed out after $TimeoutSeconds seconds."
    }
  }
  $process.WaitForExit()

  if (!(Test-Path -LiteralPath $exitCodePath -PathType Leaf)) {
    throw "Luna executor runner did not create the exit-code file."
  }
  $exitCodeText = [IO.File]::ReadAllText($exitCodePath).Trim()
  $codexExitCode = 0
  if (![int]::TryParse($exitCodeText, [ref]$codexExitCode)) {
    throw "Luna executor runner wrote an invalid exit code: '$exitCodeText'."
  }
  if ($codexExitCode -ne 0) {
    throw "Luna executor failed with exit code $codexExitCode."
  }
  if (!(Test-Path -LiteralPath $resultFullPath -PathType Leaf)) {
    throw "Luna executor did not create the result file."
  }

  try {
    $result = Get-Content -LiteralPath $resultFullPath -Raw -Encoding UTF8 | ConvertFrom-Json -ErrorAction Stop
  } catch {
    throw "Luna executor result is not valid JSON: $($_.Exception.Message)"
  }
  Assert-ExecutorResult -Result $result -ExpectedTaskType $TaskType -ExpectedEffort $ReasoningEffort
} catch {
  $executionErrors.Add($_.Exception.Message)
} finally {
  foreach ($temporaryPath in $temporaryPaths) {
    if (Test-Path -LiteralPath $temporaryPath) {
      Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
    }
  }
  try {
    $finalHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read final HEAD"
    $finalSnapshot = Get-WorkspaceSnapshot -Root $resolvedRepoRoot
    $actualChangedPaths = Compare-WorkspaceSnapshots -Before $baselineSnapshot -After $finalSnapshot
  } catch {
    $executionErrors.Add("Capture final Git state failed: $($_.Exception.Message)")
  }
  if ($ownsWriterMutex -and $null -ne $writerMutex) {
    try { $writerMutex.ReleaseMutex() } catch { $executionErrors.Add("Release writer mutex failed: $($_.Exception.Message)") }
  }
  if ($null -ne $writerMutex) {
    $writerMutex.Dispose()
  }
  if ($null -ne $process) {
    $process.Dispose()
  }
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

if (Test-Path -LiteralPath $eventLogFullPath -PathType Leaf) {
  foreach ($eventLine in Get-Content -LiteralPath $eventLogFullPath -Encoding UTF8) {
    try {
      $event = $eventLine | ConvertFrom-Json -ErrorAction Stop
      if ($event.type -eq "turn.completed" -and $null -ne $event.usage) {
        $usage = $event.usage
      }
    } catch {
      # Raw event logs are retained even when a diagnostic line is not JSON.
    }
  }
}

$actualSummary = if ($actualChangedPaths.Count -eq 0) { "<none>" } else { $actualChangedPaths -join ", " }
if ($executionErrors.Count -gt 0) {
  $details = $executionErrors -join " | "
  throw "$details Final HEAD: $finalHead. Actual changed paths: $actualSummary. Event log: $eventLogFullPath. Stderr log: $stderrLogFullPath."
}

if (Test-Path -LiteralPath $eventLogFullPath -PathType Leaf) {
  Get-Content -LiteralPath $eventLogFullPath -Encoding UTF8 | Write-Output
}
Write-Host "Luna executor completed with status '$($result.status)'."
Write-Host "Result: $resultFullPath"
Write-Host "Event log: $eventLogFullPath"
Write-Host "Stderr log: $stderrLogFullPath"
Write-Host "Actual changed paths: $actualSummary"
if ($null -ne $usage) {
  Write-Host "Usage: input=$($usage.input_tokens), cached-input=$($usage.cached_input_tokens), output=$($usage.output_tokens), reasoning-output=$($usage.reasoning_output_tokens)"
}
