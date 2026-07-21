$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$rootPackageJsonPath = Join-Path $repoRoot "package.json"
$rootPackage = Get-Content $rootPackageJsonPath -Raw | ConvertFrom-Json
$version = [string]$rootPackage.version

if ([string]::IsNullOrWhiteSpace($version)) {
  throw "Root package version is missing: $rootPackageJsonPath"
}

$releaseScript = Join-Path $PSScriptRoot "release-all-win.ps1"
Write-Host "Packaging Lazycat v$version as lite portable (local only, no upload)..."
& $releaseScript -Tag "v$version" -SkipUpload
