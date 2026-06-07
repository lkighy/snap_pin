# snap_pin 架构规则

本文档是项目的长期约束。新增功能前先确认它属于哪一层；如果需要打破规则，先新增 ADR 或在 PR/提交说明中写清楚原因。

## 1. 产品边界

snap_pin 是以 Tauri 为桌面外壳、Rust 为核心服务、egui/wgpu 为高性能截图和贴图渲染层的工具。目标功能包括截图 overlay、选区、放大镜、标注、pin window、OCR、翻译、历史记录、插件管理和 AI Chat。

第一优先级是截图和贴图体验稳定、低延迟、可恢复；OCR 和翻译必须作为异步增强能力接入，不得阻塞截图选区、拖拽、缩放和置顶操作。

## 2. 进程与层级

推荐结构如下：

```text
Tauri UI / Tray / Settings
        |
        | IPC
        v
Rust Core Service
        |
        +-- Screenshot / OCR / Translate / Hotkey / Clipboard / History
        |
        +-- platform_api: screen capture / OCR / hotkey / clipboard / window capabilities
        |
        +-- egui_overlay process or window: overlay and pin rendering

Platform Runtime
        |
        +-- platform_win32: Windows implementation
        +-- platform_macos: macOS implementation
        +-- platform_linux: Linux implementation
```

Tauri 负责用户入口、配置、历史和托盘，不直接实现截图、OCR、翻译或平台原生细节。Tauri 可以在启动和命令接线边界创建或持有 `platform_runtime`，但普通 command 不应直接调用 Windows、macOS 或 Linux API。

egui overlay 负责所有对性能和透明窗口行为敏感的视觉交互，包括全屏透明 overlay、鼠标选区、放大镜、标注、贴图窗口、浮动 OCR/翻译文本和实时渲染更新。

core_service 负责调度和状态，不直接依赖 Tauri、egui 或具体平台实现类型。需要截图、系统 OCR、剪贴板、热键、窗口操作或文件对话框时，只依赖 `platform_api` 的 trait、DTO 和能力状态。

`platform_api` 定义跨平台能力接口、通用类型、能力状态和错误模型。`platform_runtime` 负责按当前操作系统组装具体实现。`platform_win32`、`platform_macos`、`platform_linux` 分别是平台实现 crate，只有这些实现 crate 允许接触对应系统原生 API。未来如需 `unsafe`，必须限制在平台实现 crate 内，并用小函数包裹。

完整平台兼容方案见 [PLATFORM_COMPATIBILITY_PLAN.md](PLATFORM_COMPATIBILITY_PLAN.md)。

## 3. Workspace 归属

本节描述目标 workspace 边界。`platform_api` 和 `platform_runtime` 已作为当前平台抽象主干引入；`platform_macos` 和 `platform_linux` 可以先由 `platform_runtime` 内的 stub 表达能力状态，后续按 [PLATFORM_COMPATIBILITY_PLAN.md](PLATFORM_COMPATIBILITY_PLAN.md) 拆成独立实现 crate。

- `apps/tauri_desktop`：Tauri shell，托盘、设置页、历史页、命令入口、前端到核心的 IPC bridge。
- `apps/egui_overlay`：egui/wgpu overlay，截图交互、贴图渲染、标注、浮动文本层。
- `crates/core_service`：业务编排，管理截图任务、OCR 任务、翻译任务、热键、剪贴板、历史和插件。
- `crates/platform_api`：平台能力 trait、通用 DTO、能力状态和平台错误类型。
- `crates/platform_runtime`：平台实现组装层，根据 `target_os` 创建 `AppPlatform`。
- `crates/platform_win32`：Windows API 实现层，WGC/DXGI/GDI 截屏、WinRT OCR、透明窗口、穿透、置顶、全局热键、剪贴板。
- `crates/platform_macos`：macOS 平台实现层，后续接 ScreenCaptureKit、Vision OCR、NSPasteboard 和窗口能力。
- `crates/platform_linux`：Linux 平台实现层，后续接 portal、Wayland/X11、剪贴板和桌面环境能力。
- `crates/ipc`：跨层消息协议、消息 envelope、传输抽象。
- `crates/shared_models`：跨层共享模型。只放稳定数据结构，不放业务编排。

