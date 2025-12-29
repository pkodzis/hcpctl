# hcpctl installer for Windows
# Usage: irm https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "pkodzis/hcpctl"
$BinaryName = "hcpctl"
$InstallDir = "$env:LOCALAPPDATA\hcpctl"

function Write-Info { param($msg) Write-Host "[INFO] $msg" -ForegroundColor Green }
function Write-Warn { param($msg) Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err { param($msg) Write-Host "[ERROR] $msg" -ForegroundColor Red; exit 1 }

# Get latest version
Write-Info "Fetching latest version..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name
Write-Info "Latest version: $Version"

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "amd64" } else { Write-Err "32-bit Windows not supported" }
$Platform = "windows_$Arch"
Write-Info "Detected platform: $Platform"

# Create temp directory
$TempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
Push-Location $TempDir

try {
    $BaseUrl = "https://github.com/$Repo/releases/download/$Version"
    $Archive = "${BinaryName}_${Version}_${Platform}.zip"

    # Download archive
    Write-Info "Downloading $Archive..."
    Invoke-WebRequest -Uri "$BaseUrl/$Archive" -OutFile $Archive

    # Download checksums
    Write-Info "Downloading SHA256SUMS..."
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile "SHA256SUMS"

    # Verify checksum
    Write-Info "Verifying checksum..."
    $ExpectedHash = (Get-Content SHA256SUMS | Where-Object { $_ -match $Archive } | ForEach-Object { ($_ -split '\s+')[0] })
    $ActualHash = (Get-FileHash -Path $Archive -Algorithm SHA256).Hash.ToLower()

    if ($ExpectedHash -ne $ActualHash) {
        Write-Err "Checksum verification failed!`nExpected: $ExpectedHash`nActual: $ActualHash"
    }
    Write-Info "Checksum verified!"

    # Try GPG verification if gpg is available
    try {
        $null = Get-Command gpg -ErrorAction Stop

        Write-Info "Downloading GPG signature..."
        Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS.sig" -OutFile "SHA256SUMS.sig" -ErrorAction Stop

        Write-Info "Downloading public key..."
        Invoke-WebRequest -Uri "https://raw.githubusercontent.com/$Repo/main/public-key.asc" -OutFile "public-key.asc" -ErrorAction Stop

        & gpg --import public-key.asc 2>$null
        $gpgResult = & gpg --verify SHA256SUMS.sig SHA256SUMS 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Info "GPG signature verified!"
        } else {
            Write-Warn "GPG signature verification failed - proceeding anyway (checksum passed)"
        }
    } catch {
        Write-Warn "GPG not available or signature not found, skipping GPG verification"
    }

    # Extract
    Write-Info "Extracting..."
    Expand-Archive -Path $Archive -DestinationPath "." -Force

    # Install
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Move-Item -Path "$BinaryName.exe" -Destination "$InstallDir\$BinaryName.exe" -Force

    Write-Info "Installed to: $InstallDir\$BinaryName.exe"

    # Check PATH
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Warn "$InstallDir is not in your PATH"

        $AddToPath = Read-Host "Add to PATH? (y/n)"
        if ($AddToPath -eq 'y') {
            [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
            Write-Info "Added to PATH. Restart your terminal to use '$BinaryName'"
        } else {
            Write-Host ""
            Write-Host "To add manually, run:"
            Write-Host "  `$env:Path += `";$InstallDir`""
            Write-Host ""
        }
    }

    Write-Info "Installation complete! Run '$BinaryName --version' to verify."

} finally {
    Pop-Location
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
