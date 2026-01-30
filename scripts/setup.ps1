# Arbitrage Dashboard Setup Script
# This script installs all prerequisites and sets up the development environment

param(
    [switch]$SkipRust,
    [switch]$SkipNode,
    [switch]$SkipGit
)

$ErrorActionPreference = "Stop"

# Colors for output
$Green = "`e[32m"
$Yellow = "`e[33m"
$Blue = "`e[34m"
$Red = "`e[31m"
$Cyan = "`e[36m"
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

function Write-Info($Message) {
    Write-Status $Message $Cyan
}

# Get the project root directory
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

Write-Info "========================================"
Write-Info "  Arbitrage Dashboard Setup"
Write-Info "========================================"
Write-Info ""

# ============================================================================
# 1. Check and Install Git
# ============================================================================
if (-not $SkipGit) {
    Write-Status "Checking Git installation..."
    if (Get-Command git -ErrorAction SilentlyContinue) {
        $GitVersion = git --version
        Write-Success "Git already installed: $GitVersion"
    } else {
        Write-Warning "Git not found. Installing Git..."
        try {
            # Download and install Git for Windows
            $GitInstaller = "$env:TEMP\GitInstaller.exe"
            Write-Status "Downloading Git installer..."
            Invoke-WebRequest -Uri "https://github.com/git-for-windows/git/releases/download/v2.43.0.windows.1/Git-2.43.0-64-bit.exe" -OutFile $GitInstaller
            
            Write-Status "Installing Git (this may take a few minutes)..."
            Start-Process -FilePath $GitInstaller -ArgumentList "/VERYSILENT", "/NORESTART", "/NOCANCEL" -Wait
            Remove-Item $GitInstaller -Force
            
            # Refresh PATH
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
            
            Write-Success "Git installed successfully"
        } catch {
            Write-Error "Failed to install Git: $_"
            Write-Info "Please install Git manually from: https://git-scm.com/download/win"
            exit 1
        }
    }
}

# ============================================================================
# 2. Check and Install Rust
# ============================================================================
if (-not $SkipRust) {
    Write-Status "Checking Rust installation..."
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        $RustVersion = rustc --version
        $CargoVersion = cargo --version
        Write-Success "Rust already installed:"
        Write-Success "  $RustVersion"
        Write-Success "  $CargoVersion"
    } else {
        Write-Warning "Rust not found. Installing Rust..."
        try {
            Write-Status "Downloading Rust installer..."
            $RustupInit = "$env:TEMP\rustup-init.exe"
            Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $RustupInit
            
            Write-Status "Installing Rust (this may take several minutes)..."
            Start-Process -FilePath $RustupInit -ArgumentList "-y", "--default-toolchain", "stable" -Wait
            Remove-Item $RustupInit -Force
            
            # Add cargo to PATH for current session
            $env:Path += ";$env:USERPROFILE\.cargo\bin"
            
            # Verify installation
            $RustVersion = rustc --version
            Write-Success "Rust installed successfully: $RustVersion"
        } catch {
            Write-Error "Failed to install Rust: $_"
            Write-Info "Please install Rust manually from: https://rustup.rs/"
            exit 1
        }
    }
    
    # Install required Rust components
    Write-Status "Installing Rust components..."
    rustup component add rustfmt clippy 2>$null
    Write-Success "Rust components installed"
}

