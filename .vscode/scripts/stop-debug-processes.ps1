param(
    [string]$Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

$ErrorActionPreference = "Stop"

$workspaceRoot = (Resolve-Path $Workspace).Path
$targetRoot = Join-Path $workspaceRoot "target"
$processNames = @("tauri_desktop", "egui_overlay")
$processes = Get-Process -Name $processNames -ErrorAction SilentlyContinue
$stopped = 0

foreach ($process in $processes) {
    $processPath = $null
    try {
        $processPath = $process.Path
    } catch {
        $processPath = $null
    }

    if (-not $processPath) {
        Write-Host "Skipping $($process.ProcessName) pid=$($process.Id): process path unavailable"
        continue
    }

    if (-not $processPath.StartsWith($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Write-Host "Skipping $($process.ProcessName) pid=$($process.Id): outside workspace target"
        continue
    }

    Write-Host "Stopping $($process.ProcessName) pid=$($process.Id) path=$processPath"
    Stop-Process -Id $process.Id -Force
    $stopped += 1
}

Write-Host "Stopped $stopped Snap Pin debug process(es)."
