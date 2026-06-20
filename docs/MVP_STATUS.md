# 第一版 MVP 状态

本文档记录当前第一版程序已经具备的能力，以及哪些部分仍是可替换占位实现。

## 当前可运行能力

当前 workspace 可以直接运行桌面壳的 MVP 演示：

```powershell
cargo run -p tauri_desktop
```

默认会启动 Tauri 窗口，包含状态、OCR/翻译设置占位、模型信息和事件输出。

如果只想运行无 GUI 的 0.1 发布验收流：

```powershell
cargo run -p tauri_desktop -- --mvp-cli
```

运行后会执行一条专用于验收的 mock 成功流程：

```text
StartCapture
CompleteCapture
OcrQueued
OcrCompleted
TranslationQueued
TranslationCompleted
history: 1 ocr result(s), 1 translation(s)
```

这个流程已经打通：

- core_service 调度。
- 模型清单默认项。
- OCR provider 抽象。
- 翻译 provider 抽象。
- MVP mock OCR 结果写入历史。
- MVP mock 翻译结果写入历史。
- overlay 消费 OCR/翻译事件并更新 TextOverlay。

注意：`--mvp-cli` 不代表默认 GUI/provider 路径会自动回退 mock。默认构建下，真实本地 OCR/翻译 runtime 未启用时仍应分别显示 `local_ocr_runtime_disabled` / `local_translate_runtime_disabled`。

## 当前 crate

- `apps/tauri_desktop`：第一版 Tauri 2 桌面壳入口，包含设置页、托盘、Tauri commands 和 CLI fallback。
- `apps/egui_overlay`：overlay 状态、选区、pin、文本层，当前尚未接真实 egui/wgpu 窗口。
- `crates/core_service`：截图、OCR、翻译、历史、设置和模型清单调度。
- `crates/model_registry`：默认 OCR/翻译模型清单和推荐模型选择。
- `crates/ocr_engine`：OCR trait、本地 MNN/外部 API/provider 路由；默认构建未启用 native OCR runtime 时返回明确 disabled 状态。
- `crates/translate_engine`：翻译 trait、本地 CTranslate2/provider 路由；默认构建未启用 native 翻译 runtime 时返回明确 disabled 状态。
- `crates/platform_api`：平台能力 trait、能力状态和平台错误。
- `crates/platform_runtime`：按 `target_os` 组装当前平台实现。
- `crates/platform_win32`：当前 Windows 截图、窗口、热键、剪贴板、文件对话框、共享内存和系统 OCR 实现边界。
- `crates/ipc`：IPC envelope 和内存 bus。
- `crates/shared_models`：跨层共享模型。

## 占位实现

当前以下内容是 mock 或接口占位：

- 截图：未接 WGC/DXGI，使用模拟图像。
- OCR：默认 GUI/provider 路径未接 `ocr-rs` / `rust-paddle-ocr` native runtime，会返回 `local_ocr_runtime_disabled`；`--mvp-cli` 只在发布验收 helper 内构造 mock OCR 成功结果。
- 翻译：本地 CTranslate2 模型包导入、校验、路由和异步事件流已接入；pin 贴图窗口的翻译按钮已能对 OCR 文本发起本地翻译并渲染 Translation overlay；默认构建未接 `ct2rs` native runtime，会返回 `local_translate_runtime_disabled`，不再用 mock 冒充真实本地翻译。
- Tauri：已接初始窗口、托盘和 commands；设置页可以读取和保存当前进程内 Settings，尚未持久化到磁盘。
- 设置页面：已覆盖截图、贴图、OCR、翻译、快捷键、历史和模型入口。截图设置包含光标、冻结画面、放大镜、遮罩透明度、边框颜色、延迟和截图后动作。
- egui：未接真实透明 overlay、GPU 渲染和 pin window。
- 外部 API：未接 reqwest/tokio 和密钥存储。

这些占位都已经放在 trait 或 crate 边界后面，后续可以替换实现而不改变上层流程。

## 下一步建议

0.1 发布边界、发布前关闭项和验收清单见
[RELEASE_0_1_PLAN.md](RELEASE_0_1_PLAN.md)。本节只保留功能推进顺序。
平台兼容与抽象路线见 [PLATFORM_COMPATIBILITY_PLAN.md](PLATFORM_COMPATIBILITY_PLAN.md)。

第一优先级：

```text
新增 platform_api，迁出通用平台类型、能力状态和平台错误
新增 platform_runtime，按 target_os 组装当前平台实现
让 platform_win32 实现 platform_api trait
移除 ocr_engine 对 platform_win32 的直接依赖
把 app 层散落的 platform_win32 调用收敛到平台接线边界
```

第二优先级：

```text
完成 0.1 发布前 P0/P1 验证
对齐 README、版本号和发布说明
确认默认构建下 OCR/翻译 runtime 缺失状态清晰
保留当前 Tauri shell、托盘、设置页和模型列表 UI 的稳定演示能力
```

第三优先级：

```text
接入 egui/eframe/wgpu overlay
实现真实选区
实现基础 pin window
把 CoreEvent 渲染为 TextOverlay
```

第四优先级：

```text
接入 Windows WGC 截图
补 DXGI fallback
处理 DPI 和多显示器
```

第五优先级：

```text
接入 OCR MNN 后端
实现模型下载/导入和校验
```

第六优先级：

```text
接入 ct2rs / CTranslate2 native runtime
先支持 OPUS-MT/MarianMT 语言对 int8 模型真实推理
完善本地翻译模型下载器和可用性状态
```

第七优先级：

```text
接入外部 OCR/翻译 API
接入系统安全存储
补隐私提示和历史策略
```
