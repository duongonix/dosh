# ================== CONFIG ==================
$GITHUB_USER = "duongonix"
$REPO_NAME   = "dosh"
$ASSET_NAME  = "dosh-x86_64-pc-windows-msvc.exe"

$INSTALL_DIR = "$env:LOCALAPPDATA\dosh"
$BIN_PATH    = "$INSTALL_DIR\$ASSET_NAME"

$ICON_NAME = "dosh.ico"
$ICON_URL  = "https://raw.githubusercontent.com/$GITHUB_USER/$REPO_NAME/main/dosh.ico"
$ICON_PATH = "$INSTALL_DIR\$ICON_NAME"
# ============================================

Write-Host "🔹 Installing DoshShell..." -ForegroundColor Cyan

# 1. Check PowerShell version
if ($PSVersionTable.PSVersion.Major -lt 5) {
    Write-Error "PowerShell 5.0+ is required"
    exit 1
}

# 2. Create install dir
if (!(Test-Path $INSTALL_DIR)) {
    New-Item -ItemType Directory -Path $INSTALL_DIR | Out-Null
}

# 3. Get latest release
$apiUrl = "https://api.github.com/repos/$GITHUB_USER/$REPO_NAME/releases/latest"

try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{
        "User-Agent" = "PowerShell"
    }
}
catch {
    Write-Error "Failed to fetch GitHub release"
    exit 1
}

# 4. Find asset
$asset = $release.assets | Where-Object { $_.name -eq $ASSET_NAME }

if (-not $asset) {
    Write-Error "Asset '$ASSET_NAME' not found"
    exit 1
}

# 5. Download binary
Write-Host "⬇ Downloading $ASSET_NAME..."
Invoke-WebRequest `
    -Uri $asset.browser_download_url `
    -OutFile $BIN_PATH `
    -UseBasicParsing

# 6. Download icon (safe)
Write-Host "⬇ Downloading icon..."
try {
    if (!(Test-Path $ICON_PATH)) {
        Invoke-WebRequest `
            -Uri $ICON_URL `
            -OutFile $ICON_PATH `
            -TimeoutSec 10 `
            -ErrorAction Stop

        Write-Host "✅ Icon downloaded"
    }
    else {
        Write-Host "ℹ Icon already exists"
    }
}
catch {
    Write-Warning "⚠ Failed to download icon, skipping"
}

# 7. Add to PATH (User)
$envPath = [Environment]::GetEnvironmentVariable("PATH", "User")

if ($envPath -notlike "*$INSTALL_DIR*") {
    [Environment]::SetEnvironmentVariable(
        "PATH",
        "$envPath;$INSTALL_DIR",
        "User"
    )
    Write-Host "✅ Added to PATH"
}

# 8. Windows Terminal profile
$settingsPath = "$env:LOCALAPPDATA\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"

if (Test-Path $settingsPath) {
    Write-Host "🔧 Configuring Windows Terminal..."

    $json = Get-Content $settingsPath -Raw | ConvertFrom-Json

    $exists = $json.profiles.list | Where-Object { $_.name -eq "dosh" }

    if (-not $exists) {
        $iconForProfile = if (Test-Path $ICON_PATH) {
            $ICON_PATH
        } else {
            "ms-appx:///ProfileIcons/pwsh.png"
        }

        $profile = @{
            guid              = ([guid]::NewGuid()).ToString("B")
            name              = "DoshShell"
            commandline       = $BIN_PATH
            startingDirectory = "%USERPROFILE%"
            icon              = $iconForProfile
        }

        $json.profiles.list += $profile
        $json | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding UTF8

        Write-Host "✅ Windows Terminal profile added"
    }
    else {
        Write-Host "ℹ Windows Terminal profile already exists"
    }
}

# 9. Final check
if (Test-Path $BIN_PATH) {
    Write-Host ""
    Write-Host "🎉 DoshShell installed successfully!" -ForegroundColor Green
    Write-Host "👉 Restart terminal and run: dosh"
}
else {
    Write-Error "Installation failed"
}
