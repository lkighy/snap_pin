# 第一版 MVP 状态

本文档记录当前第一版程序已经具备的能力，以及哪些部分仍是可替换占位实现。

## 当前可运行能力

当前 workspace 可以直接运行桌面壳的 MVP 演示：

```powershell
cargo run -p tauri_desktop
```

默认会启动 Tauri 窗口，包含状态、OCR/翻译设置占位、模型信息和事件输出。

如果只想运行无 GUI 的核心链路演示：

```powershell
cargo run -p tauri_desktop -- --mvp-cli
```

运行后会执行一条完整的模拟流程：

```text
StartCapture
CompleteCapture
RunOcrAndTranslate
OcrQueued
OcrCompleted
TranslationQueued
TranslationCompleted
History updated
```

这个流程已经打通：

- core_service 调度。
- 模型清单默认项。
- OCR provider 抽象。
- 翻译 provider 抽象。
- OCR 结果写入历史。
- 翻译结果写入历史。
- overlay 消费 OCR/翻译事件并更新 TextOverlay。

## 当前 crate

- `apps/tauri_desktop`：第一版 Tauri 2 桌面壳入口，包含设置页、托盘、Tauri commands 和 CLI fallback。
- `apps/egui_overlay`：overlay 状态、选区、pin、文本层，当前尚未接真实 egui/wgpu 窗口。
- `crates/core_service`：截图、OCR、翻译、历史、设置和模型清单调度。
- `crates/model_registry`：默认 OCR/翻译模型清单和推荐模型选择。
- `crates/ocr_engine`：OCR trait 和 mock MNN engine。
- `crates/translate_engine`：翻译 trait 和 mock CTranslate2 engine。
- `crates/platform_win32`：Win32 截图、窗口、热键和剪贴板接口占位。
- `crates/ipc`：IPC envelope 和内存 bus。
- `crates/shared_models`：跨层共享模型。

## 占位实现

当前以下内容是 mock 或接口占位：

- 截图：未接 WGC/DXGI，使用模拟图像。
- OCR：未接 `ocr-rs` / `rust-paddle-ocr`，使用 `MockOcrEngine`。
- 翻译：未接 `ct2rs` / CTranslate2，使用 `MockTranslateEngine`。
- Tauri：已接初始窗口、托盘和 commands；设置页可以读取和保存当前进程内 Settings，尚未持久化到磁盘。
- 设置页面：已覆盖截图、贴图、OCR、翻译、快捷键、历史和模型入口。截图设置包含光标、冻结画面、放大镜、遮罩透明度、边框颜色、延迟和截图后动作。
- egui：未接真实透明 overlay、GPU 渲染和 pin window。
- 外部 API：未接 reqwest/tokio 和密钥存储。

这些占位都已经放在 trait 或 crate 边界后面，后续可以替换实现而不改变上层流程。

## 下一步建议

第一优先级：

```text
接入 Tauri 2 shell
托盘菜单
设置页
模型列表 UI
全局快捷键开关
```

第二优先级：

```text
接入 egui/eframe/wgpu overlay
实现真实选区
实现基础 pin window
把 CoreEvent 渲染为 TextOverlay
```

第三优先级：

```text
接入 Windows WGC 截图
补 DXGI fallback
处理 DPI 和多显示器
```

第四优先级：

```text
接入 OCR MNN 后端
接入翻译 CTranslate2 后端
实现模型下载/导入和校验
```

第五优先级：

```text
接入外部 OCR/翻译 API
接入系统安全存储
补隐私提示和历史策略
```
