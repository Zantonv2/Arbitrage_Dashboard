# Arbitrage Dashboard Development Startup Script
# This script starts both the Rust backend server and the Svelte frontend
# Usage: .\scripts\start-dev.ps1
# The script will exit after starting both services

param(
    [switch]$SkipBuild,
    [switch]$Release,
    [string]$ConfigPath = "config/config.toml"
)

$ErrorActionPreference = "Stop"

# Colors for output
$Green = "`e[32m"
$Yellow = "`e[33m"
$Blue = "`e[34m"
$Red = "`e[31m"
$Reset = "`e[0m"

function Write-Status($Message, $Color = $Blue) {
    Write-Host "$Color[$(Get-Date -Format 'HH:mm:ss')] $Message$Reset"
}

function Write-Success($Message) {
    Write-Status $Message $Green
}

function Write-Warning($Message) {
    Write-Status $Message $Yellow
}

function Write-Error($Message) {
    Write-Status $Message $Red
}

# Get the project root directory
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

Write-Status "Starting Arbitrage Dashboard Development Environment..."
Write-Status "Project Root: $ProjectRoot"

# Check prerequisites
Write-Status "Checking prerequisites..."

# Check Rust/Cargo
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Rust/Cargo not found. Please run .\scripts\setup.ps1 first"
    exit 1
}

# Check Node.js/Bun
if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    Write-Warning "Bun not found. Checking for npm..."
    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        Write-Error "Neither Bun nor npm found. Please run .\scripts\setup.ps1 first"
        exit 1
    }
    $PackageManager = "npm"
    $RunCmd = "npm run"
} else {
    $PackageManager = "bun"
    $RunCmd = "bun run"
}

Write-Success "Using package manager: $PackageManager"

# Check if config exists
if (-not (Test-Path $ConfigPath)) {
    Write-Warning "Config file not found at $ConfigPath"
    if (Test-Path "config/config.example.toml") {
        Write-Status "Creating config from example..."
        Copy-Item "config/config.example.toml" $ConfigPath
        Write-Success "Config created at $ConfigPath"
        Write-Warning "IMPORTANT: Please edit config/config.toml with your API credentials before continuing"
        Write-Status "Press any key to continue..."
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    } else {
        Write-Error "No config file found. Please create config/config.toml"
        exit 1
    }
}

# Build backend if needed
if (-not $SkipBuild) {
    Write-Status "Building Rust backend..."
    try {
        if ($Release) {
            cargo build --release
        } else {
            cargo build
        }
        Write-Success "Backend build completed"
    } catch {
        Write-Error "Backend build failed: $_"
        exit 1
    }
} else {
    Write-Warning "Skipping backend build (--SkipBuild specified)"
}

# Install frontend dependencies if needed
Write-Status "Checking frontend dependencies..."
Set-Location "$ProjectRoot\frontend"
if (-not (Test-Path "node_modules")) {
    Write-Status "Installing frontend dependencies..."
    if ($PackageManager -eq "bun") {
        bun install
    } else {
        npm install
    }
    Write-Success "Frontend dependencies installed"
}
Set-Location $ProjectRoot

# Start Backend in new window
Write-Status "Starting Rust backend server in new window..."
$BackendCmd = if ($Release) { 
    ".\target\release\arbitrage-server.exe" 
} else { 
    ".\target\debug\arbitrage-server.exe" 
}

if (-not (Test-Path $BackendCmd)) {
    Write-Error "Backend executable not found at $BackendCmd. Please build first."
    exit 1
}

$BackendEnv = @{
    "RUST_LOG" = "info"
    "CONFIG_PATH" = $ConfigPath
}

# Build environment variables string
$EnvVarsString = ""
foreach ($key in $BackendEnv.Keys) {
    $EnvVarsString += "`$env:$key = '$($BackendEnv[$key])'; "
}

# Start backend in minimized window
$BackendProcess = Start-Process -FilePath "powershell.exe" -ArgumentList "-NoExit", "-Command", "$EnvVarsString cd '$ProjectRoot'; & '$BackendCmd'" -WindowStyle Minimized -PassThru

Write-Success "Backend server started (PID: $($BackendProcess.Id))"

# Wait a moment for backend to initialize
Start-Sleep -Seconds 3

# Start Frontend in new window
Write-Status "Starting Svelte frontend dev server in new window..."

$FrontendCmd = if ($PackageManager -eq "bun") {
    "bun run dev"
} else {
    "npm run dev"
}

$FrontendProcess = Start-Process -FilePath "powershell.exe" -ArgumentList "-NoExit", "-Command", "cd '$ProjectRoot\frontend'; $FrontendCmd" -WindowStyle Minimized -PassThru

Write-Success "Frontend dev server started (PID: $($FrontendProcess.Id))"

# Display status
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  Arbitrage Dashboard is starting!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Backend API:  " -NoNewline; Write-Host "http://localhost:8080" -ForegroundColor Green
Write-Host "  Frontend:     " -NoNewline; Write-Host "http://localhost:5173" -ForegroundColor Green
Write-Host "  WebSocket:    " -NoNewline; Write-Host "ws://localhost:8080/ws" -ForegroundColor Green
Write-Host ""
Write-Host "  Backend PID:  " -NoNewline; Write-Host "$($BackendProcess.Id)" -ForegroundColor Yellow
Write-Host "  Frontend PID: " -NoNewline; Write-Host "$($FrontendProcess.Id)" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Both servers are running in separate windows" -ForegroundColor Yellow
Write-Host "  Close those windows to stop the servers" -ForegroundColor Yellow
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Save PIDs to file for easy shutdown
$PidFile = "$ProjectRoot\.running-pids"
"$($BackendProcess.Id)`n$($FrontendProcess.Id)" | Out-File -FilePath $PidFile -Encoding UTF8

Write-Success "Setup complete! Opening browser..."

# Wait a moment for servers to be ready
Start-Sleep -Seconds 2

# Open browser
Start-Process "http://localhost:5173"

# Exit the script (PowerShell window closes)
exit 0
