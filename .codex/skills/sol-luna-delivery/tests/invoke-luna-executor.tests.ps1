[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$scriptRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\scripts\invoke-luna-executor.ps1")).Path
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$testShell = (Get-Process -Id $PID).Path
$tempBase = $PSScriptRoot
$testRoot = Join-Path $tempBase ("lazycat-sol-luna-tests-{0}" -f [Guid]::NewGuid().ToString('N'))
$passed = 0
$backgroundProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()

function Assert-True {
  param(
    [Parameter(Mandatory = $true)]
    [bool]$Condition,

    [Parameter(Mandatory = $true)]
    [string]$Message
  )

  if (!$Condition) {
    throw $Message
  }
}

function Assert-Contains {
  param(
    [AllowEmptyString()]
    [string]$Value,

    [Parameter(Mandatory = $true)]
    [string]$Expected,

    [Parameter(Mandatory = $true)]
    [string]$Message
  )

  if ($Value.IndexOf($Expected, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
    throw "$Message Expected '$Expected'. Actual: $Value"
  }
}

function Complete-Test {
  param([Parameter(Mandatory = $true)][string]$Name)

  $script:passed++
  Write-Host "PASS $Name"
}

function Invoke-Git {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Root,

    [Parameter(Mandatory = $true)]
    [string[]]$Arguments
  )

  $output = @(& git -C $Root @Arguments 2>&1)
  if ($LASTEXITCODE -ne 0) {
    throw "git $($Arguments -join ' ') failed: $($output -join "`n")"
  }
  return $output
}

function New-TestRepository {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,

    [switch]$WithAllowedFile
  )

  $root = Join-Path $testRoot $Name
  $null = New-Item -ItemType Directory -Path $root
  $null = Invoke-Git -Root $root -Arguments @("init", "--quiet")
  $null = Invoke-Git -Root $root -Arguments @("config", "user.email", "sol-luna-tests@example.invalid")
  $null = Invoke-Git -Root $root -Arguments @("config", "user.name", "Sol Luna Tests")
  [IO.File]::WriteAllText((Join-Path $root "baseline.txt"), "baseline", [Text.UTF8Encoding]::new($false))
  if ($WithAllowedFile) {
    [IO.File]::WriteAllText((Join-Path $root "allowed.txt"), "committed", [Text.UTF8Encoding]::new($false))
  }
  $null = Invoke-Git -Root $root -Arguments @("add", ".")
  $null = Invoke-Git -Root $root -Arguments @("commit", "--quiet", "-m", "test baseline")
  return $root
}

function Write-TaskPacket {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [ValidateSet("read-only-analysis", "implementation")]
    [string]$Type,

    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [string[]]$AllowedPaths
  )

  $allowedJson = ConvertTo-Json -InputObject @($AllowedPaths) -Compress
  $content = @"
# Executor Task Packet

## Task Type

$Type

## Goal

Exercise the deterministic wrapper test path.

## Context

The fake executor is local and must not access a real model.

## Confirmed Decisions

The test case controls all behavior through a fake command.

## Non-goals

No network or product behavior.

## Allowed Scope

Use only the temporary test repository.

## Allowed Changed Paths

$allowedJson

## Forbidden Scope

Do not access the LazyCat worktree.

## Steps

Run the deterministic fake behavior and return its result.

## Acceptance Criteria

The wrapper accepts or rejects the case as specified by the test.

## Validation

Use wrapper exit status and generated evidence files.

## Stop Conditions

Stop on any unexpected command or path.

## Output Requirements

Return one schema-compatible JSON result.
"@
  [IO.File]::WriteAllText($Path, $content, [Text.UTF8Encoding]::new($false))
}

function New-CasePaths {
  param([Parameter(Mandatory = $true)][string]$Name)

  return [PSCustomObject]@{
    Task = Join-Path $testRoot "$Name.task.md"
    Result = Join-Path $testRoot "$Name.result.json"
    Event = Join-Path $testRoot "$Name.events.jsonl"
    Stderr = Join-Path $testRoot "$Name.stderr.log"
    HostOut = Join-Path $testRoot "$Name.host.out.log"
    HostErr = Join-Path $testRoot "$Name.host.err.log"
  }
}

function Invoke-WrapperCase {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,

    [Parameter(Mandatory = $true)]
    [string]$Repo,

    [Parameter(Mandatory = $true)]
    [ValidateSet("read-only-analysis", "implementation")]
    [string]$Type,

    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [string[]]$AllowedPaths,

    [Parameter(Mandatory = $true)]
    [string]$Mode,

    [int]$TimeoutSeconds = 10
  )

  $paths = New-CasePaths -Name $Name
  Write-TaskPacket -Path $paths.Task -Type $Type -AllowedPaths $AllowedPaths
  $env:SOL_LUNA_FAKE_MODE = $Mode
  $env:SOL_LUNA_FAKE_TASK_TYPE = $Type
  $previousErrorAction = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $output = @(& $testShell -NoProfile -File $scriptRoot `
      -RepoRoot $Repo `
      -TaskPath $paths.Task `
      -TaskType $Type `
      -ReasoningEffort xhigh `
      -ResultPath $paths.Result `
      -EventLogPath $paths.Event `
      -StderrLogPath $paths.Stderr `
      -TimeoutSeconds $TimeoutSeconds `
      -CodexCommand $fakeCodex 2>&1)
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorAction
  }
  return [PSCustomObject]@{
    ExitCode = $exitCode
    Output = $output -join "`n"
    Paths = $paths
  }
}

