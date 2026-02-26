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

  $strawberryPerl = "C:\Strawberry\perl\bin"
  $pathPrefix = ""
  if (Test-Path $strawberryPerl) {
    $pathPrefix = "set PATH=$strawberryPerl;%PATH% && "
  }

  $cmd = "`"$vsDevCmd`" -arch=x64 && $pathPrefix$Command"
  cmd /c $cmd
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed in VS developer environment (exit code $LASTEXITCODE): $Command"
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
    $src = Join-Path $ReleaseDir $item
    if (!(Test-Path $src)) {
      throw "Required portable artifact missing: $src"
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

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$desktopRoot = Join-Path $repoRoot "apps/desktop"
$tauriRoot = Join-Path $desktopRoot "src-tauri"
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

  $appVersion = Get-AppVersion -TauriConfigPath $tauriConfigPath
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

    Write-Host "[2/6] Build slim NSIS installer..."
    Invoke-InVsDevEnv -Command "pnpm --filter @lazycat/desktop exec tauri build --bundles nsis"
    $latestSlimSetup = Get-LatestSetupExe -NsisBundleDir $nsisBundleDir
    Copy-Item -Path $latestSlimSetup -Destination $setupLiteExe -Force

    Write-Host "[3/6] Build integrated NSIS installer (fixedRuntime)..."
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
    $h = Get-FileHash -Path $f -Algorithm SHA256
    "{0}  {1}" -f $h.Hash.ToLowerInvariant(), (Split-Path $f -Leaf)
  }
  $hashLines | Set-Content -Encoding UTF8 $shaFile

  if ($SkipUpload) {
    Write-Host "[6/6] Skip upload. Artifacts generated in: $outDir"
    return
  }

  Write-Host "[6/6] Push tag and upload GitHub release assets..."
  $tagQuery = git tag --list $Tag
  if ($LASTEXITCODE -ne 0) {
    throw "git tag --list failed with exit code $LASTEXITCODE"
  }
  $tagExists = (($tagQuery | Out-String).Trim())
  if (-not $tagExists) {
    git tag $Tag
    if ($LASTEXITCODE -ne 0) {
      throw "git tag failed with exit code $LASTEXITCODE"
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
  Pop-Location
}
