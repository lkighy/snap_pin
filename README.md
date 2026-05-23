# snap_pin

`snap_pin` is a Tauri-based desktop tool inspired by Snipaste. The planned core
experience is fast screenshot selection, pin windows, OCR, translation, history,
and extensible AI/plugin workflows.

The repository is currently a Rust workspace scaffold with clear ownership
boundaries. See [docs/ARCHITECTURE_RULES.md](docs/ARCHITECTURE_RULES.md) before
adding implementation code.

OCR-specific model and runtime decisions are tracked in
[docs/OCR_STRATEGY.md](docs/OCR_STRATEGY.md).

Translation-specific model and runtime decisions are tracked in
[docs/TRANSLATION_STRATEGY.md](docs/TRANSLATION_STRATEGY.md).

The current runnable MVP status is tracked in
[docs/MVP_STATUS.md](docs/MVP_STATUS.md).

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

## Workspace

- `apps/tauri_desktop`: Tauri shell boundary for tray, settings, commands, and UI IPC.
- `apps/egui_overlay`: egui/wgpu overlay and pin-window rendering boundary.
- `crates/core_service`: orchestration for screenshot, OCR, translation, hotkeys, clipboard, history, and plugins.
- `crates/platform_win32`: Windows-specific APIs such as DXGI/WGC, windows, hotkeys, and clipboard.
- `crates/ipc`: message envelopes and transport abstraction between Tauri, core, and overlay.
- `crates/shared_models`: shared domain types used across every layer.

## Current Status

This is an architecture-first scaffold. Heavy dependencies such as Tauri,
egui/eframe, wgpu, Windows bindings, OCR SDKs, and translation SDKs should be
introduced in focused implementation phases.
