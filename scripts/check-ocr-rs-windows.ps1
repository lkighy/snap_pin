param(
    [string]$LibClangPath = "",
    [switch]$SkipCargoCheck,
    [string]$CargoCommand = "check -p ocr_engine --features local-ocr-rs"
)

$ErrorActionPreference = "Stop"

function Require-Command {
    param(
        [string]$Name,
        [string]$InstallHint
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    throw @"
$Name was not found.

$InstallHint
"@
}

function Find-MsvcToolchain {
    $cl = Get-Command cl.exe -ErrorAction SilentlyContinue
    if ($cl) {
        return @{
            Description = "cl.exe on PATH: $($cl.Source)"
            VcVars64 = ""
        }
    }

    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ($programFilesX86) {
        $vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path -LiteralPath $vswhere) {
            $installationPath = & $vswhere `
                -latest `
                -products * `
                -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
                -property installationPath

            if ($LASTEXITCODE -eq 0 -and $installationPath) {
                $vcvars64 = Join-Path $installationPath "VC\Auxiliary\Build\vcvars64.bat"
                if (Test-Path -LiteralPath $vcvars64) {
                    return @{
                        Description = "Visual Studio C++ tools: $installationPath"
                        VcVars64 = $vcvars64
                    }
                }
            }
        }
    }

    throw @"
MSVC C++ build tools were not found.

Install Visual Studio Build Tools 2022 with "Desktop development with C++",
or install Visual Studio 2022 and include the MSVC x64/x86 build tools component.
"@
}

function Import-VcVars64 {
    param([string]$VcVars64)

    if ([string]::IsNullOrWhiteSpace($VcVars64)) {
        return
    }

    Write-Host "Loading MSVC environment from $VcVars64"
    $envDump = & cmd.exe /c "call `"$VcVars64`" >nul && set"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to load MSVC environment from $VcVars64"
    }

    foreach ($line in $envDump) {
        $separator = $line.IndexOf("=")
        if ($separator -le 0) {
            continue
        }

        $name = $line.Substring(0, $separator)
        $value = $line.Substring($separator + 1)
        Set-Item -Path "Env:$name" -Value $value
    }
}

function Use-VendoredMnnDll {
    param([string]$WorkspaceRoot)

    $mnnBin = Join-Path $WorkspaceRoot "third_party\ocr-rs-2.2.2\3rd_party\prebuilt\mnn-dev-windows-x86_64\lib"
    $mnnDll = Join-Path $mnnBin "MNN.dll"
    if (Test-Path -LiteralPath $mnnDll) {
        if (($env:PATH -split ";") -notcontains $mnnBin) {
            $env:PATH = "$mnnBin;$env:PATH"
        }
        return $mnnDll
    }

    return ""
}

function Copy-MnnDllToRelease {
    param(
        [string]$WorkspaceRoot,
        [string]$MnnDll
    )

    if ([string]::IsNullOrWhiteSpace($MnnDll)) {
        return
    }

    $releaseDir = Join-Path $WorkspaceRoot "target\release"
    if (Test-Path -LiteralPath $releaseDir) {
        Copy-Item -LiteralPath $MnnDll -Destination (Join-Path $releaseDir "MNN.dll") -Force
    }
}

function Resolve-LibClangCandidate {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $null
    }

    $dirs = @($Path, (Join-Path $Path "bin"))
    foreach ($dir in $dirs) {
        $candidate = Join-Path $dir "libclang.dll"
        if (Test-Path -LiteralPath $candidate) {
            return (Resolve-Path -LiteralPath $dir).Path
        }
    }

    return $null
}

function Find-LibClang {
    param([string]$ExplicitPath)

    if ($ExplicitPath) {
        $hit = Resolve-LibClangCandidate -Path $ExplicitPath
        if ($hit) {
            return $hit
        }
        throw "The supplied LLVM/libclang path '$ExplicitPath' does not contain libclang.dll"
    }

    if ($env:LIBCLANG_PATH) {
        $hit = Resolve-LibClangCandidate -Path $env:LIBCLANG_PATH
        if ($hit) {
            return $hit
        }
    }

    $roots = @(
        "C:\Program Files\LLVM\bin",
        "C:\Program Files\LLVM",
        "C:\tools\LLVM\bin",
        "C:\tools\LLVM",
        "C:\ProgramData\chocolatey\bin",
        "C:\ProgramData\chocolatey\lib\llvm\tools\bin"
    )

    foreach ($root in $roots) {
        $hit = Resolve-LibClangCandidate -Path $root
        if ($hit) {
            return $hit
        }
    }

    $pathHit = Get-Command libclang.dll -ErrorAction SilentlyContinue
    if ($pathHit) {
        return (Split-Path $pathHit.Source -Parent)
    }

    throw @"
libclang.dll was not found.

Install LLVM, then rerun this script. Examples:
  choco install llvm -y
  winget install LLVM.LLVM

Or pass the folder containing libclang.dll:
  pwsh scripts/check-ocr-rs-windows.ps1 -LibClangPath "C:\Program Files\LLVM\bin"
"@
}

if ($env:OS -ne "Windows_NT") {
    throw "This helper is intended for Windows builds."
}

Require-Command -Name "cargo" -InstallHint "Install Rust from https://rustup.rs/." | Out-Null
Require-Command -Name "rustc" -InstallHint "Install Rust from https://rustup.rs/." | Out-Null

$rustVersion = & rustc -vV
$rustHostLine = $rustVersion | Where-Object { $_ -like "host:*" } | Select-Object -First 1
if ($rustHostLine -and $rustHostLine -notmatch "msvc") {
    Write-Warning "Current Rust host is '$rustHostLine'. Windows ocr-rs builds are expected to use an MSVC target."
}

$msvc = Find-MsvcToolchain
Import-VcVars64 -VcVars64 $msvc.VcVars64
$libclang = Find-LibClang -ExplicitPath $LibClangPath
$env:LIBCLANG_PATH = $libclang
if (($env:PATH -split ";") -notcontains $libclang) {
    $env:PATH = "$libclang;$env:PATH"
}
$workspaceRoot = (Resolve-Path ".").Path
$mnnDll = Use-VendoredMnnDll -WorkspaceRoot $workspaceRoot

Write-Host $msvc.Description
Write-Host "Using LIBCLANG_PATH=$env:LIBCLANG_PATH"
if ($mnnDll) {
    Write-Host "Using MNN.dll=$mnnDll"
}

if (-not $SkipCargoCheck) {
    $cargoArgs = $CargoCommand -split "\s+"
    Write-Host "Running: cargo $CargoCommand"
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
    if ($exitCode -eq 0) {
        if ([string]::IsNullOrWhiteSpace($mnnDll)) {
            $mnnDll = Use-VendoredMnnDll -WorkspaceRoot $workspaceRoot
        }
        Copy-MnnDllToRelease -WorkspaceRoot $workspaceRoot -MnnDll $mnnDll
    }
    exit $exitCode
}
