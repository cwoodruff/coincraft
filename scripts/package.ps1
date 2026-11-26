# Builds CoinCraft installers for Windows (setup .exe and .msi).
# Run in PowerShell from anywhere: .\scripts\package.ps1   The results go in dist\.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust isn't installed. Get it from https://rustup.rs and run this again."
}

cargo packager --version *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing cargo-packager (first time only)..."
    cargo install cargo-packager --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

cargo packager --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "Done. Installers are in: $(Join-Path (Get-Location) 'dist')"
Get-ChildItem dist -File | Select-Object -ExpandProperty Name
