# Build script for Windows PowerShell
# Run this script to build the entire project

Write-Host "Building Arbitrage Dashboard..." -ForegroundColor Green

# Check if Rust is installed
if (!(Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Rust/Cargo not found. Please install Rust from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Create necessary directories
Write-Host "Creating directories..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path "data" | Out-Null
New-Item -ItemType Directory -Force -Path "logs" | Out-Null
New-Item -ItemType Directory -Force -Path "config" | Out-Null

# Copy example config if config doesn't exist
if (!(Test-Path "config/config.toml")) {
    Write-Host "Creating default configuration..." -ForegroundColor Yellow
    Copy-Item "config/config.example.toml" "config/config.toml"
}

# Build the project
Write-Host "Building Rust project..." -ForegroundColor Yellow
cargo build --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build completed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "To run the server:" -ForegroundColor Cyan
    Write-Host "  cargo run --bin arbitrage-server" -ForegroundColor White
    Write-Host ""
    Write-Host "To run tests:" -ForegroundColor Cyan
    Write-Host "  cargo test" -ForegroundColor White
    Write-Host ""
    Write-Host "Configuration file: config/config.toml" -ForegroundColor Cyan
} else {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}