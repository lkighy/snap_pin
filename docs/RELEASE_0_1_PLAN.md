# 0.1 发布计划

本文档定义 snap_pin `0.1.0` 的发布边界、发布前任务和验收清单。0.1 的目标不是完整复刻 Snipaste，而是交付一个平台抽象优先的技术预览版：Windows 作为首个可运行实现，但业务层必须为后续 macOS/Linux 接入预留稳定能力边界。

平台兼容与抽象的完整方案见 [PLATFORM_COMPATIBILITY_PLAN.md](PLATFORM_COMPATIBILITY_PLAN.md)。

## 发布定位

`0.1.0` 是面向早期试用和后续功能迭代的技术预览版。当前不以最快发布 Windows-only 版本为目标，而是优先完成平台能力抽象，避免后续接入其他系统时反复拆改业务层。

发布后应能证明：

- Tauri 桌面壳、设置页、托盘和命令入口可以稳定启动。
- 截图、pin、OCR、翻译、历史和模型管理已经有清晰模块边界。
- OCR 和翻译可以通过统一 provider 抽象切换实现。
- 截图、系统 OCR、剪贴板、热键、窗口操作、文件对话框和共享内存已经进入平台能力边界。
- 本地模型导入、runtime 可用性和错误状态不会误导用户。
- 未完成的真实 runtime 和高级后端被明确标记为后续工作。

## 0.1 范围

必须包含：

- Windows 桌面壳：主窗口、托盘入口、基础设置页、Tauri commands。
- 平台抽象：新增 `platform_api`，定义平台能力 trait、通用 DTO、能力状态和平台错误。
- 平台运行时：新增 `platform_runtime`，按当前 `target_os` 组装 `AppPlatform`。
- Windows 实现：让 `platform_win32` 作为 `platform_api` 的 Windows 实现存在，而不是上层直接依赖的通用平台层。
- 非 Windows 准备：macOS/Linux 暂不要求真实能力实现，但必须能用明确 `Unavailable`、`NeedsSetup` 或 `PermissionDenied` 表达能力状态。
- 非 GUI MVP：`cargo run -p tauri_desktop -- --mvp-cli` 可跑通核心模拟流程。
- 设置页：截图、贴图、OCR、翻译、快捷键、历史和模型入口可见，并能读写当前进程内设置。
- 截图链路：保留当前模拟截图和 overlay/pin 边界，真实 WGC/DXGI 接入排在平台抽象落地之后。
- Pin 文本层：OCR/翻译事件能更新 TextOverlay，pin 窗口侧的 OCR/翻译操作路径保持可演示。
- OCR：保留 mock/system/local provider 抽象；默认构建未启用本地 MNN runtime 时返回明确错误。
- 翻译：保留本地 CTranslate2 provider 抽象；默认构建未启用 native runtime 时返回明确错误。
- 模型管理：默认模型清单、导入校验、模型状态展示和推荐模型选择保持可用。
- 文档：README、MVP 状态、OCR/翻译策略和本发布计划互相指向。

## 明确非目标

0.1 不承诺：

- 完整透明 overlay 截图体验。
- 真实 WGC/DXGI 捕获和多显示器/DPI 完整适配。
- 开箱即用的本地 OCR 推理包下载。
- 开箱即用的本地 CTranslate2 翻译推理。
- 外部 OCR/翻译 API、密钥安全存储和云端隐私确认。
- 安装包、自动更新、代码签名和崩溃上报。
- 与 Snipaste 对齐的完整标注工具、贴图管理和快捷键矩阵。
- macOS/Linux 的真实截图、系统 OCR、全局热键和 overlay 行为。

这些内容进入 0.2 或后续版本规划。

## 发布前关闭项

### P0：发布阻塞

- 完成 `platform_api` crate，包含 `AppPlatform`、`ScreenCapture`、`SystemOcr`、`Clipboard`、`GlobalHotkey`、`WindowOps`、`FileDialog`、`SharedMemory` 的 trait 或稳定接口草案。
- 完成 `CapabilityStatus` / `PlatformCapabilities`，UI 和业务层不再用 OS 名称判断能力。
- 完成 `platform_runtime` crate，Windows 返回真实 `platform_win32` adapter，非 Windows 返回明确 stub 能力状态。
- `ocr_engine` 不再直接依赖 `platform_win32`；系统 OCR 从模型 OCR engine 中拆出，作为平台能力调度。
- app 业务模块不再散落直接调用 `platform_win32::...`；只允许启动和接线边界接触 `platform_runtime` 或 Windows adapter。
- 确认 `cargo run -p tauri_desktop -- --mvp-cli` 可完成事件流演示。
- 确认 `cargo run -p tauri_desktop` 可启动桌面壳，主窗口和托盘入口无启动崩溃。
- 确认默认构建下 OCR/翻译 runtime 缺失时展示明确错误，而不是返回 mock 成功结果。
- 确认设置页保存不会把 API key、模型绝对路径或其他敏感信息写入普通日志。
- 确认 README 的运行命令和当前 workspace 一致。
- 确认 `Cargo.toml`、`apps/tauri_desktop/tauri.conf.json` 的版本均为 `0.1.0`。

