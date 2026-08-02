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
  "Forbidden Scope",
  "Steps",
  "Acceptance Criteria",
  "Validation",
  "Stop Conditions",
  "Output Requirements"
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

function Assert-OutsideRepository {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string]$Label
  )

  $rootPrefix = $Root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
  if ($Path.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "$Label must be outside the repository: $Path"
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
$null = Get-Command codex -ErrorAction Stop

$resultFullPath = [IO.Path]::GetFullPath($ResultPath)
Assert-OutsideRepository -Path $resultFullPath -Root $resolvedRepoRoot -Label "Result path"
$resultDirectory = Split-Path -Parent $resultFullPath
if (!(Test-Path -LiteralPath $resultDirectory -PathType Container)) {
  throw "Result directory does not exist: $resultDirectory"
}
if (Test-Path -LiteralPath $resultFullPath) {
  throw "Result path already exists: $resultFullPath"
}

if ([string]::IsNullOrWhiteSpace($EventLogPath)) {
  $eventLogFullPath = [IO.Path]::ChangeExtension($resultFullPath, ".events.jsonl")
} else {
  $eventLogFullPath = [IO.Path]::GetFullPath($EventLogPath)
}
Assert-OutsideRepository -Path $eventLogFullPath -Root $resolvedRepoRoot -Label "Event log path"
if ($eventLogFullPath -eq $resultFullPath) {
  throw "Event log path must differ from result path."
}
$eventLogDirectory = Split-Path -Parent $eventLogFullPath
if (!(Test-Path -LiteralPath $eventLogDirectory -PathType Container)) {
  throw "Event log directory does not exist: $eventLogDirectory"
}
if (Test-Path -LiteralPath $eventLogFullPath) {
  throw "Event log path already exists: $eventLogFullPath"
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

$baselineHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read baseline HEAD"
$baselineStatus = Get-GitValue -Root $resolvedRepoRoot -Arguments @("status", "--porcelain=v1") -Action "Read baseline status"
$sandbox = if ($TaskType -eq "read-only-analysis") { "read-only" } else { "workspace-write" }

$prompt = @"
You are the bounded Luna execution agent in a Sol-led workflow.

Execution contract:
- Model requested by the host: $model
- Reasoning effort requested by the host: $ReasoningEffort
- Downgrade reason: $DowngradeReason
- Task type: $TaskType
- Git baseline HEAD: $baselineHead
- Git baseline status follows:
$baselineStatus

Read and follow every applicable AGENTS.md before acting. Treat the task packet as frozen. Do not add requirements, redesign confirmed decisions, broaden scope, commit, amend, reset, restore, rebase, package, publish, or start a product UI unless the packet explicitly authorizes that exact action. Preserve unrelated dirty-worktree changes.

You are already the Luna executor. Do not invoke the Sol-Luna wrapper, start another Codex agent, or delegate this packet again.

If required information is missing, instructions conflict, the worktree changes unexpectedly, or the task reaches a new product or architecture decision, stop and return status "blocked" with evidence. Never report success when a required validation failed or was not run.

Return only JSON that matches the provided output schema. Echo taskType and reasoningEffort exactly as supplied by the host.

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
  "--output-schema", $schemaPath,
  "--json",
  "-o", $resultFullPath,
  "-"
)

if ($ValidateOnly) {
  Write-Host "Task packet is valid."
  Write-Host "Model: $model"
  Write-Host "Reasoning effort: $ReasoningEffort"
  Write-Host "Sandbox: $sandbox"
  Write-Host "Repository: $resolvedRepoRoot"
  Write-Host "Result: $resultFullPath"
  Write-Host "Event log: $eventLogFullPath"
  exit 0
}

$events = @($prompt | & codex @arguments)
$codexExitCode = $LASTEXITCODE
$events | Set-Content -LiteralPath $eventLogFullPath -Encoding UTF8
$events | Write-Output
$usage = $null
foreach ($eventLine in $events) {
  try {
    $event = $eventLine | ConvertFrom-Json
    if ($event.type -eq "turn.completed" -and $null -ne $event.usage) {
      $usage = $event.usage
    }
  } catch {
    # Preserve the original event log; non-JSON diagnostics are not usage events.
  }
}

if ($codexExitCode -ne 0) {
  throw "Luna executor failed with exit code $codexExitCode. Event log: $eventLogFullPath"
}
if (!(Test-Path -LiteralPath $resultFullPath -PathType Leaf)) {
  throw "Luna executor did not create the result file: $resultFullPath"
}

$result = Get-Content -LiteralPath $resultFullPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($result.taskType -ne $TaskType) {
  throw "Executor result taskType mismatch. Expected '$TaskType', got '$($result.taskType)'."
}
if ($result.reasoningEffort -ne $ReasoningEffort) {
  throw "Executor result reasoningEffort mismatch. Expected '$ReasoningEffort', got '$($result.reasoningEffort)'."
}

$finalHead = Get-GitValue -Root $resolvedRepoRoot -Arguments @("rev-parse", "HEAD") -Action "Read final HEAD"
$finalStatus = Get-GitValue -Root $resolvedRepoRoot -Arguments @("status", "--porcelain=v1") -Action "Read final status"

if ($finalHead -ne $baselineHead) {
  throw "Luna executor created or changed a commit. Baseline: $baselineHead; final: $finalHead."
}
if ($TaskType -eq "read-only-analysis" -and $finalStatus -ne $baselineStatus) {
  throw "Read-only Luna task changed the Git worktree. Review the worktree before continuing."
}
if ($TaskType -eq "read-only-analysis" -and @($result.changedFiles).Count -gt 0) {
  throw "Read-only Luna result reported changed files."
}
if (@($result.validation | Where-Object { $_.status -eq "failed" }).Count -gt 0) {
  throw "Luna executor reported failed validation. Result: $resultFullPath"
}
if (@($result.scopeDeviations).Count -gt 0) {
  throw "Luna executor reported scope deviations. Result: $resultFullPath"
}
if ($result.status -ne "success") {
  throw "Luna executor returned status '$($result.status)'. Result: $resultFullPath"
}

Write-Host "Luna executor completed with status '$($result.status)'."
Write-Host "Result: $resultFullPath"
Write-Host "Event log: $eventLogFullPath"
if ($null -ne $usage) {
  Write-Host "Usage: input=$($usage.input_tokens), cached-input=$($usage.cached_input_tokens), output=$($usage.output_tokens), reasoning-output=$($usage.reasoning_output_tokens)"
}
