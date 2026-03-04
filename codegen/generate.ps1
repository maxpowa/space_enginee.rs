<#
.SYNOPSIS
    Builds and runs the StandaloneExtractor codegen tool to generate .rs files for the crate.
.DESCRIPTION
    This script builds the C# codegen project, then runs it against Space Engineers to
    reflect over game assemblies and generate Rust struct/enum definitions for the
    space_engineers crate.

    The Bin64 directory is auto-detected from the Steam registry. Override with -Bin64.
.PARAMETER Bin64
    Path to the Space Engineers Bin64 directory. Auto-detected from Steam if not provided.
.EXAMPLE
    .\codegen\generate.ps1
.EXAMPLE
    .\codegen\generate.ps1 -Bin64 "D:\Steam\steamapps\common\SpaceEngineers\Bin64"
#>
param(
    [string]$Bin64
)

$ErrorActionPreference = "Stop"

$codegenDir = $PSScriptRoot
$repoRoot   = Split-Path $codegenDir -Parent
$sysDir     = Join-Path $repoRoot "crate-sys" "src"
$csproj     = Join-Path $codegenDir "StandaloneExtractor.csproj"

# ── Build ────────────────────────────────────────────────────────────────────
Write-Host "Building StandaloneExtractor..." -ForegroundColor Cyan
dotnet build $csproj -c Debug
if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE" }

# ── Locate executable ────────────────────────────────────────────────────────
$exe = Join-Path $codegenDir "bin" "Debug" "netframework48" "StandaloneExtractor.exe"
if (-not (Test-Path $exe)) {
    throw "Build output not found: $exe"
}

# ── Assemble arguments ───────────────────────────────────────────────────────
$runArgs = @("--output", $sysDir)
if ($Bin64) {
    $runArgs += @("--bin64", $Bin64)
}

# ── Run ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Running StandaloneExtractor..." -ForegroundColor Cyan
Write-Host "  Executable : $exe"                -ForegroundColor Gray
Write-Host "  RS output  : $sysDir"           -ForegroundColor Gray
if ($Bin64) {
    Write-Host "  Bin64      : $Bin64"          -ForegroundColor Gray
}
Write-Host ""

& $exe @runArgs

if ($LASTEXITCODE -ne 0) {
    throw "Code generation failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Done! Generated .rs files written to $sysDir" -ForegroundColor Green