### P1：发布质量

- 运行 `cargo fmt --check`。
- 运行 `cargo check --workspace`。
- 运行 `pnpm --dir apps/tauri_desktop/ui build`。
- 运行平台依赖检查：

```powershell
rg "platform_win32" apps crates/ocr_engine crates/core_service
rg "HWND|Win32|DXGI|WGC|windows::|windows_sys" apps crates/core_service crates/ocr_engine crates/translate_engine
```

- 运行 `cargo run -p tauri_desktop -- --mvp-cli` 并把事件流结果记录到发布说明。
- 在 Windows 上手动打开设置页，检查 OCR/翻译状态文案和禁用态。
- 检查 pin 图 OCR 后复制行为是否与 [PIN_OCR_TODO.md](PIN_OCR_TODO.md) 当前基线一致。

### P2：发布说明

- 写一份 `0.1.0` 发布说明，标记为平台抽象优先的 Windows 可运行技术预览。
- 在发布说明里说明 macOS/Linux 当前是能力边界准备，不承诺真实可用功能。
- 在发布说明里列出默认构建限制：真实 OCR/翻译 native runtime 默认关闭。
- 在发布说明里列出可选验证命令：

```powershell
cargo run -p tauri_desktop -- --mvp-cli
pwsh scripts/check-ocr-rs-windows.ps1
pwsh scripts/check-translate-ct2-windows.ps1
```

## 验收清单

发布前逐项勾选：

- [ ] `cargo fmt --check` 通过。
- [ ] `cargo check --workspace` 通过。
- [ ] `pnpm --dir apps/tauri_desktop/ui build` 通过。
- [ ] `platform_api` 定义平台能力 trait、能力状态和平台错误。
- [ ] `platform_runtime` 可以创建当前 OS 的 `AppPlatform`。
- [ ] `platform_win32` 只作为 Windows 实现或迁移包装被使用。
- [ ] `ocr_engine` 不再依赖 `platform_win32`。
- [ ] app 业务模块不再直接散落调用 `platform_win32::...`。
- [ ] `cargo run -p tauri_desktop -- --mvp-cli` 输出完整 MVP 事件流。
- [ ] `cargo run -p tauri_desktop` 能启动主程序。
- [ ] 设置页能打开并显示截图、贴图、OCR、翻译、快捷键、历史和模型相关配置。
- [ ] 未启用 `local-ocr-rs` 时，本地 OCR 显示 `local_ocr_runtime_disabled` 或等价明确错误。
- [ ] 未启用 `local-translate-ct2` 时，本地翻译显示 `local_translate_runtime_disabled` 或等价明确错误。
- [ ] README 的运行说明与实际命令一致。
- [ ] 发布说明列明 0.1 的非目标和已知限制。

## 推荐发布命令

开发校验：

```powershell
cargo fmt --check
cargo check --workspace
pnpm --dir apps/tauri_desktop/ui build
rg "platform_win32" apps crates/ocr_engine crates/core_service
rg "HWND|Win32|DXGI|WGC|windows::|windows_sys" apps crates/core_service crates/ocr_engine crates/translate_engine
cargo run -p tauri_desktop -- --mvp-cli
```

可选 native runtime 校验：

```powershell
pwsh scripts/check-ocr-rs-windows.ps1
pwsh scripts/check-translate-ct2-windows.ps1
```

当前 `tauri.conf.json` 中 `bundle.active` 仍为 `false`。如果 0.1 需要分发安装包，需要另开发布打包任务，补齐图标、bundle 配置、签名策略和安装包 smoke test。

## 0.1 之后

建议 0.2 优先级：

- 接入真实 egui/eframe/wgpu 透明 overlay。
- 接入 Windows WGC 截图和 DXGI fallback。
- 为 macOS/Linux 增加平台实现 stub crate 或更完整的能力探测。
- 完成本地 PP-OCRv5 MNN 模型导入、状态和下载器。
- 完成 CTranslate2 native runtime 的真实推理适配。
- 完成 pin OCR 文本复制的最终规则和右键菜单。

更后续再推进：

- 外部 OCR/翻译 API。
- 系统安全存储。
- 历史隐私策略和清理策略。
- 安装包、自动更新和代码签名。
