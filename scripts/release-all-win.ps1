param(
  [Parameter(Mandatory = $true)]
  [string]$Tag,
  [string]$Repo = "",
  [switch]$SkipBuild,
  [switch]$SkipUpload
)

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

function Ensure-Command {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [string]$Hint = ""
  )

  if (!(Get-Command $Name -ErrorAction SilentlyContinue)) {
    if ($Hint) {
      throw "Command not found: $Name. $Hint"
    }
    throw "Command not found: $Name"
  }
}

function Invoke-InVsDevEnv {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Command
  )

  $vsDevCmd = Resolve-VsDevCmd
  if (-not $vsDevCmd) {
    throw "VsDevCmd.bat not found. Install Visual Studio 2022 (Community/BuildTools) with Desktop C++ workload."
  }

  # Strip Git's usr/bin from PATH to prevent its link.exe shadowing MSVC's
  $savedPath = $env:Path
  $env:Path = ($env:Path -split ';' | Where-Object { $_ -notmatch 'Git\\usr\\bin' }) -join ';'

  $strawberryPerl = "C:\Strawberry\perl\bin"
  $pathPrefix = ""
  if (Test-Path $strawberryPerl) {
    $pathPrefix = "set PATH=$strawberryPerl;%PATH% && "
  }

  $cmd = "`"$vsDevCmd`" -arch=x64 && $pathPrefix$Command"
  try {
    cmd /c $cmd
    if ($LASTEXITCODE -ne 0) {
      throw "Command failed in VS developer environment (exit code $LASTEXITCODE): $Command"
    }
  }
  finally {
    $env:Path = $savedPath
  }
}

function Get-AppVersion {
  param(
    [Parameter(Mandatory = $true)]
    [string]$TauriConfigPath
  )

  $cfg = Get-Content $TauriConfigPath -Raw | ConvertFrom-Json
  return [string]$cfg.version
}

function Get-JsonVersion {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  $json = Get-Content $Path -Raw | ConvertFrom-Json
  return [string]$json.version
}

function Get-CargoPackageVersion {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  $content = Get-Content $Path -Raw
  $match = [regex]::Match($content, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
  if (-not $match.Success) {
    throw "Failed to read Cargo package version from: $Path"
  }
  return $match.Groups[1].Value
}

function Write-JsonFile {
  param(
    [Parameter(Mandatory = $true)]
    [object]$Object,
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  $Object | ConvertTo-Json -Depth 100 | Set-Content -Encoding UTF8 $Path
}

function New-Zip {
  param(
    [Parameter(Mandatory = $true)]
    [string]$ZipPath,
    [Parameter(Mandatory = $true)]
    [string]$SourceDir
  )

  if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
  }

  if (Get-Command 7z -ErrorAction SilentlyContinue) {
    Push-Location $SourceDir
    try {
      & 7z a -tzip $ZipPath "*"
      if ($LASTEXITCODE -ne 0) {
        throw "7z failed with exit code $LASTEXITCODE"
      }
    }
    finally {
      Pop-Location
    }
    return
  }

  Compress-Archive -Path (Join-Path $SourceDir "*") -DestinationPath $ZipPath -CompressionLevel Optimal
}

function Get-Sha256Hex {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  if (Get-Command Get-FileHash -ErrorAction SilentlyContinue) {
    return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
  }

  $stream = [System.IO.File]::OpenRead($Path)
  try {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
      $hashBytes = $sha256.ComputeHash($stream)
      return ([System.BitConverter]::ToString($hashBytes).Replace("-", "").ToLowerInvariant())
    }
    finally {
      $sha256.Dispose()
    }
  }
  finally {
    $stream.Dispose()
  }
}

function Remove-PathIfExists {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  if (Test-Path $Path) {
    Remove-Item -Recurse -Force $Path
  }
}

function Copy-PortableFiles {
  param(
    [Parameter(Mandatory = $true)]
    [string]$ReleaseDir,
    [Parameter(Mandatory = $true)]
    [string]$StageDir
  )

  $required = @(
    "lazycat-desktop.exe",
    "lazycat_lib.dll",
    "manuals",
    "regex-library",
    "hotkey-library"
  )

  foreach ($item in $required) {
    $candidates = @(
      (Join-Path $ReleaseDir $item)
    )

    if ($item -eq "lazycat_lib.dll") {
      $candidates += (Join-Path $ReleaseDir "deps/$item")
    }

    $src = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $src) {
      throw "Required portable artifact missing: $($candidates -join ', ')"
    }

    $dst = Join-Path $StageDir $item
    if (Test-Path $dst) {
      Remove-Item -Recurse -Force $dst
    }
    Copy-Item -Path $src -Destination $dst -Recurse -Force
  }
}

function Get-LatestSetupExe {
  param(
    [Parameter(Mandatory = $true)]
    [string]$NsisBundleDir
  )

  $setup = Get-ChildItem -Path $NsisBundleDir -Filter "*-setup.exe" -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $setup) {
    throw "NSIS setup executable not found under: $NsisBundleDir"
  }
  return $setup.FullName
}

function Ensure-ObjectProperty {
  param(
    [Parameter(Mandatory = $true)]
    [object]$Target,
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [Parameter(Mandatory = $true)]
    [object]$Value
  )

  if ($null -eq $Target.PSObject.Properties[$Name]) {
    $Target | Add-Member -MemberType NoteProperty -Name $Name -Value $Value
    return
  }

  if ($null -eq $Target.$Name) {
    $Target.$Name = $Value
  }
}

function Assert-ReleaseVersions {
  param(
    [Parameter(Mandatory = $true)]
    [hashtable]$Versions
  )

  $uniqueVersions = @($Versions.Values | Sort-Object -Unique)
  if ($uniqueVersions.Count -eq 1) {
    return [string]$uniqueVersions[0]
  }

  $details = $Versions.GetEnumerator() |
    Sort-Object Name |
    ForEach-Object { "{0}={1}" -f $_.Name, $_.Value }
  throw "Release version mismatch detected: $($details -join '; ')"
}

function New-LiteNsisFromFull {
  param(
    [Parameter(Mandatory = $true)]
    [string]$NsisDir,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath
  )

  $fullScript = Join-Path $NsisDir "installer.nsi"
  $liteScript = Join-Path $NsisDir "installer-lite.nsi"

  if (!(Test-Path $fullScript)) {
    throw "Full NSIS script not found: $fullScript"
  }

  $lines = Get-Content $fullScript -Encoding UTF8
  $filtered = foreach ($line in $lines) {
    # Skip WebView2 File directives (install section)
    if ($line -match 'File\s+/a\s+"/oname=WebView2\\') { continue }
    # Skip WebView2 Delete directives (uninstall section)
    if ($line -match 'Delete\s+"?\$INSTDIR\\WebView2\\') { continue }
    # Skip WebView2 RMDir directives (uninstall section)
    if ($line -match 'RMDir\s+/REBOOTOK\s+"?\$INSTDIR\\WebView2') { continue }

    # Update defines for lite variant
    if ($line -match '^\s*!define\s+INSTALLWEBVIEW2MODE') {
      '  !define INSTALLWEBVIEW2MODE "downloadBootstrapper"'
    }
    elseif ($line -match '^\s*!define\s+OUTFILE') {
      '  !define OUTFILE "nsis-output-lite.exe"'
    }
    else {
      $line
    }
  }

  $filtered | Set-Content -Encoding UTF8 $liteScript

  $tauriNsisDir = Join-Path $env:LOCALAPPDATA "tauri/NSIS"
  $makensis = Join-Path $tauriNsisDir "makensis.exe"
  if (!(Test-Path $makensis)) {
    throw "makensis.exe not found at: $makensis"
  }

  Push-Location $NsisDir
  try {
    & $makensis $liteScript 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) {
      throw "makensis failed for lite installer with exit code $LASTEXITCODE"
    }
  }
  finally {
    Pop-Location
  }

  $liteOutput = Join-Path $NsisDir "nsis-output-lite.exe"
  if (!(Test-Path $liteOutput)) {
    throw "Lite NSIS output not found: $liteOutput"
  }
  Copy-Item -Path $liteOutput -Destination $OutputPath -Force

  # Cleanup temp files
  Remove-Item -Force $liteScript -ErrorAction SilentlyContinue
  Remove-Item -Force $liteOutput -ErrorAction SilentlyContinue
}

function Get-CurrentBranch {
  $branch = ((git branch --show-current) | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) {
    throw "git branch --show-current failed with exit code $LASTEXITCODE"
  }
  if (-not $branch) {
    throw "Current git branch is empty. Release script requires a named branch."
  }
  return $branch
}

function Ensure-CleanWorktree {
  $status = git status --short
  if ($LASTEXITCODE -ne 0) {
    throw "git status --short failed with exit code $LASTEXITCODE"
  }
  $dirty = (($status | Out-String).Trim())
  if ($dirty) {
    throw "Working tree is not clean. Commit or stash changes before publishing a GitHub Release.`n$dirty"
  }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$desktopRoot = Join-Path $repoRoot "apps/desktop"