新增 crate 必须满足两个条件：拥有清晰稳定的所有权边界；能减少跨层耦合，而不是把逻辑搬到新名字下。

## 4. 依赖方向

依赖必须单向：

```text
apps/* -> crates/core_service, crates/ipc, crates/shared_models
apps/* -> crates/platform_api, crates/platform_runtime
core_service -> ipc, shared_models, platform_api
ocr_engine -> shared_models, model_registry
translate_engine -> shared_models, model_registry
platform_runtime -> platform_api
platform_runtime -> platform_win32    cfg(windows)
platform_runtime -> platform_macos    cfg(target_os = "macos")
platform_runtime -> platform_linux    cfg(target_os = "linux")
platform_win32 -> platform_api, shared_models
platform_macos -> platform_api, shared_models
platform_linux -> platform_api, shared_models
ipc -> shared_models
shared_models -> no internal project crate
```

禁止 `core_service` 依赖 `apps/tauri_desktop`、`apps/egui_overlay`、`platform_runtime` 或任何具体平台实现 crate。禁止 `ocr_engine` 依赖 `platform_win32`、`platform_macos`、`platform_linux` 或 `platform_runtime`；系统 OCR 是平台能力，不属于模型 OCR engine。禁止 `shared_models` 依赖任何上层 crate。禁止在 Tauri command 或 egui 业务模块中直接调用 Win32、macOS、Linux 原生 API。

本地门禁脚本：

```powershell
pwsh scripts/check-platform-boundaries.ps1
```

该脚本至少检查：

- `cargo fmt --check`。
- `cargo check --workspace --no-default-features`。
- `apps/*`、`core_service`、`ocr_engine` 不直接引用 `platform_win32`。
- `ocr_engine` 不依赖任何平台实现或 `platform_runtime`。
- `core_service` 只允许依赖 `platform_api`，不允许依赖具体平台实现或 `platform_runtime`。
- Windows-only token 只允许出现在平台实现 crate，或极少数 app 启动/窗口接线边界。

## 5. IPC 规则

所有跨层通信必须使用 `ipc::IpcEnvelope` 或它的后续序列化版本。消息必须包含：

- `id`：用于追踪和去重。
- `source` / `target`：用于明确方向。
- `payload`：只能放命令、事件、健康检查或明确版本化的扩展 payload。

命令表示意图，例如 `StartCapture`、`RunOcr`、`Translate`。事件表示结果或状态，例如 `CaptureStarted`、`OcrCompleted`、`TranslationCompleted`。

长任务不得同步等待 UI。OCR、翻译、历史写入和网络请求必须通过事件回传进度和结果。

## 6. 截图与 overlay 规则

截图 overlay 必须优先保证：

- 透明全屏窗口稳定覆盖所有目标显示器。
- 鼠标选区、放大镜和标注在高 DPI 下坐标正确。
- GPU 渲染路径优先，CPU 图像复制必须可控且可度量。
- 退出、取消和异常恢复路径可靠。

选区坐标统一使用 `shared_models::Rect`。屏幕像素、逻辑坐标、DPI 缩放之间的转换必须集中封装，不能散落在 UI 事件处理中。

## 7. pin window 规则

贴图窗口必须支持透明、置顶、拖拽、缩放和可选鼠标穿透。贴图内容和浮动文本是同一个 pin 的渲染层，不应拆成彼此不知道的窗口状态。

OCR 和翻译文本使用 `TextOverlay` 表示，绑定到图像局部坐标或 pin bounds 的明确坐标系。实时更新时只更新文本层和必要纹理，不重建整个窗口。

## 8. OCR 与翻译规则

OCR provider 和翻译 provider 必须隐藏在 core_service 的任务接口之后。Tauri 设置页只保存 provider 配置，不直接调用 provider SDK。

