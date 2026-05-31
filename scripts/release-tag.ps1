param(
  [Parameter(Mandatory = $true)]
  [string]$Tag,
  [string]$Remote = "origin",
  [switch]$SkipChecks,
  [switch]$AllowDirty
)

$ErrorActionPreference = "Stop"

function Fail($msg) {
  Write-Error $msg
  exit 1
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  Fail "git is required"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Fail "cargo is required"
}

$version = $Tag
if ($version.StartsWith("v")) {
  $version = $version.Substring(1)
}

if ($version -notmatch '^\d+\.\d+\.\d+([\-+][0-9A-Za-z\.\-]+)?$') {
  Fail "invalid tag/version: '$Tag'. Expected like v1.2.3"
}

$repoRoot = (git rev-parse --show-toplevel).Trim()
Set-Location $repoRoot

$status = git status --porcelain
if (-not $AllowDirty -and -not [string]::IsNullOrWhiteSpace($status)) {
  Fail "working tree is not clean. Commit/stash first, or pass -AllowDirty"
}

Write-Host "==> Updating crate versions to $version"

$cargoTomls = Get-ChildItem -Path $repoRoot -Recurse -Filter Cargo.toml -File |
  Where-Object { $_.FullName -notmatch '\\target\\' -and $_.FullName -notmatch '\\.git\\' }

foreach ($file in $cargoTomls) {
  $lines = Get-Content -LiteralPath $file.FullName
  $pkgIdx = -1
  $verIdx = -1
  for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^\s*\[package\]\s*$') {
      $pkgIdx = $i
      break
    }
  }
  if ($pkgIdx -lt 0) {
    Write-Host "  skip (workspace-only): $($file.FullName)"
    continue
  }
  for ($j = $pkgIdx + 1; $j -lt $lines.Count; $j++) {
    if ($lines[$j] -match '^\s*\[') { break }
    if ($lines[$j] -match '^\s*version\s*=\s*".*"\s*$') {
      $verIdx = $j
      break
    }
  }
  if ($verIdx -lt 0) {
    Fail "invalid Cargo.toml (missing package version): $($file.FullName)"
  }

  $newLine = 'version = "' + $version + '"'
  if ($lines[$verIdx] -ne $newLine) {
    $lines[$verIdx] = $newLine
    Set-Content -LiteralPath $file.FullName -Value $lines
    Write-Host "  updated: $($file.FullName)"
  }
}

if (-not $SkipChecks) {
  Write-Host "==> Running checks"
  cargo fmt
  cargo check --workspace
  cargo build --workspace
  cargo test --workspace
}

Write-Host "==> Committing release changes"
git add -A
$staged = git diff --cached --name-only
if ([string]::IsNullOrWhiteSpace($staged)) {
  Fail "no version changes staged"
}
git commit -m "release: $Tag"

Write-Host "==> Tagging and pushing"
git tag $Tag
git push $Remote HEAD
git push $Remote $Tag

Write-Host ""
Write-Host "Release tag pushed: $Tag"
Write-Host "GitHub release workflow should start automatically."
