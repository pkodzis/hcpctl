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
    Write-Info "Downloading $BaseUrl/$Archive ..."
    Invoke-WebRequest -Uri "$BaseUrl/$Archive" -OutFile $Archive

    # Download checksums
    Write-Info "Downloading SHA256SUMS..."
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile "SHA256SUMS"

    # Verify checksum
    Write-Host "[INFO] Verifying checksum... " -ForegroundColor Green -NoNewline
    $ExpectedHash = (Get-Content SHA256SUMS | Where-Object { $_ -match $Archive } | ForEach-Object { ($_ -split '\s+')[0] })
    $ActualHash = (Get-FileHash -Path $Archive -Algorithm SHA256).Hash.ToLower()

    if ($ExpectedHash -ne $ActualHash) {
        Write-Host "FAILED" -ForegroundColor Red
        Write-Err "Checksum verification failed!`nExpected: $ExpectedHash`nActual: $ActualHash"
    }
    Write-Host "OK" -ForegroundColor Green

    # Try GPG verification if gpg is available
    $gpgAvailable = $false
    try {
        $null = Get-Command gpg -ErrorAction Stop
        $gpgAvailable = $true
    } catch {
        Write-Warn "GPG not installed, skipping signature verification (install: winget install GnuPG.Gpg4win)"
    }

    if ($gpgAvailable) {
        $sigDownloaded = $false
        $keyDownloaded = $false

        try {
            Write-Info "Downloading GPG signature..."
            Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS.sig" -OutFile "SHA256SUMS.sig" -ErrorAction Stop
            $sigDownloaded = $true
        } catch {
            Write-Warn "GPG signature not found for this release"
        }

        if ($sigDownloaded) {
            try {
                Write-Info "Downloading public key..."
                Invoke-WebRequest -Uri "https://raw.githubusercontent.com/$Repo/main/public-key.asc" -OutFile "public-key.asc" -ErrorAction Stop
                $keyDownloaded = $true
            } catch {
                Write-Warn "Public key not found, skipping GPG verification"
            }
        }

        if ($sigDownloaded -and $keyDownloaded) {
            # Import key and verify - gpg outputs to stderr even on success, so we capture all output
            $ErrorActionPreference = "Continue"

            Write-Host "[INFO] Importing GPG key... " -ForegroundColor Green -NoNewline
            $null = & gpg --batch --yes --import public-key.asc 2>&1
            if ($LASTEXITCODE -ne 0) {
                Write-Host "FAILED" -ForegroundColor Yellow
                Write-Warn "GPG key import failed - skipping signature verification"
            } else {
                Write-Host "OK" -ForegroundColor Green

                Write-Host "[INFO] Verifying GPG signature... " -ForegroundColor Green -NoNewline
                $null = & gpg --batch --verify SHA256SUMS.sig SHA256SUMS 2>&1
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "OK" -ForegroundColor Green
                } else {
                    Write-Host "FAILED" -ForegroundColor Yellow
                    Write-Warn "GPG signature verification failed - proceeding anyway (checksum passed)"
                }
            }

            $ErrorActionPreference = "Stop"
        }
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