OCR 结果必须保留结构化文本块、置信度、语言和 bounds，同时提供 plain text 方便复制和翻译。

系统 OCR 属于 `platform_api::SystemOcr` 能力，例如 Windows WinRT OCR、macOS Vision OCR 或未来 Linux 可用实现。`ocr_engine` 只负责本地模型 OCR 和外部 OCR API，不直接依赖具体平台实现。

本地 OCR 默认路线采用 RapidOCR 模型生态中的 PaddleOCR PP-OCRv5 mobile 模型，优先以 MNN 后端满足开箱可用和低配运行需求；外部 OCR API 作为用户主动配置的并列 provider，适合低配机器、企业 OCR 服务和复杂文档场景。具体模型包、导入规则、外部 API 和 Rust 接入方案见 [OCR_STRATEGY.md](OCR_STRATEGY.md)。

翻译请求必须保留 source text、source language、target language、provider 和 request id。任何 API key 不允许进入日志、历史记录或 IPC 普通事件。

本地翻译默认路线采用 `ct2rs + CTranslate2 + OPUS-MT/MarianMT int8`，多语言可选 `NLLB distilled / M2M100`，`rust-bert` 和 `Candle` 仅作为实验后端；外部翻译 API 作为用户主动配置的并列 provider。具体模型包、下载导入规则、外部 API 和 Rust 接入方案见 [TRANSLATION_STRATEGY.md](TRANSLATION_STRATEGY.md)。

## 9. 历史、隐私和安全

历史记录默认可配置关闭。保存截图、OCR 文本、翻译文本前必须经过 core_service 的策略判断。

敏感配置使用系统安全存储或加密存储，不写入普通 JSON、日志或崩溃报告。

插件和 AI Chat 只能通过受控接口访问截图、OCR 和翻译结果。插件不得直接访问 `platform_runtime` 或任何具体平台实现 crate；确需平台能力时必须经过受限的 `platform_api` facade 和权限策略。

## 10. 错误处理

底层错误转成领域错误或 `CoreEvent::Error` 后再跨层传播。错误消息必须适合显示给用户；调试细节放日志，不放 UI 文案。

能恢复的错误要给出恢复事件，例如 capture cancel、provider unavailable、network retry exhausted。不能让 overlay 或 Tauri UI 卡在半启动状态。

## 11. 测试规则

共享模型、IPC envelope、platform_api trait contract 和 core_service 调度必须优先写单元测试。Windows、macOS、Linux、Tauri、egui 的真实集成测试可以后置，但必须保留可替换 trait 或 mock 边界。

每次引入新 provider、截图 backend 或 IPC payload，都要至少覆盖：

- 正常任务流。
- 取消或失败路径。
- 设置更新后行为。
- 不泄漏 API key 或敏感文本到日志。

## 12. 分阶段路线

Phase 1：建立 `platform_api`，迁出通用平台类型、能力状态、平台错误和 trait。

Phase 2：让 `platform_win32` 实现 `platform_api`，保留旧函数作为迁移包装。

Phase 3：新增 `platform_runtime`，按当前 OS 创建 `AppPlatform`，非 Windows 先返回明确能力状态。

Phase 4：移除 `ocr_engine`、`core_service` 和 app 业务模块对 `platform_win32` 的直接依赖。

Phase 5：接入 Tauri 2 shell、托盘、设置页、全局快捷键开关和本地配置时，只通过平台能力接口接线。

Phase 6：接入 egui/eframe/wgpu overlay，完成选区、放大镜、基础 pin window，并通过 `WindowOps` / `ScreenCapture` 使用平台能力。

Phase 7：接入 Windows 截图 backend，优先 WGC，必要时补 DXGI/GDI fallback；同时保持 macOS/Linux stub 能力状态可编译。

Phase 8：接入 OCR pipeline、翻译 provider、浮动文本、历史、插件管理、AI Chat 和更多平台实现。
