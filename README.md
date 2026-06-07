# snap_pin

`snap_pin` is a Tauri-based desktop tool inspired by Snipaste. The planned core
experience is fast screenshot selection, pin windows, OCR, translation, history,
and extensible AI/plugin workflows.

The repository is currently a Rust workspace scaffold with clear ownership
boundaries. See [docs/ARCHITECTURE_RULES.md](docs/ARCHITECTURE_RULES.md) before
adding implementation code.

Platform abstraction and future macOS/Linux compatibility are tracked in
[docs/PLATFORM_COMPATIBILITY_PLAN.md](docs/PLATFORM_COMPATIBILITY_PLAN.md).

OCR-specific model and runtime decisions are tracked in
[docs/OCR_STRATEGY.md](docs/OCR_STRATEGY.md).
OCR backends deferred until after 0.1 are tracked in
[docs/POST_0_1_OCR_BACKENDS.md](docs/POST_0_1_OCR_BACKENDS.md).

Translation-specific model and runtime decisions are tracked in
[docs/TRANSLATION_STRATEGY.md](docs/TRANSLATION_STRATEGY.md).

The current runnable MVP status is tracked in
[docs/MVP_STATUS.md](docs/MVP_STATUS.md).

The `0.1.0` release boundary and pre-release checklist are tracked in
[docs/RELEASE_0_1_PLAN.md](docs/RELEASE_0_1_PLAN.md).

## Run

Install the Tauri desktop frontend dependencies once:

```powershell
pnpm --dir apps/tauri_desktop/ui install
```

For desktop UI development, run the frontend server in one terminal:

```powershell
pnpm --dir apps/tauri_desktop/ui dev
```

Then run the Tauri shell in another terminal:

```powershell
cargo run -p tauri_desktop
```

For a built frontend snapshot:

```powershell
pnpm --dir apps/tauri_desktop/ui build
cargo run --release -p tauri_desktop
```

For the non-GUI MVP flow:

```powershell
cargo run -p tauri_desktop -- --mvp-cli
```

## OCR Runtime

The default workspace build keeps the heavy local OCR runtime disabled. Without
`local-ocr-rs` the app still builds, but local MNN OCR reports
`local_ocr_runtime_disabled`.

To build the MNN-backed `ocr-rs` adapter on Windows, the machine needs:

- Rust MSVC toolchain.
- Visual Studio 2022 C++ build tools.
- LLVM, so bindgen can load `libclang.dll`.

The helper script checks the Windows environment, loads the MSVC build variables
when Visual Studio is installed, sets `LIBCLANG_PATH`, and runs the feature
build:

```powershell
pwsh scripts/check-ocr-rs-windows.ps1
```

Install LLVM with a package manager if `libclang.dll` is missing:

```powershell
choco install llvm -y
# or
winget install LLVM.LLVM
```

If LLVM is installed in a non-standard location, pass the directory containing
`libclang.dll`, or the LLVM install root:

```powershell
pwsh scripts/check-ocr-rs-windows.ps1 -LibClangPath "C:\Program Files\LLVM\bin"
pwsh scripts/check-ocr-rs-windows.ps1 -LibClangPath "D:\tools\LLVM"
```

For a full release build after the environment check succeeds:

```powershell
pwsh scripts/check-ocr-rs-windows.ps1 -CargoCommand "build -p tauri_desktop --release --features local-ocr-rs"
```

The workspace patches `ocr-rs` locally only to build it as a Rust library. The
upstream crate also emits a `cdylib`, which is not used by snap_pin and can fail
to link against the prebuilt Windows MNN static library in release builds.

## Translation Runtime

The default workspace build keeps the native CTranslate2 runtime disabled.
Without `local-translate-ct2`, local translation model packages can be imported
and validated, but translation reports `local_translate_runtime_disabled`.

To check the CTranslate2-backed adapter on Windows, the machine needs:

- Rust MSVC toolchain.
- Visual Studio 2022 C++ build tools.
- CMake.

Run the helper script after installing those tools:

```powershell
pwsh scripts/check-translate-ct2-windows.ps1
```

For a full desktop build with local OCR and local translation enabled:

```powershell
pwsh scripts/check-translate-ct2-windows.ps1 -CargoCommand "build -p tauri_desktop --release --features local-ocr-rs,local-translate-ct2"
```

Install CMake if it is missing:

```powershell
winget install Kitware.CMake
# or
choco install cmake -y
```

## Workspace

- `apps/tauri_desktop`: Tauri shell boundary for tray, settings, commands, and UI IPC.
- `apps/egui_overlay`: egui/wgpu overlay and pin-window rendering boundary.
- `crates/core_service`: orchestration for screenshot, OCR, translation, hotkeys, clipboard, history, and plugins.
- `crates/platform_api`: cross-platform platform traits, DTOs, capability status, and platform errors.
- `crates/platform_runtime`: current-OS platform assembly. App startup and command wiring use this crate to obtain `AppPlatform`.
- `crates/platform_win32`: Windows implementation for capture, windows, hotkeys, clipboard, dialogs, shared memory, and system OCR.
- `crates/ipc`: message envelopes and transport abstraction between Tauri, core, and overlay.
- `crates/shared_models`: shared domain types used across every layer.

## Platform Direction

The project uses a capability-based platform layer instead of exposing
Windows-specific APIs to business code:

```text
platform_api -> cross-platform traits, DTOs, capabilities, errors
platform_runtime -> current-OS implementation assembly
platform_win32 -> Windows implementation
platform_macos -> future macOS implementation
platform_linux -> future Linux implementation
```

Application and business layers should depend on platform capabilities instead
of checking for Windows directly.

`ocr_engine` owns local model OCR and external OCR API clients only. System OCR
is dispatched through `platform_api::SystemOcr` by core/app wiring.

## Checks

Run the platform boundary gate before changes that touch app wiring, OCR,
platform crates, or workspace dependencies:

```powershell
pwsh scripts/check-platform-boundaries.ps1
```

The script runs formatting, `cargo check --workspace --no-default-features`,
and dependency boundary searches for direct platform implementation leaks.

## Current Status

This is an architecture-first scaffold. Heavy dependencies such as Tauri,
egui/eframe, wgpu, Windows bindings, OCR SDKs, and translation SDKs should be
introduced in focused implementation phases.
