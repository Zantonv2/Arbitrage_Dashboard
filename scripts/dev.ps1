# Development script for Windows PowerShell
# Run this script to start the development server with hot reload

Write-Host "Starting Arbitrage Dashboard in development mode..." -ForegroundColor Green

# Check if cargo-watch is installed
if (!(Get-Command "cargo-watch" -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-watch for hot reload..." -ForegroundColor Yellow
    cargo install cargo-watch
}

# Create necessary directories
New-Item -ItemType Directory -Force -Path "data" | Out-Null
New-Item -ItemType Directory -Force -Path "logs" | Out-Null
New-Item -ItemType Directory -Force -Path "config" | Out-Null

# Copy example config if config doesn't exist
if (!(Test-Path "config/config.toml")) {
    Write-Host "Creating default configuration..." -ForegroundColor Yellow
    Copy-Item "config/config.example.toml" "config/config.toml"
}

Write-Host "Starting development server with hot reload..." -ForegroundColor Yellow
Write-Host "Server will restart automatically when code changes" -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop" -ForegroundColor Cyan
Write-Host ""

# Start with hot reload
cargo watch -x "run --bin arbitrage-server"