# Generate DepotDownloader config for GitHub Actions
# This script authenticates with Steam and creates a base64-encoded config
# that can be used as the DEPOT_DOWNLOADER_CONFIG secret.
#
# Usage: .\generate-config.ps1 -Username <steam_username>

param(
    [Parameter(Mandatory=$true)]
    [string]$Username,

    [Parameter(Mandatory=$false)]
    [string]$OutputFile = "depot-downloader-config.txt"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  DepotDownloader Config Generator" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Download DepotDownloader if needed
$ddPath = "$PSScriptRoot\depotdownloader-temp"
$ddExe = "$ddPath\DepotDownloader.exe"

if (!(Test-Path $ddExe)) {
    Write-Host "Downloading DepotDownloader..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $ddPath | Out-Null

    $ProgressPreference = 'SilentlyContinue'
    $releasesUrl = "https://api.github.com/repos/SteamRE/DepotDownloader/releases/latest"
    $releaseInfo = Invoke-RestMethod -Uri $releasesUrl
    $assetUrl = $releaseInfo.assets | Where-Object { $_.name -like "*windows-x64*" } | Select-Object -First 1 -ExpandProperty browser_download_url

    if (-not $assetUrl) {
        Write-Error "Failed to find DepotDownloader download URL"
        exit 1
    }

    Invoke-WebRequest -Uri $assetUrl -OutFile "$ddPath\depotdownloader.zip"
    Expand-Archive -Path "$ddPath\depotdownloader.zip" -DestinationPath $ddPath -Force
    Remove-Item "$ddPath\depotdownloader.zip"
    Write-Host "DepotDownloader downloaded to $ddPath" -ForegroundColor Green
} else {
    Write-Host "Using existing DepotDownloader at $ddPath" -ForegroundColor Green
}

$isolatedStoragePath = "$env:LOCALAPPDATA\IsolatedStorage"

# Step 2: Run DepotDownloader to authenticate
Write-Host ""
Write-Host "Running DepotDownloader to authenticate..." -ForegroundColor Yellow
Write-Host "You will be prompted for your password and possibly a 2FA code." -ForegroundColor Yellow
Write-Host ""

# Create a minimal filelist (we just want to authenticate, not download much)
$filelistPath = "$ddPath\filelist.txt"
"regex:^thiswontmatchanything$" | Out-File -FilePath $filelistPath -Encoding utf8

$ddArgs = @(
    "-app", "244850",
    "-depot", "244851",
    "-username", $Username,
    "-remember-password",
    "-filelist", $filelistPath,
    "-dir", "$ddPath\temp-download"
)

& $ddExe @ddArgs

if ($LASTEXITCODE -ne 0) {
    Write-Error "DepotDownloader failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Authentication successful!" -ForegroundColor Green

# Step 3: Find the AssemFiles folder with the config
Write-Host ""
Write-Host "Finding DepotDownloader's config folder..." -ForegroundColor Yellow

if (!(Test-Path $isolatedStoragePath)) {
    Write-Error "No IsolatedStorage directory found after authentication"
    exit 1
}

# Step 4: Create zip with just the config files (not the folder structure)
Write-Host ""
Write-Host "Creating config archive..." -ForegroundColor Yellow

# Find the AssemFiles folder (contains the actual config)
$assemFiles = Get-ChildItem -Path $isolatedStoragePath -Recurse -Directory -Filter "AssemFiles" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if (-not $assemFiles) {
    Write-Error "Could not find AssemFiles folder"
    exit 1
}

Write-Host "Found config at: $($assemFiles.FullName)" -ForegroundColor Green

$zipPath = "$ddPath\dd-config.zip"
if (Test-Path $zipPath) { Remove-Item $zipPath }

# Zip just the contents of AssemFiles (not the folder structure)
Compress-Archive -Path "$($assemFiles.FullName)\*" -DestinationPath $zipPath

# Step 5: Base64 encode
Write-Host "Base64 encoding..." -ForegroundColor Yellow
$base64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($zipPath))

# Step 6: Save to file
$base64 | Out-File -FilePath $OutputFile -Encoding utf8 -NoNewline
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  Config generated successfully!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Output saved to: $OutputFile" -ForegroundColor Cyan
Write-Host "Base64 length: $($base64.Length) characters" -ForegroundColor Gray
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Copy the contents of $OutputFile" -ForegroundColor White
Write-Host "2. Go to your GitHub repo -> Settings -> Secrets -> Actions" -ForegroundColor White
Write-Host "3. Create a new secret named: DEPOT_DOWNLOADER_CONFIG" -ForegroundColor White
Write-Host "4. Paste the base64 content as the value" -ForegroundColor White
Write-Host ""

# Cleanup
Remove-Item $zipPath -ErrorAction SilentlyContinue
Remove-Item "$ddPath\temp-download" -Recurse -Force -ErrorAction SilentlyContinue

# Optionally copy to clipboard
$copyToClipboard = Read-Host "Copy to clipboard? (y/n)"
if ($copyToClipboard -eq "y") {
    $base64 | Set-Clipboard
    Write-Host "Copied to clipboard!" -ForegroundColor Green
}

