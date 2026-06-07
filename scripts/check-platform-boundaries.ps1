[CmdletBinding()]
param(
    [switch]$SkipFmt,
    [switch]$SkipCargoCheck
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Action
    )

    Write-Host "==> $Name"
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed"
    }
}

function Invoke-Rg {
    param(
        [string]$Pattern,
        [string[]]$Paths
    )

    $output = & rg $Pattern @Paths 2>$null
    $code = $LASTEXITCODE
    if ($code -ne 0 -and $code -ne 1) {
        throw "rg failed for pattern '$Pattern'"
    }
    return @($output)
}

function Assert-NoMatches {
    param(
        [string]$Pattern,
        [string[]]$Paths,
        [string]$Message
    )

    $matches = @(Invoke-Rg -Pattern $Pattern -Paths $Paths)
    if ($matches.Count -gt 0) {
        Write-Host ""
        Write-Host $Message
        $matches | ForEach-Object { Write-Host "  $_" }
        throw $Message
    }
}

Push-Location $RepoRoot
try {
    if (-not (Get-Command rg -ErrorAction SilentlyContinue)) {
        throw "ripgrep (rg) is required for platform boundary checks"
    }

    if (-not $SkipFmt) {
        Invoke-Step "cargo fmt --check" {
            cargo fmt --check
        }
    }

    if (-not $SkipCargoCheck) {
        Invoke-Step "cargo check --workspace --no-default-features" {
            cargo check --workspace --no-default-features
        }
    }

    Assert-NoMatches `
        -Pattern "platform_win32" `
        -Paths @(
            "apps",
            "crates/core_service",
            "crates/ocr_engine",
            "crates/shared_models",
            "crates/translate_engine",
            "crates/ipc",
            "crates/model_registry"
        ) `
        -Message "Direct platform_win32 references are only allowed in platform_runtime or platform implementation crates."

    Assert-NoMatches `
        -Pattern "platform_(win32|runtime|macos|linux)" `
        -Paths @("crates/ocr_engine/Cargo.toml") `
        -Message "ocr_engine must not depend on platform implementation crates or platform_runtime."

    Assert-NoMatches `
        -Pattern "platform_(win32|runtime|macos|linux)" `
        -Paths @("crates/core_service/Cargo.toml") `
        -Message "core_service may depend on platform_api, but not platform_runtime or concrete platform implementations."

    Assert-NoMatches `
        -Pattern "platform_(win32|runtime|macos|linux)" `
        -Paths @("crates/platform_api/Cargo.toml") `
        -Message "platform_api must not depend on platform_runtime or concrete platform implementations."

    $nativeTokenMatches = @(Invoke-Rg `
        -Pattern "HWND|Win32|DXGI|WGC|windows::|windows_sys" `
        -Paths @("apps", "crates/core_service", "crates/ocr_engine", "crates/translate_engine"))

    $nativeTokenViolations = @(
        $nativeTokenMatches | Where-Object {
            $_ -notmatch "apps[\\/]+egui_overlay[\\/]+src[\\/]+capture[\\/]+window\.rs:.*RawWindowHandle::Win32"
        }
    )

    if ($nativeTokenViolations.Count -gt 0) {
        Write-Host ""
        Write-Host "Windows-only tokens leaked outside platform implementations or approved app wiring boundaries:"
        $nativeTokenViolations | ForEach-Object { Write-Host "  $_" }
        throw "Windows-only token boundary check failed"
    }

    Write-Host "==> platform boundary checks passed"
}
finally {
    Pop-Location
}