function Get-WrapperArguments {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Repo,

    [Parameter(Mandatory = $true)]
    [string]$Task,

    [Parameter(Mandatory = $true)]
    [string]$Result,

    [Parameter(Mandatory = $true)]
    [string]$Event,

    [Parameter(Mandatory = $true)]
    [string]$Stderr,

    [int]$TimeoutSeconds = 10
  )

  return @(
    "-NoProfile", "-File", $scriptRoot,
    "-RepoRoot", $Repo,
    "-TaskPath", $Task,
    "-TaskType", "implementation",
    "-ReasoningEffort", "xhigh",
    "-ResultPath", $Result,
    "-EventLogPath", $Event,
    "-StderrLogPath", $Stderr,
    "-TimeoutSeconds", $TimeoutSeconds,
    "-CodexCommand", $fakeCodex
  )
}

function Invoke-TestShell {
  param(
    [Parameter(Mandatory = $true)]
    [string[]]$Arguments
  )

  $previousErrorAction = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $output = @(& $testShell @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorAction
  }
  return [PSCustomObject]@{
    ExitCode = $exitCode
    Output = $output -join "`n"
  }
}

try {
  $null = New-Item -ItemType Directory -Path $testRoot
  $fakeCodex = Join-Path $testRoot "fake-codex.ps1"
  $fakeContent = @'
$allArguments = @($args)
$repoIndex = [Array]::IndexOf($allArguments, "-C")
$resultIndex = [Array]::IndexOf($allArguments, "-o")
if ($repoIndex -lt 0 -or $resultIndex -lt 0 -or $allArguments -notcontains "--ephemeral") {
  [Console]::Error.WriteLine("fake executor received invalid arguments")
  throw "fake executor received invalid arguments"
}

$repoRoot = $allArguments[$repoIndex + 1]
$resultPath = $allArguments[$resultIndex + 1]
$null = [Console]::In.ReadToEnd()
$mode = $env:SOL_LUNA_FAKE_MODE
$taskType = $env:SOL_LUNA_FAKE_TASK_TYPE
[Console]::Out.WriteLine('{"type":"thread.started","thread_id":"fake"}')
[Console]::Out.Flush()

if ($mode -eq "nonzero") {
  [Console]::Error.WriteLine("fake stderr marker")
  throw "fake nonzero exit"
}
if ($mode -eq "timeout" -or $mode -eq "sleep-success") {
  Start-Sleep -Seconds 4
}

$changedFiles = @()
if ($mode -eq "write-allowed" -or $mode -eq "mismatch" -or $mode -eq "dirty-write") {
  [IO.File]::WriteAllText((Join-Path $repoRoot "allowed.txt"), "changed-$mode", [Text.UTF8Encoding]::new($false))
  if ($mode -ne "mismatch") { $changedFiles = @("allowed.txt") }
}
if ($mode -eq "write-outside") {
  [IO.File]::WriteAllText((Join-Path $repoRoot "outside.txt"), "outside", [Text.UTF8Encoding]::new($false))
  $changedFiles = @("outside.txt")
}
if ($mode -eq "malformed") {
  [IO.File]::WriteAllText($resultPath, "{bad", [Text.UTF8Encoding]::new($false))
  return
}

$result = [ordered]@{
  taskType = $taskType
  reasoningEffort = "xhigh"
  status = "success"
  summary = "fake success"
  scopeInspected = @()
  changedFiles = $changedFiles
  commands = @()
  validation = @()
  findings = @()
  uninspectedScope = @()
  scopeDeviations = @()
  remainingIssues = @()
}
if ($mode -eq "invalid-shape") {
  $result.Remove("remainingIssues")
}
$json = ConvertTo-Json -InputObject $result -Depth 8 -Compress
[IO.File]::WriteAllText($resultPath, $json, [Text.UTF8Encoding]::new($false))
[Console]::Out.WriteLine('{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}')
[Console]::Out.Flush()
return
'@
  [IO.File]::WriteAllText($fakeCodex, $fakeContent, [Text.UTF8Encoding]::new($false))

  $validateRepo = New-TestRepository -Name "validate-only-repo"
  $validatePaths = New-CasePaths -Name "validate-only"
  $validateOutput = @(& $testShell -NoProfile -File $scriptRoot `
    -RepoRoot $validateRepo `
    -TaskPath (Join-Path $repositoryRoot ".codex\skills\sol-luna-delivery\evals\files\valid-read-only-task.md") `
    -TaskType read-only-analysis `
    -ResultPath $validatePaths.Result `
    -EventLogPath $validatePaths.Event `
    -StderrLogPath $validatePaths.Stderr `
    -CodexCommand $fakeCodex `
    -ValidateOnly 2>&1)
  Assert-True -Condition ($LASTEXITCODE -eq 0) -Message "ValidateOnly should succeed: $($validateOutput -join "`n")"
  Assert-True -Condition (!(Test-Path -LiteralPath $validatePaths.Result) -and !(Test-Path -LiteralPath $validatePaths.Event) -and !(Test-Path -LiteralPath $validatePaths.Stderr)) -Message "ValidateOnly must not create outputs."
  Assert-Contains -Value ($validateOutput -join "`n") -Expected "Stderr log:" -Message "ValidateOnly must report stderr path."
  Complete-Test "ValidateOnly"

  $repo = New-TestRepository -Name "read-only"
  $case = Invoke-WrapperCase -Name "read-only" -Repo $repo -Type read-only-analysis -AllowedPaths @() -Mode "success"
  $caseStderr = if (Test-Path -LiteralPath $case.Paths.Stderr) { Get-Content -Raw $case.Paths.Stderr } else { "<missing>" }
  Assert-True -Condition ($case.ExitCode -eq 0) -Message "Read-only success failed: $($case.Output) Fake stderr: $caseStderr"
  Assert-True -Condition (Test-Path -LiteralPath $case.Paths.Event) -Message "Read-only event log is missing."
  Complete-Test "read-only success"

  $repo = New-TestRepository -Name "implementation"
  $case = Invoke-WrapperCase -Name "implementation" -Repo $repo -Type implementation -AllowedPaths @("allowed.txt") -Mode "write-allowed"
  Assert-True -Condition ($case.ExitCode -eq 0) -Message "Implementation success failed: $($case.Output)"
  Assert-Contains -Value $case.Output -Expected "Actual changed paths: allowed.txt" -Message "Implementation must report actual path."
  Complete-Test "implementation exact scope"

  $repo = New-TestRepository -Name "dirty" -WithAllowedFile
  [IO.File]::WriteAllText((Join-Path $repo "allowed.txt"), "dirty-before", [Text.UTF8Encoding]::new($false))
  $case = Invoke-WrapperCase -Name "dirty" -Repo $repo -Type implementation -AllowedPaths @("allowed.txt") -Mode "dirty-write"
  Assert-True -Condition ($case.ExitCode -eq 0) -Message "Dirty baseline change was not detected correctly: $($case.Output)"
  Complete-Test "baseline dirty path delta"

  $repo = New-TestRepository -Name "outside"
  $case = Invoke-WrapperCase -Name "outside" -Repo $repo -Type implementation -AllowedPaths @("allowed.txt") -Mode "write-outside"
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "Out-of-scope write should fail."
  Assert-Contains -Value $case.Output -Expected "outside Allowed Changed Paths" -Message "Out-of-scope failure is unclear."
  Complete-Test "out-of-scope rejection"

  $repo = New-TestRepository -Name "mismatch"
  $case = Invoke-WrapperCase -Name "mismatch" -Repo $repo -Type implementation -AllowedPaths @("allowed.txt") -Mode "mismatch"
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "changedFiles mismatch should fail."
  Assert-Contains -Value $case.Output -Expected "does not match actual changes" -Message "changedFiles mismatch failure is unclear."
  Complete-Test "changedFiles mismatch"

  $repo = New-TestRepository -Name "nonzero"
  $case = Invoke-WrapperCase -Name "nonzero" -Repo $repo -Type implementation -AllowedPaths @() -Mode "nonzero"
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "Nonzero fake executor should fail."
  Assert-Contains -Value $case.Output -Expected "Final HEAD:" -Message "Nonzero failure must include final HEAD."
  Assert-Contains -Value (Get-Content -Raw $case.Paths.Stderr) -Expected "fake stderr marker" -Message "Stderr log did not retain diagnostics."
  Complete-Test "nonzero diagnostics"

  $repo = New-TestRepository -Name "malformed"
  $case = Invoke-WrapperCase -Name "malformed" -Repo $repo -Type implementation -AllowedPaths @() -Mode "malformed"
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "Malformed result should fail."
  Assert-Contains -Value $case.Output -Expected "not valid JSON" -Message "Malformed result failure is unclear."
  Complete-Test "malformed result"

  $repo = New-TestRepository -Name "invalid-shape"
  $case = Invoke-WrapperCase -Name "invalid-shape" -Repo $repo -Type implementation -AllowedPaths @() -Mode "invalid-shape"
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "Invalid result shape should fail."
  Assert-Contains -Value $case.Output -Expected "missing property: remainingIssues" -Message "Invalid result shape failure is unclear."
  Complete-Test "local result contract"

  $repo = New-TestRepository -Name "timeout"
  $timer = [Diagnostics.Stopwatch]::StartNew()
  $case = Invoke-WrapperCase -Name "timeout" -Repo $repo -Type implementation -AllowedPaths @() -Mode "timeout" -TimeoutSeconds 1
  $timer.Stop()
  Assert-True -Condition ($case.ExitCode -ne 0) -Message "Timeout should fail."
  Assert-Contains -Value $case.Output -Expected "timed out after 1 seconds" -Message "Timeout failure is unclear."
  Assert-True -Condition ($timer.Elapsed.TotalSeconds -lt 8) -Message "Timeout did not stop the fake executor promptly."
  $remainingTestProcesses = @(Get-CimInstance Win32_Process | Where-Object {
    $null -ne $_.CommandLine -and $_.CommandLine.IndexOf($testRoot, [StringComparison]::OrdinalIgnoreCase) -ge 0
  })
  Assert-True -Condition ($remainingTestProcesses.Count -eq 0) -Message "Timeout left a fake executor process running."
  Complete-Test "timeout and process cleanup"

  $repo = New-TestRepository -Name "streaming"
  $paths = New-CasePaths -Name "streaming"
  Write-TaskPacket -Path $paths.Task -Type implementation -AllowedPaths @()
  $env:SOL_LUNA_FAKE_MODE = "sleep-success"
  $env:SOL_LUNA_FAKE_TASK_TYPE = "implementation"
  $arguments = Get-WrapperArguments -Repo $repo -Task $paths.Task -Result $paths.Result -Event $paths.Event -Stderr $paths.Stderr
  $process = Start-Process -FilePath $testShell -ArgumentList $arguments -RedirectStandardOutput $paths.HostOut -RedirectStandardError $paths.HostErr -PassThru
  $backgroundProcesses.Add($process)
  $deadline = [DateTime]::UtcNow.AddSeconds(8)
  while (
    (!(Test-Path -LiteralPath $paths.Event) -or (Get-Item -LiteralPath $paths.Event -ErrorAction SilentlyContinue).Length -eq 0) -and
    [DateTime]::UtcNow -lt $deadline
  ) { Start-Sleep -Milliseconds 100 }
  Assert-True -Condition (Test-Path -LiteralPath $paths.Event) -Message "Event log was not created while executor was running."
  Assert-True -Condition ((Get-Item $paths.Event).Length -gt 0 -and !$process.HasExited) -Message "Event log did not stream before executor exit."
  $process.WaitForExit()
  $streamingHostError = Get-Content -Raw $paths.HostErr
  $streamingSucceeded = if ($null -eq $process.ExitCode) {
    (Test-Path -LiteralPath $paths.Result) -and [string]::IsNullOrWhiteSpace($streamingHostError)
  } else {
    $process.ExitCode -eq 0
  }
  Assert-True -Condition $streamingSucceeded -Message "Streaming wrapper failed: $streamingHostError"
  $process.Dispose()
  Complete-Test "live event streaming"

  $repo = New-TestRepository -Name "mutex"
  $first = New-CasePaths -Name "mutex-first"
  $second = New-CasePaths -Name "mutex-second"
  Write-TaskPacket -Path $first.Task -Type implementation -AllowedPaths @()
  Write-TaskPacket -Path $second.Task -Type implementation -AllowedPaths @()
  $env:SOL_LUNA_FAKE_MODE = "sleep-success"
  $env:SOL_LUNA_FAKE_TASK_TYPE = "implementation"
  $arguments = Get-WrapperArguments -Repo $repo -Task $first.Task -Result $first.Result -Event $first.Event -Stderr $first.Stderr
  $firstProcess = Start-Process -FilePath $testShell -ArgumentList $arguments -RedirectStandardOutput $first.HostOut -RedirectStandardError $first.HostErr -PassThru
  $backgroundProcesses.Add($firstProcess)
  $deadline = [DateTime]::UtcNow.AddSeconds(8)
  while (!(Test-Path -LiteralPath $first.Event) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
  Assert-True -Condition (Test-Path -LiteralPath $first.Event) -Message "First writer did not start."
  $env:SOL_LUNA_FAKE_MODE = "success"
  $previousErrorAction = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $secondOutput = @(& $testShell -NoProfile -File $scriptRoot `
      -RepoRoot $repo -TaskPath $second.Task -TaskType implementation -ReasoningEffort xhigh `
      -ResultPath $second.Result -EventLogPath $second.Event -StderrLogPath $second.Stderr `
      -TimeoutSeconds 10 -CodexCommand $fakeCodex 2>&1)
    $secondExitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorAction
  }
  Assert-True -Condition ($secondExitCode -ne 0) -Message "Second writer should fail while the mutex is held."
  Assert-Contains -Value ($secondOutput -join "`n") -Expected "already running" -Message "Writer lock failure is unclear."
  $firstProcess.WaitForExit()
  $firstHostError = Get-Content -Raw $first.HostErr
  $firstSucceeded = if ($null -eq $firstProcess.ExitCode) {
    (Test-Path -LiteralPath $first.Result) -and [string]::IsNullOrWhiteSpace($firstHostError)
  } else {
    $firstProcess.ExitCode -eq 0
  }
  Assert-True -Condition $firstSucceeded -Message "First writer failed: $firstHostError"
  $firstProcess.Dispose()
  Complete-Test "writer mutex"

  $repo = New-TestRepository -Name "preflight"
  $paths = New-CasePaths -Name "preflight"
  Write-TaskPacket -Path $paths.Task -Type read-only-analysis -AllowedPaths @()
  $previousErrorAction = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $preflightOutput = @(& $testShell -NoProfile -File $scriptRoot -RepoRoot $repo -TaskPath $paths.Task `
      -TaskType read-only-analysis -ReasoningEffort low -ResultPath $paths.Result -CodexCommand $fakeCodex -ValidateOnly 2>&1)
    $preflightExitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorAction
  }
  Assert-True -Condition ($preflightExitCode -ne 0) -Message "Missing downgrade reason should fail."
  Assert-Contains -Value ($preflightOutput -join "`n") -Expected "DowngradeReason is required" -Message "Downgrade failure is unclear."

  $typePaths = New-CasePaths -Name "preflight-type"
  $typeResult = Invoke-TestShell -Arguments @(
    "-NoProfile", "-File", $scriptRoot,
    "-RepoRoot", $repo,
    "-TaskPath", $paths.Task,
    "-TaskType", "implementation",
    "-ResultPath", $typePaths.Result,
    "-CodexCommand", $fakeCodex,
    "-ValidateOnly"
  )
  Assert-True -Condition ($typeResult.ExitCode -ne 0) -Message "Task type mismatch should fail."
  Assert-Contains -Value $typeResult.Output -Expected "Task type mismatch" -Message "Task type mismatch failure is unclear."

  $insideResult = Join-Path $repo "inside-result.json"
  $insideResultCheck = Invoke-TestShell -Arguments @(
    "-NoProfile", "-File", $scriptRoot,
    "-RepoRoot", $repo,
    "-TaskPath", $paths.Task,
    "-TaskType", "read-only-analysis",
    "-ResultPath", $insideResult,
    "-CodexCommand", $fakeCodex,
    "-ValidateOnly"
  )
  Assert-True -Condition ($insideResultCheck.ExitCode -ne 0) -Message "Repository-contained result path should fail."
  Assert-Contains -Value $insideResultCheck.Output -Expected "must be outside the repository" -Message "Repository-contained result failure is unclear."

  $insideTask = Join-Path $repo "inside.task.md"
  Write-TaskPacket -Path $insideTask -Type read-only-analysis -AllowedPaths @()
  $insideTaskPaths = New-CasePaths -Name "preflight-inside-task"
  $insideTaskCheck = Invoke-TestShell -Arguments @(
    "-NoProfile", "-File", $scriptRoot,
    "-RepoRoot", $repo,
    "-TaskPath", $insideTask,
    "-TaskType", "read-only-analysis",
    "-ResultPath", $insideTaskPaths.Result,
    "-CodexCommand", $fakeCodex
  )
  Assert-True -Condition ($insideTaskCheck.ExitCode -ne 0) -Message "Repository-contained execution task should fail."
  Assert-Contains -Value $insideTaskCheck.Output -Expected "Task packet must be outside" -Message "Repository-contained task failure is unclear."
  Complete-Test "preflight negative"

  Write-Host "All $passed Sol-Luna wrapper tests passed."
} finally {
  Remove-Item Env:SOL_LUNA_FAKE_MODE -ErrorAction SilentlyContinue
  Remove-Item Env:SOL_LUNA_FAKE_TASK_TYPE -ErrorAction SilentlyContinue
  foreach ($child in $backgroundProcesses) {
    try {
      if (!$child.HasExited) {
        Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
        $null = $child.WaitForExit(5000)
      }
      $child.Dispose()
    } catch {
      # Cleanup remains best effort; the validated test root check below still applies.
    }
  }
  $resolvedBase = (Resolve-Path $tempBase).Path.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
  $candidate = [IO.Path]::GetFullPath($testRoot)
  if ($candidate.StartsWith($resolvedBase, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $candidate)) {
    Remove-Item -LiteralPath $candidate -Recurse -Force
  }
}