# ============================================================================
# 3. Check and Install Node.js / Bun
# ============================================================================
if (-not $SkipNode) {
    Write-Status "Checking Node.js / Bun installation..."
    
    # Check for Bun first (preferred)
    if (Get-Command bun -ErrorAction SilentlyContinue) {
        $BunVersion = bun --version
        Write-Success "Bun already installed: v$BunVersion"
        $PackageManager = "bun"
    }
    # Check for Node.js/npm
    elseif (Get-Command node -ErrorAction SilentlyContinue) {
        $NodeVersion = node --version
        Write-Success "Node.js already installed: $NodeVersion"
        $PackageManager = "npm"
        
        # Check npm version
        $NpmVersion = npm --version
        Write-Success "npm version: $NpmVersion"
    } else {
        Write-Warning "Neither Bun nor Node.js found."
        Write-Info "Would you like to install:"
        Write-Info "  1. Bun (faster, recommended)"
        Write-Info "  2. Node.js LTS"
        Write-Info "  3. Skip (install manually later)"
        
        $Choice = Read-Host "Enter your choice (1-3)"
        
        switch ($Choice) {
            "1" {
                Write-Status "Installing Bun..."
                try {
                    # Install Bun using PowerShell
                    powershell -c "irm bun.sh/install.ps1 | iex"
                    
                    # Add to PATH
                    $env:Path += ";$env:USERPROFILE\.bun\bin"
                    
                    $BunVersion = bun --version
                    Write-Success "Bun installed successfully: v$BunVersion"
                    $PackageManager = "bun"
                } catch {
                    Write-Error "Failed to install Bun: $_"
                    Write-Info "Falling back to Node.js..."
                    $Choice = "2"
                }
            }
            
            "2" {
                Write-Status "Installing Node.js LTS..."
                try {
                    # Download Node.js LTS installer
                    $NodeInstaller = "$env:TEMP\node-installer.msi"
                    Write-Status "Downloading Node.js installer..."
                    Invoke-WebRequest -Uri "https://nodejs.org/dist/v20.11.0/node-v20.11.0-x64.msi" -OutFile $NodeInstaller
                    
                    Write-Status "Installing Node.js (this may take a few minutes)..."
                    Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", $NodeInstaller, "/qn", "/norestart" -Wait
                    Remove-Item $NodeInstaller -Force
                    
                    # Refresh PATH
                    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
                    
                    $NodeVersion = node --version
                    Write-Success "Node.js installed successfully: $NodeVersion"
                    $PackageManager = "npm"
                } catch {
                    Write-Error "Failed to install Node.js: $_"
                    Write-Info "Please install Node.js manually from: https://nodejs.org/"
                    exit 1
                }
            }
            
            "3" {
                Write-Warning "Skipping Node.js/Bun installation"
                Write-Info "You will need to install Node.js or Bun manually to run the frontend"
                $PackageManager = $null
            }
            
            default {
                Write-Error "Invalid choice"
                exit 1
            }
        }
    }
}

# ============================================================================
# 4. Setup Project
# ============================================================================
Write-Info ""
Write-Info "========================================"
Write-Info "  Setting up project..."
Write-Info "========================================"

# Create config from example if needed
Write-Status "Checking configuration..."
if (-not (Test-Path "config/config.toml")) {
    if (Test-Path "config/config.example.toml") {
        Write-Status "Creating config from example..."
        Copy-Item "config/config.example.toml" "config/config.toml"
        Write-Success "Config created at config/config.toml"
        Write-Warning "IMPORTANT: Please edit config/config.toml with your API credentials"
    } else {
        Write-Warning "No config example found. You will need to create config/config.toml manually"
    }
} else {
    Write-Success "Config already exists"
}

# Install frontend dependencies
if ($PackageManager) {
    Write-Status "Installing frontend dependencies..."
    Set-Location "$ProjectRoot\frontend"
    
    try {
        if ($PackageManager -eq "bun") {
            bun install
        } else {
            npm install
        }
        Write-Success "Frontend dependencies installed"
    } catch {
        Write-Error "Failed to install frontend dependencies: $_"
    }
    
    Set-Location $ProjectRoot
}

# Build Rust project
Write-Status "Building Rust project (first build may take several minutes)..."
try {
    cargo build
    Write-Success "Rust project built successfully"
} catch {
    Write-Error "Failed to build Rust project: $_"
    Write-Info "You can try building manually with: cargo build"
}

# ============================================================================
# 5. Summary
# ============================================================================
Write-Info ""
Write-Info "========================================"
Write-Info "  Setup Complete!"
Write-Info "========================================"
Write-Info ""
Write-Success "All prerequisites installed and project configured!"
Write-Info ""
Write-Info "Next steps:"
Write-Info "  1. Edit config/config.toml with your API credentials"
Write-Info "  2. Run the development server:"
Write-Info "     .\scripts\start-dev.ps1"
Write-Info ""
Write-Info "Available commands:"
Write-Info "  .\scripts\start-dev.ps1     - Start backend + frontend"
Write-Info "  cargo run --bin arbitrage-server  - Start backend only"
Write-Info "  cd frontend && $PackageManager run dev  - Start frontend only"
Write-Info ""
Write-Info "========================================"
