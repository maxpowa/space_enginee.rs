# GitHub action powershell script to setup DepotDownloader and download Space Engineers

Write-Output ""
Write-Output "#################################"
Write-Output "#   Downloading DepotDownloader #"
Write-Output "#################################"
Write-Output ""

$DepotDownloaderPath = "$PWD\depotdownloader"

if (!(Test-Path "$DepotDownloaderPath\DepotDownloader.exe")) {
    New-Item -ItemType Directory -Force -Path $DepotDownloaderPath | Out-Null

    # Get the latest release URL for windows-x64
    $ProgressPreference = 'SilentlyContinue'
    $ReleasesUrl = "https://api.github.com/repos/SteamRE/DepotDownloader/releases/latest"
    $ReleaseInfo = Invoke-RestMethod -Uri $ReleasesUrl
    $AssetUrl = $ReleaseInfo.assets | Where-Object { $_.name -like "*windows-x64*" } | Select-Object -First 1 -ExpandProperty browser_download_url

    if (-not $AssetUrl) {
        Write-Error "Failed to find DepotDownloader windows-x64 asset"
        exit 1
    }

    Write-Output "Downloading from: $AssetUrl"
    Invoke-WebRequest -Uri $AssetUrl -OutFile "$DepotDownloaderPath\depotdownloader.zip"
    Expand-Archive -Path "$DepotDownloaderPath\depotdownloader.zip" -DestinationPath $DepotDownloaderPath -Force
    Remove-Item "$DepotDownloaderPath\depotdownloader.zip"
}

$DepotDownloaderExe = "$DepotDownloaderPath\DepotDownloader.exe"

Write-Output ""
Write-Output "#################################"
Write-Output "#       Downloading Game        #"
Write-Output "#################################"
Write-Output ""

# Use full game (244850) - requires Steam authentication
# https://steamdb.info/app/244850/depots/
$SteamAppId = if ($env:STEAM_APP_ID) { $env:STEAM_APP_ID } else { "244850" }
$SteamDepotId = if ($env:STEAM_DEPOT_ID) { $env:STEAM_DEPOT_ID } else { "244851" }
$SteamGamePath = if ($env:STEAM_GAME_PATH) { $env:STEAM_GAME_PATH } else { "$PWD\game" }
$SteamUsername = if ($env:STEAM_USERNAME) { $env:STEAM_USERNAME } else { "" }
$SteamPassword = if ($env:STEAM_PASSWORD) { $env:STEAM_PASSWORD } else { "" }

Write-Output "App ID: $SteamAppId"
Write-Output "Depot ID: $SteamDepotId"
Write-Output "Install path: $SteamGamePath"

if (-not $SteamUsername) {
    Write-Error "STEAM_USERNAME is required for downloading the paid game"
    exit 1
}

# Check if login tokens are available (restored from DEPOT_DOWNLOADER_CONFIG secret)
$isolatedStoragePath = "$env:LOCALAPPDATA\IsolatedStorage"
if (Test-Path $isolatedStoragePath) {
    $fileCount = (Get-ChildItem -Path $isolatedStoragePath -Recurse -File -ErrorAction SilentlyContinue).Count
    if ($fileCount -gt 0) {
        Write-Output "Found login tokens in IsolatedStorage ($fileCount files)"
    }
} else {
    Write-Output "No login tokens found - password required"
}

# Create filelist to download only Bin64 folder (contains game binaries needed for extraction)
$FilelistPath = "$PWD\filelist.txt"
@"
regex:^Bin64/
"@ | Out-File -FilePath $FilelistPath -Encoding utf8

Write-Output "Using filelist to download only Bin64 folder"

# Run DepotDownloader
# -app: Steam App ID
# -depot: Steam Depot ID
# -username/-password: Steam credentials
# -remember-password: Save login tokens for future runs (avoids 2FA prompts)
# -filelist: Only download files matching patterns in the filelist
# -dir: Output directory
$DepotDownloaderArgs = @(
    "-app", $SteamAppId,
    "-depot", $SteamDepotId,
    "-username", $SteamUsername,
    "-remember-password",
    "-filelist", $FilelistPath,
    "-dir", $SteamGamePath
)

# Only pass password if we don't have cached tokens (or password is provided anyway)
if ($SteamPassword) {
    $DepotDownloaderArgs += @("-password", $SteamPassword)
}

Write-Output "Running: DepotDownloader.exe -app $SteamAppId -depot $SteamDepotId -username *** -remember-password -filelist $FilelistPath -dir $SteamGamePath"
& $DepotDownloaderExe @DepotDownloaderArgs

if ($LASTEXITCODE -eq 0) {
    Write-Output ""
    Write-Output "#################################"
    Write-Output "#       Download Success        #"
    Write-Output "#################################"
    Write-Output ""

    # List downloaded files for verification
    $bin64Path = Join-Path $SteamGamePath "Bin64"
    if (Test-Path $bin64Path) {
        $fileCount = (Get-ChildItem -Path $bin64Path -Recurse -File).Count
        Write-Output "Downloaded $fileCount files to Bin64"
    }
} else {
    Write-Output ""
    Write-Output "#################################"
    Write-Output "#       Download Failed         #"
    Write-Output "#################################"
    Write-Output ""
    Write-Output "Exit code: $LASTEXITCODE"
    exit $LASTEXITCODE
}







