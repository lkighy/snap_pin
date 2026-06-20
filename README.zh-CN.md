# snap_pin

`snap_pin` 是一个受 Snipaste 启发的 Tauri 桌面工具。计划中的核心体验包括快速截图选区、贴图窗口、OCR、翻译、历史记录，以及可扩展的 AI / 插件工作流。

英文文档见 [README.md](README.md)。

本仓库目前是一个 Rust workspace 脚手架，模块边界已经明确。新增实现前，建议先阅读 [docs/ARCHITECTURE_RULES.md](docs/ARCHITECTURE_RULES.md)。

平台抽象以及未来 macOS/Linux 兼容方向见 [docs/PLATFORM_COMPATIBILITY_PLAN.md](docs/PLATFORM_COMPATIBILITY_PLAN.md)。

OCR 的模型与运行时方案见 [docs/OCR_STRATEGY.md](docs/OCR_STRATEGY.md)。
0.1 之后再接入的 OCR 后端见 [docs/POST_0_1_OCR_BACKENDS.md](docs/POST_0_1_OCR_BACKENDS.md)。

翻译的模型与运行时方案见 [docs/TRANSLATION_STRATEGY.md](docs/TRANSLATION_STRATEGY.md)。

启动、截图、OCR 和翻译的优化顺序见 [docs/PERFORMANCE_OPTIMIZATION_ORDER.md](docs/PERFORMANCE_OPTIMIZATION_ORDER.md)。

当前可运行 MVP 状态见 [docs/MVP_STATUS.md](docs/MVP_STATUS.md)。

`0.1.0` 发布边界和发布前清单见 [docs/RELEASE_0_1_PLAN.md](docs/RELEASE_0_1_PLAN.md)。

## 运行

先安装一次 Tauri 桌面前端依赖：

```powershell
pnpm --dir apps/tauri_desktop/ui install
```

开发桌面 UI 时，先在一个终端启动前端服务：

```powershell
pnpm --dir apps/tauri_desktop/ui dev
```

然后在另一个终端启动 Tauri 壳：

```powershell
cargo run -p tauri_desktop
```

如果想用已构建的前端快照：

```powershell
pnpm --dir apps/tauri_desktop/ui build
cargo run --release -p tauri_desktop
```

如果只运行无 GUI 的 MVP 流程：

```powershell
cargo run -p tauri_desktop -- --mvp-cli
```

## OCR 运行时

默认 workspace 构建会关闭较重的本地 OCR 运行时。没有 `local-ocr-rs` 时，应用仍能构建，但本地 MNN OCR 会返回 `local_ocr_runtime_disabled`。

在 Windows 上构建 MNN 版 `ocr-rs` 适配器，需要：

- Rust MSVC toolchain。
- Visual Studio 2022 C++ build tools。
- LLVM，用于让 bindgen 加载 `libclang.dll`。

运行脚本会检查 Windows 环境，在安装了 Visual Studio 时加载 MSVC 构建变量，设置 `LIBCLANG_PATH`，并执行带特性构建：

```powershell
pwsh scripts/check-ocr-rs-windows.ps1
```

如果缺少 `libclang.dll`，可以安装 LLVM：

```powershell
choco install llvm -y
# or
winget install LLVM.LLVM
```

如果 LLVM 安装在非标准位置，可以传入包含 `libclang.dll` 的目录，或者 LLVM 安装根目录：

```powershell
pwsh scripts/check-ocr-rs-windows.ps1 -LibClangPath "C:\Program Files\LLVM\bin"
pwsh scripts/check-ocr-rs-windows.ps1 -LibClangPath "D:\tools\LLVM"
```

如果环境检查通过，再做完整 release 构建：

```powershell
pwsh scripts/check-ocr-rs-windows.ps1 -CargoCommand "build -p tauri_desktop --release --features local-ocr-rs"
```

workspace 里对 `ocr-rs` 的本地 patch 只用于把它作为 Rust 库构建。上游 crate 还会产出 `cdylib`，而 snap_pin 并不使用它；在 release 构建里，这可能会和预编译的 Windows MNN 静态库发生链接问题。

## 翻译运行时

默认 workspace 构建会关闭原生 CTranslate2 运行时。没有 `local-translate-ct2` 时，本地翻译模型包仍可导入和校验，但翻译会返回 `local_translate_runtime_disabled`。

在 Windows 上检查 CTranslate2 适配器，需要：

- Rust MSVC toolchain。
- Visual Studio 2022 C++ build tools。
- CMake。

安装完这些工具后运行脚本：

```powershell
pwsh scripts/check-translate-ct2-windows.ps1
```

如果要做启用本地 OCR 和本地翻译的完整桌面构建：

```powershell
pwsh scripts/check-translate-ct2-windows.ps1 -CargoCommand "build -p tauri_desktop --release --features local-ocr-rs,local-translate-ct2"
```

如果缺少 CMake：

```powershell
winget install Kitware.CMake
# or
choco install cmake -y
```

## Workspace

- `apps/tauri_desktop`：Tauri 壳边界，负责托盘、设置、命令和 UI IPC。
- `apps/egui_overlay`：egui/wgpu overlay 与贴图窗口渲染边界。
- `crates/core_service`：截图、OCR、翻译、热键、剪贴板、历史和插件的编排层。
- `crates/platform_api`：跨平台的平台 trait、DTO、能力状态和平台错误。
- `crates/platform_runtime`：当前操作系统的平台组装层。应用启动和命令接线通过它拿到 `AppPlatform`。
- `crates/platform_win32`：Windows 实现，覆盖截图、窗口、热键、剪贴板、对话框、共享内存和系统 OCR。
- `crates/ipc`：Tauri、core 和 overlay 之间的消息封装与传输抽象。
- `crates/shared_models`：跨所有层共用的领域模型。

## 平台方向

项目使用基于 capability 的平台层，不把 Windows 专用 API 暴露给业务代码：

```text
platform_api -> cross-platform traits, DTOs, capabilities, errors
platform_runtime -> current-OS implementation assembly
platform_win32 -> Windows implementation
platform_macos -> future macOS implementation
platform_linux -> future Linux implementation
```

应用层和业务层应该依赖平台能力，而不是直接判断 Windows。

`ocr_engine` 只负责本地模型 OCR 和外部 OCR API 客户端。系统 OCR 通过 `platform_api::SystemOcr`，由 core/app 的接线层调度。

## 检查

在修改 app 接线、OCR、平台 crate 或 workspace 依赖之前，先运行平台边界检查：

```powershell
pwsh scripts/check-platform-boundaries.ps1
```

脚本会运行格式化、`cargo check --workspace --no-default-features`，以及依赖边界搜索，防止平台实现泄漏到上层。

## 当前状态

这是一个架构优先的脚手架。Tauri、egui/eframe、wgpu、Windows bindings、OCR SDK 和翻译 SDK 这类重依赖，应该分阶段引入。
