$ErrorActionPreference = 'Stop'

function Add-EncodedRustFlag {
    param(
        [string]$Flag
    )

    $separator = [char]0x1f
    $current = $env:CARGO_ENCODED_RUSTFLAGS
    if ($current -and $current.Split($separator) -contains $Flag) {
        return
    }

    if ($current) {
        $env:CARGO_ENCODED_RUSTFLAGS = "$current$separator$Flag"
    }
    else {
        $env:CARGO_ENCODED_RUSTFLAGS = $Flag
    }
}

$workspaceRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$realUserProfile = $env:USERPROFILE
$tempRoot = Join-Path $workspaceRoot '.vscode\.temp'
$userProfile = Join-Path $tempRoot 'userprofile'
$localAppData = Join-Path $tempRoot 'localappdata'
$appData = Join-Path $tempRoot 'appdata'
$installerRunId = Get-Date -Format 'yyyyMMdd-HHmmss'
$installerTargetDir = Join-Path $workspaceRoot "target\installer-runs\$installerRunId"

New-Item -ItemType Directory -Force $userProfile, $localAppData, $appData | Out-Null
New-Item -ItemType Directory -Force $installerTargetDir | Out-Null

$env:USERPROFILE = $userProfile
$env:LOCALAPPDATA = $localAppData
$env:APPDATA = $appData
$env:RUSTUP_HOME = Join-Path $realUserProfile '.rustup'
$env:CARGO_HOME = Join-Path $realUserProfile '.cargo'
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'
$env:CARGO_TARGET_DIR = $installerTargetDir
$env:CARGO_ENCODED_RUSTFLAGS = $null
$env:RUSTFLAGS = $null
Add-EncodedRustFlag -Flag "-Ctarget-feature=+crt-static"

$workspaceTargetDir = Join-Path $workspaceRoot 'target'
$overlayReleaseDir = Join-Path $workspaceTargetDir 'release'
$overlaySource = Join-Path $overlayReleaseDir 'egui_overlay.exe'
$rustHost = (& rustc -vV | Select-String '^host:' | ForEach-Object { $_.ToString().Split(':', 2)[1].Trim() })
$overlayPackagedName = if ($rustHost) { "egui_overlay-$rustHost.exe" } else { 'egui_overlay-x86_64-pc-windows-msvc.exe' }
$overlayPackagedPath = Join-Path $overlayReleaseDir $overlayPackagedName

Write-Host "Enabled static CRT Rust flag for installer build: -Ctarget-feature=+crt-static"
Write-Host "Building overlay release binary for installer bundle."
$null = New-Item -ItemType Directory -Force $overlayReleaseDir
Push-Location $workspaceRoot
try {
    $oldCargoTargetDir = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = $workspaceTargetDir
    cargo build --release -p egui_overlay --features local-ocr-rs,local-translate-ct2
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    if (Test-Path -LiteralPath $overlaySource) {
        Copy-Item -LiteralPath $overlaySource -Destination $overlayPackagedPath -Force
        Write-Host "Copied overlay package binary to $overlayPackagedPath"
    }
    else {
        throw "Expected overlay release binary not found at $overlaySource"
    }
}
finally {
    $env:CARGO_TARGET_DIR = $oldCargoTargetDir
    Pop-Location
}

Write-Host "Using rustup home: $env:RUSTUP_HOME"
Write-Host "Using cargo home: $env:CARGO_HOME"
Write-Host "Using rustup toolchain: $env:RUSTUP_TOOLCHAIN"
Write-Host "Using cargo target dir: $env:CARGO_TARGET_DIR"
Write-Host "Using cargo encoded rustflags: $env:CARGO_ENCODED_RUSTFLAGS"

Set-Location (Join-Path $workspaceRoot 'apps\tauri_desktop')
cargo tauri build --features local-ocr-rs,local-translate-ct2
