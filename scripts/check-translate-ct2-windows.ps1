param(
    [string]$CargoCommand = "check -p translate_engine --features local-translate-ct2",
    [string[]]$CargoArgs = @(),
    [string]$CargoArgsJson = "",
    [string[]]$ExtraPath = @()
)

$ErrorActionPreference = "Stop"

function Find-VsDevShell {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        return $null
    }

    $installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installationPath) {
        return $null
    }

    $devShell = Join-Path $installationPath "Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    if (Test-Path $devShell) {
        return $devShell
    }

    return $null
}

function Import-VsDevShell {
    $devShell = Find-VsDevShell
    if (-not $devShell) {
        Write-Warning "Visual Studio C++ build tools were not found. Install VS 2022 Build Tools with the C++ workload."
        return
    }

    Import-Module $devShell
    Enter-VsDevShell -VsInstallPath (Split-Path (Split-Path (Split-Path $devShell))) -SkipAutomaticLocation -DevCmdArguments "-arch=x64 -host_arch=x64"
}

function Add-CommandDirectoryIfFound {
    param(
        [string]$Name,
        [string[]]$CandidatePaths
    )

    if (Get-Command $Name -ErrorAction SilentlyContinue) {
        return
    }

    foreach ($candidate in $CandidatePaths) {
        if (Test-Path $candidate) {
            $directory = Split-Path $candidate
            $env:PATH = "$directory;$env:PATH"
            Write-Host "Added $Name directory to PATH for this task: $directory"
            return
        }
    }
}

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
    } else {
        $env:CARGO_ENCODED_RUSTFLAGS = $Flag
    }
}

function Assert-Command {
    param(
        [string]$Name,
        [string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is required. $InstallHint"
    }
}

Write-Host "Checking local CTranslate2 translation build environment..."

Assert-Command -Name "cargo" -InstallHint "Install the Rust MSVC toolchain."

Import-VsDevShell

Add-CommandDirectoryIfFound -Name "cmake" -CandidatePaths @(
    "C:\Program Files\CMake\bin\cmake.exe",
    "C:\Program Files (x86)\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
)

Assert-Command -Name "cmake" -InstallHint "Install CMake, for example: winget install Kitware.CMake"

foreach ($path in $ExtraPath) {
    if ($path -and (Test-Path $path)) {
        $env:PATH = "$path;$env:PATH"
        Write-Host "Added extra PATH for this task: $path"
    }
}

$repoMnnDllDir = Join-Path (Get-Location) "third_party\ocr-rs-2.2.2\3rd_party\prebuilt\mnn-dev-windows-x86_64\lib"
if (Test-Path (Join-Path $repoMnnDllDir "MNN.dll")) {
    $env:PATH = "$repoMnnDllDir;$env:PATH"
    Write-Host "Added MNN.dll directory to PATH for this task: $repoMnnDllDir"
}

$effectiveCargoArgs = if ($CargoArgsJson.Trim()) {
    @($CargoArgsJson | ConvertFrom-Json) | ForEach-Object { [string]$_ }
} elseif ($CargoArgs.Count -gt 0) {
    $CargoArgs
} else {
    $CargoCommand.Split(" ", [System.StringSplitOptions]::RemoveEmptyEntries)
}

if (($effectiveCargoArgs -join " ").Contains("local-translate-ct2")) {
    Add-EncodedRustFlag -Flag "-Ctarget-feature=+crt-static"
    Write-Host "Enabled Rust static CRT for local CTranslate2: -Ctarget-feature=+crt-static"
}

Write-Host "Running cargo $($effectiveCargoArgs -join ' ')"
& cargo @effectiveCargoArgs
