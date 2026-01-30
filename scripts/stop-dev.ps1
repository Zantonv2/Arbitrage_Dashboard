# Arbitrage Dashboard Stop Script
# This script stops all running development servers

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

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$PidFile = "$ProjectRoot\.running-pids"

Write-Status "Stopping Arbitrage Dashboard servers..."

# Try to read PIDs from file
if (Test-Path $PidFile) {
    $ProcessIds = Get-Content $PidFile
    foreach ($ProcessId in $ProcessIds) {
        if ($ProcessId -match '^\d+$') {
            try {
                $Process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
                if ($Process) {
                    Stop-Process -Id $ProcessId -Force
                    Write-Success "Stopped process $ProcessId ($($Process.ProcessName))"
                }
            } catch {
                Write-Warning "Could not stop process ${ProcessId}: $_"
            }
        }
    }
    Remove-Item $PidFile -Force
    Write-Success "PID file cleaned up"
}

# Also try to find and stop by process names
$ProcessNames = @("arbitrage-server", "node", "bun")
foreach ($Name in $ProcessNames) {
    $Processes = Get-Process -Name $Name -ErrorAction SilentlyContinue
    foreach ($Process in $Processes) {
        try {
            # Check if it's running from our project directory
            if ($Process.Path -and $Process.Path.StartsWith($ProjectRoot)) {
                Stop-Process -Id $Process.Id -Force
                Write-Success "Stopped $Name process $($Process.Id)"
            }
        } catch {
            # Ignore errors
        }
    }
}

Write-Success "All servers stopped!"