$tauriRoot = Join-Path $desktopRoot "src-tauri"
$rootPackageJsonPath = Join-Path $repoRoot "package.json"
$desktopPackageJsonPath = Join-Path $desktopRoot "package.json"
$cargoTomlPath = Join-Path $tauriRoot "Cargo.toml"
$releaseDir = Join-Path $tauriRoot "target/release"
$nsisBundleDir = Join-Path $releaseDir "bundle/nsis"
$tauriConfigPath = Join-Path $tauriRoot "tauri.conf.json"
$offlineConfigPath = Join-Path $tauriRoot "tauri.conf.release-offline.tmp.json"
$outDir = Join-Path $repoRoot "dist/releases/$Tag"
$stageLiteDir = Join-Path $outDir "_stage_portable_lite"
$stageFullDir = Join-Path $outDir "_stage_portable_full"

Push-Location $repoRoot
try {
  Ensure-Command -Name "pnpm" -Hint "Install Node.js and pnpm first."
  Ensure-Command -Name "git"
  if (!(Get-Command gh -ErrorAction SilentlyContinue)) {
    $ghDir = "C:\Program Files\GitHub CLI"
    if (Test-Path (Join-Path $ghDir "gh.exe")) {
      $env:Path = "$ghDir;$($env:Path)"
    }
  }
  Ensure-Command -Name "gh" -Hint "Install GitHub CLI and run: gh auth login"

  $appVersion = Assert-ReleaseVersions -Versions @{
    "package.json" = (Get-JsonVersion -Path $rootPackageJsonPath)
    "apps/desktop/package.json" = (Get-JsonVersion -Path $desktopPackageJsonPath)
    "apps/desktop/src-tauri/Cargo.toml" = (Get-CargoPackageVersion -Path $cargoTomlPath)
    "apps/desktop/src-tauri/tauri.conf.json" = (Get-AppVersion -TauriConfigPath $tauriConfigPath)
  }
  $expectedTag = "v$appVersion"
  if ($Tag -ne $expectedTag) {
    throw "Tag/version mismatch. Expected tag $expectedTag for app version $appVersion, but got $Tag."
  }

  $currentBranch = $null
  if (-not $SkipUpload) {
    & gh auth status 1> $null
    if ($LASTEXITCODE -ne 0) {
      throw "gh auth status failed. Run 'gh auth login' first."
    }

    $currentBranch = Get-CurrentBranch
    if ($currentBranch -ne "main") {
      throw "GitHub Release publishing must run from main. Current branch: $currentBranch"
    }
    Ensure-CleanWorktree
  }

  $baseName = "Lazycat_${appVersion}_x64"
  $portableLiteZip = Join-Path $outDir "${baseName}_portable-lite.zip"
  $portableFullZip = Join-Path $outDir "${baseName}_portable-full.zip"
  $setupLiteExe = Join-Path $outDir "${baseName}_setup-lite.exe"
  $setupFullExe = Join-Path $outDir "${baseName}_setup-full.exe"
  $shaFile = Join-Path $outDir "SHA256SUMS.txt"

  $runtimeDir = Get-ChildItem -Path (Join-Path $tauriRoot "WebView2") -Directory -Filter "Microsoft.WebView2.FixedVersionRuntime.*" | Select-Object -First 1
  if (-not $runtimeDir) {
    throw "WebView2 Fixed Runtime not found under $tauriRoot\WebView2. Required for integrated package."
  }

  if (!(Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
  }

  if (-not $SkipBuild) {
    Write-Host "[1/6] Build web renderer..."
    pnpm --filter @lazycat/desktop build:web
    if ($LASTEXITCODE -ne 0) {
      throw "build:web failed with exit code $LASTEXITCODE"
    }

    Write-Host "[2/6] Build integrated NSIS installer (fixedRuntime)..."
    $offlineCfg = Get-Content $tauriConfigPath -Raw | ConvertFrom-Json
    Ensure-ObjectProperty -Target $offlineCfg -Name "bundle" -Value ([pscustomobject]@{})
    Ensure-ObjectProperty -Target $offlineCfg.bundle -Name "windows" -Value ([pscustomobject]@{})
    Ensure-ObjectProperty -Target $offlineCfg.bundle.windows -Name "webviewInstallMode" -Value ([pscustomobject]@{})
    $offlineCfg.bundle.windows.webviewInstallMode = [pscustomobject]@{
      type = "fixedRuntime"
      path = "WebView2/$($runtimeDir.Name)"
    }
    Write-JsonFile -Object $offlineCfg -Path $offlineConfigPath

    try {
      Invoke-InVsDevEnv -Command "pnpm --filter @lazycat/desktop exec tauri build --bundles nsis --config src-tauri/tauri.conf.release-offline.tmp.json"
    }
    finally {
      if (Test-Path $offlineConfigPath) {
        Remove-Item -Force $offlineConfigPath
      }
    }
    $latestFullSetup = Get-LatestSetupExe -NsisBundleDir $nsisBundleDir
    Copy-Item -Path $latestFullSetup -Destination $setupFullExe -Force

    Write-Host "[3/6] Build slim NSIS installer (makensis only)..."
    $nsisDir = Join-Path $releaseDir "nsis/x64"
    New-LiteNsisFromFull -NsisDir $nsisDir -OutputPath $setupLiteExe

    Write-Host "[4/6] Build portable zip packages..."
    foreach ($dir in @($stageLiteDir, $stageFullDir)) {
      if (Test-Path $dir) {
        Remove-Item -Recurse -Force $dir
      }
      New-Item -ItemType Directory -Path $dir | Out-Null
    }

    Copy-PortableFiles -ReleaseDir $releaseDir -StageDir $stageLiteDir
    Copy-PortableFiles -ReleaseDir $releaseDir -StageDir $stageFullDir
    Copy-Item -Path $runtimeDir.FullName -Destination (Join-Path $stageFullDir $runtimeDir.Name) -Recurse -Force

    New-Zip -ZipPath $portableLiteZip -SourceDir $stageLiteDir
    New-Zip -ZipPath $portableFullZip -SourceDir $stageFullDir

    foreach ($dir in @($stageLiteDir, $stageFullDir)) {
      if (Test-Path $dir) {
        Remove-Item -Recurse -Force $dir
      }
    }
  }

  Write-Host "[5/6] Generate SHA256 file..."
  $artifacts = @($setupLiteExe, $setupFullExe, $portableLiteZip, $portableFullZip)
  foreach ($f in $artifacts) {
    if (!(Test-Path $f)) {
      throw "Artifact missing: $f"
    }
  }

  $hashLines = foreach ($f in $artifacts) {
    "{0}  {1}" -f (Get-Sha256Hex -Path $f), (Split-Path $f -Leaf)
  }
  $hashLines | Set-Content -Encoding UTF8 $shaFile

  if ($SkipUpload) {
    Write-Host "[6/6] Skip upload. Artifacts generated in: $outDir"
    return
  }

  Write-Host "[6/6] Push tag and upload GitHub release assets..."
  git push origin $currentBranch
  if ($LASTEXITCODE -ne 0) {
    throw "git push origin $currentBranch failed with exit code $LASTEXITCODE"
  }

  $tagQuery = git tag --list $Tag
  if ($LASTEXITCODE -ne 0) {
    throw "git tag --list failed with exit code $LASTEXITCODE"
  }
  $tagExists = (($tagQuery | Out-String).Trim())
  $headCommit = ((git rev-parse HEAD) | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) {
    throw "git rev-parse HEAD failed with exit code $LASTEXITCODE"
  }
  if (-not $tagExists) {
    git tag $Tag
    if ($LASTEXITCODE -ne 0) {
      throw "git tag failed with exit code $LASTEXITCODE"
    }
  }
  else {
    $tagCommit = ((git rev-list -n 1 $Tag) | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) {
      throw "git rev-list -n 1 $Tag failed with exit code $LASTEXITCODE"
    }
    if ($tagCommit -ne $headCommit) {
      throw "Tag $Tag already exists at $tagCommit, but current HEAD is $headCommit."
    }
  }

  git push origin $Tag
  if ($LASTEXITCODE -ne 0) {
    throw "git push origin $Tag failed with exit code $LASTEXITCODE"
  }

  $ghCommon = @()
  if ($Repo) {
    $ghCommon += @("--repo", $Repo)
  }

  $releaseExists = $true
  $oldEap = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    & gh release view $Tag @ghCommon 1> $null 2> $null
    $releaseViewCode = $LASTEXITCODE
  }
  finally {
    $ErrorActionPreference = $oldEap
  }
  if ($releaseViewCode -ne 0) {
    $releaseExists = $false
  }

  $assetsToUpload = @($setupLiteExe, $setupFullExe, $portableLiteZip, $portableFullZip, $shaFile)
  if ($releaseExists) {
    & gh release upload $Tag @assetsToUpload --clobber @ghCommon
    if ($LASTEXITCODE -ne 0) {
      throw "gh release upload failed with exit code $LASTEXITCODE"
    }
  }
  else {
    & gh release create $Tag @assetsToUpload --title $Tag --generate-notes @ghCommon
    if ($LASTEXITCODE -ne 0) {
      throw "gh release create failed with exit code $LASTEXITCODE"
    }
  }

  Write-Host "Done. Release artifacts:"
  Get-ChildItem -Path $outDir -File | Select-Object Name, Length, LastWriteTime
}
finally {
  Remove-PathIfExists -Path $stageLiteDir
  Remove-PathIfExists -Path $stageFullDir
  if (Test-Path $offlineConfigPath) {
    Remove-Item -Force $offlineConfigPath
  }
  Pop-Location
}
