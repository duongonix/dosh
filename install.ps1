# ================== CONFIG ==================
$GITHUB_USER = "duongonix"
$REPO_NAME = "dosh"
$ASSET_NAME = "dosh-x86_64-pc-windows-msvc.exe"   # tên file trong GitHub Release

$INSTALL_DIR = "$env:LOCALAPPDATA\dosh"
$BIN_PATH = "$INSTALL_DIR\$ASSET_NAME"

$ICON_NAME = "dosh.ico"
$ICON_URL  = "https://raw.githubusercontent.com/$GITHUB_USER/$REPO_NAME/main/dosh.ico"
$ICON_PATH = "$INSTALL_DIR\$ICON_NAME"


# ============================================

Write-Host "🔹 Installing DoshShell..." -ForegroundColor Cyan

# 1. Kiểm tra PowerShell version
if ($PSVersionTable.PSVersion.Major -lt 5) {
    Write-Error "PowerShell 5.0+ is required."
    exit 1
}

# 2. Tạo thư mục cài đặt
if (!(Test-Path $INSTALL_DIR)) {
    New-Item -ItemType Directory -Path $INSTALL_DIR | Out-Null
}

# 3. Lấy thông tin release mới nhất
$apiUrl = "https://api.github.com/repos/$GITHUB_USER/$REPO_NAME/releases/latest"

try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "PowerShell" }
}
catch {
    Write-Error "Failed to fetch GitHub release."
    exit 1
}

# 4. Tìm asset exe
$asset = $release.assets | Where-Object { $_.name -eq $ASSET_NAME }

if (-not $asset) {
    Write-Error "Release asset '$ASSET_NAME' not found."
    exit 1
}

# 5. Tải file exe
Write-Host "⬇ Downloading $ASSET_NAME..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $BIN_PATH

# 6. Thêm vào PATH (User)
$envPath = [Environment]::GetEnvironmentVariable("PATH", "User")

if ($envPath -notlike "*$INSTALL_DIR*") {
    [Environment]::SetEnvironmentVariable(
        "PATH",
        "$envPath;$INSTALL_DIR",
        "User"
    )
    Write-Host "✅ Added to PATH"
}


# 7. Kiểm tra cài đặt
if (Test-Path $BIN_PATH) {
    Write-Host "🎉 Installation completed!"
    Write-Host "👉 Restart terminal and run: dosh"

    $settingsPath = "$env:LOCALAPPDATA\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"

    if (-not (Test-Path $settingsPath)) {
        Write-Error "Không tìm thấy settings.json của Windows Terminal"
        exit 1
    }

    $json = Get-Content $settingsPath -Raw | ConvertFrom-Json

    # ID cố định cho dosh (không đổi mỗi lần)
    $doshGuid = [guid]::NewGuid().ToString()

    # Kiểm tra profile đã tồn tại chưa
    $exists = $json.profiles.list | Where-Object { $_.guid -eq $doshGuid }

    if ($exists) {
        Write-Host "Profile dosh đã tồn tại, không cần thêm"
        exit 0
    }

    $path = "$env:LOCALAPPDATA\dosh"


    # Thêm profile mới
    $doshProfile = @{
        guid              = $doshGuid
        name              = "dosh"
        commandline       = "$path\dosh-x86_64-pc-windows-msvc.exe"
        startingDirectory = "%USERPROFILE%"
        icon              = "$path\dosh.ico"
    }

    $json.profiles.list += $doshProfile

    # Ghi lại file
    $json | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding UTF8

    Write-Host "Đã thêm profile DoshShell vào Windows Terminal"

}
else {
    Write-Error "Installation failed."
}
