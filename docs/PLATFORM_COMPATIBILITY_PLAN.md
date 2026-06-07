# 平台兼容与抽象计划

本文档定义 snap_pin 的长期平台兼容方案。目标是在继续推进 Windows 能力的同时，避免把 Windows API、句柄、权限模型和错误状态固化到业务层，降低后续接入 macOS、Linux 或其他桌面环境时的迁移成本。

核心原则：

- 业务层不判断操作系统，只查询平台能力。
- 平台相关类型、trait 和错误状态先稳定，再接具体实现。
- Windows、macOS、Linux 实现都挂在同一组平台能力接口下。
- 某个平台暂不支持某能力时，返回明确 `Unavailable` 或 `NeedsSetup`，而不是在 UI/业务层写死分支。
- 系统 OCR、截图、剪贴板、热键、窗口操作、文件对话框和共享内存都属于平台能力，不进入 OCR/翻译模型引擎。

## 目标结构

建议把当前 `platform_win32` 拆成平台 API、平台运行时和平台实现三层：

```text
crates/platform_api
  跨平台 trait、通用 DTO、能力状态、平台错误。

crates/platform_runtime
  当前进程的平台实现组装层，根据 target_os 创建 AppPlatform。

crates/platform_win32
  Windows 实现：WGC/DXGI/GDI、WinRT OCR、Win32 热键、剪贴板、窗口操作、文件对话框。

crates/platform_macos
  macOS 实现：ScreenCaptureKit、Vision OCR、NSPasteboard、系统快捷键、窗口层级和权限检测。

crates/platform_linux
  Linux 实现：portal、Wayland/X11 后端、剪贴板、全局快捷键可用性和桌面环境差异处理。
```

0.1 阶段可以只实现 `platform_api`、`platform_runtime` 和 `platform_win32`，并为 macOS/Linux 保留 stub 或 feature-gated 实现。重点是让上层代码从一开始只依赖平台能力接口。

## 依赖方向

目标依赖方向：

```text
shared_models

platform_api -> shared_models

platform_win32 -> platform_api, shared_models
platform_macos -> platform_api, shared_models
platform_linux -> platform_api, shared_models

platform_runtime -> platform_api
platform_runtime -> platform_win32    cfg(windows)
platform_runtime -> platform_macos    cfg(target_os = "macos")
platform_runtime -> platform_linux    cfg(target_os = "linux")

core_service -> platform_api
ocr_engine -> shared_models, model_registry
translate_engine -> shared_models, model_registry
apps/* -> platform_runtime, platform_api
```

约束：

- `ocr_engine` 不允许依赖 `platform_win32`、`platform_macos`、`platform_linux` 或 `platform_runtime`。
- `translate_engine` 不允许依赖任何平台实现 crate。
- `core_service` 可以依赖 `platform_api` 的 trait 和 DTO，但不得依赖具体平台实现。
- `apps/*` 只允许在启动、窗口接线、命令接线等边界处接触 `platform_runtime`。
- `shared_models` 不依赖任何平台 crate。
- 插件、AI Chat、历史和普通 IPC payload 不直接暴露平台实现类型。

## 平台能力模型

不要让 UI 询问“当前是不是 Windows”。UI 和 core_service 只读取能力状态。

建议状态：

```rust
pub enum CapabilityStatus {
    Supported,
    Degraded { reason: String },
    NeedsSetup { reason: String, action: Option<String> },
    PermissionDenied { reason: String },
    Unavailable { reason: String },
}
```

建议能力清单：

```rust
pub struct PlatformCapabilities {
    pub screen_capture: CapabilityStatus,
    pub overlay_window: CapabilityStatus,
    pub pin_window: CapabilityStatus,
    pub system_ocr: CapabilityStatus,
    pub clipboard_read: CapabilityStatus,
    pub clipboard_write: CapabilityStatus,
    pub global_hotkey: CapabilityStatus,
    pub file_dialog: CapabilityStatus,
    pub shared_memory: CapabilityStatus,
    pub secure_storage: CapabilityStatus,
}
```

能力状态示例：

- Windows 未启用 WinRT OCR 语言包：`NeedsSetup`。
- macOS 未授权屏幕录制：`PermissionDenied`。
- Linux Wayland 环境缺少 portal：`NeedsSetup`。
- Linux 桌面环境不支持全局快捷键：`Unavailable`。
- 截图可用但只能走慢速 fallback：`Degraded`。

## 核心接口

### AppPlatform

`platform_runtime` 对上层暴露一个聚合入口：

```rust
pub trait AppPlatform: Send + Sync {
    fn capabilities(&self) -> PlatformCapabilities;
    fn screen_capture(&self) -> &dyn ScreenCapture;
    fn system_ocr(&self) -> &dyn SystemOcr;
    fn clipboard(&self) -> &dyn Clipboard;
    fn global_hotkey(&self) -> &dyn GlobalHotkey;
    fn window_ops(&self) -> &dyn WindowOps;
    fn file_dialog(&self) -> &dyn FileDialog;
    fn shared_memory(&self) -> &dyn SharedMemory;
}
```

平台运行时负责创建真实实现：

```rust
pub fn create_platform() -> Box<dyn AppPlatform>;
```

### ScreenCapture

```rust
pub trait ScreenCapture {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError>;
    fn virtual_bounds(&self) -> Result<Rect, PlatformError>;
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError>;
}
```

通用类型放在 `platform_api`：

```rust
pub struct CaptureRequest {
    pub region: Option<Rect>,
    pub include_cursor: bool,
    pub backend_hint: Option<CaptureBackendHint>,
}

pub struct CapturedFrame {
    pub pixel_size: Size,
    pub scale_factor: f32,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}
```

注意：

- `CaptureBackendHint` 只能表达通用偏好，例如 `BestAvailable`、`LowLatency`、`Compatibility`。
- WGC、DXGI、GDI、ScreenCaptureKit、portal 等具体名称只出现在平台实现和诊断日志里。
- DPI、坐标转换和 monitor origin 必须在平台层归一化为 `Rect`、`Size` 和 `MonitorInfo`。

### SystemOcr

系统 OCR 是平台能力，不属于 `ocr_engine`。

```rust
pub trait SystemOcr {
    fn availability(&self) -> CapabilityStatus;
    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, PlatformError>;
}
```

约束：

- Windows WinRT OCR、macOS Vision OCR、Linux 桌面或服务型 OCR 都实现 `SystemOcr`。
- `ocr_engine` 只保留本地模型 OCR 和外部 OCR API。
- `core_service` 根据用户选择的 OCR provider，在 system/local/external 之间调度。

### Clipboard

```rust
pub trait Clipboard {
    fn read(&self) -> Result<ClipboardPayload, PlatformError>;
    fn write(&self, payload: ClipboardPayload) -> Result<(), PlatformError>;
}
```

`ClipboardPayload` 必须使用跨平台表达：

```rust
pub enum ClipboardPayload {
    Text(String),
    ImageRgba { width: usize, height: usize, bytes: Vec<u8> },
    Files(Vec<PathBuf>),
}
```

### GlobalHotkey

```rust
pub trait GlobalHotkey {
    fn register(
        &self,
        registration: HotkeyRegistration,
        sink: HotkeyEventSink,
    ) -> Result<Box<dyn HotkeyToken>, PlatformError>;
}
```

约束：

- `HotkeyRegistration` 保存用户语义化快捷键字符串，不保存 Win32 modifier bit。
- 具体平台自行解析 `Ctrl`、`Cmd`、`Option`、`Meta` 等修饰键。
- Linux 必须允许返回 `Unavailable` 或 `NeedsSetup`，因为桌面环境差异很大。

### WindowOps

```rust
pub trait WindowOps {
    fn set_always_on_top(&self, window: NativeWindowRef, enabled: bool) -> Result<(), PlatformError>;
    fn set_click_through(&self, window: NativeWindowRef, enabled: bool) -> Result<(), PlatformError>;
    fn park_window(&self, window: NativeWindowRef, bounds: Rect) -> Result<(), PlatformError>;
    fn suspend_for_modal(&self, window: NativeWindowRef) -> Result<(), PlatformError>;
    fn restore_after_modal(&self, window: NativeWindowRef, always_on_top: bool) -> Result<(), PlatformError>;
}
```

`NativeWindowRef` 是通用封装，不允许在业务层传递裸 `HWND`、`NSWindow` 或 X11/Wayland 句柄。

### FileDialog

```rust
pub trait FileDialog {
    fn pick_folder(&self, title: &str) -> Result<Option<PathBuf>, PlatformError>;
    fn save_png_path(&self, default_name: &str) -> Result<Option<PathBuf>, PlatformError>;
}
```

Tauri 已有跨平台对话框能力时，可以在 `platform_runtime` 中选择 Tauri adapter；如果 overlay 进程无法直接使用 Tauri，则由对应平台实现提供 fallback。

### SharedMemory

```rust
pub trait SharedMemory {
    fn create(&self, request: SharedMemoryCreateRequest) -> Result<SharedMemoryHandle, PlatformError>;
    fn open(&self, name: &str, byte_len: usize) -> Result<Vec<u8>, PlatformError>;
}
```

共享内存名称、权限和生命周期差异必须隐藏在实现层。IPC payload 只传跨平台 handle 描述，不传 Windows-only 结构。

## 错误模型

`PlatformError` 放在 `platform_api`，必须包含可机器判断的 code：

```rust
pub struct PlatformError {
    pub code: String,
    pub message: String,
    pub capability: Option<PlatformCapability>,
    pub recoverable: bool,
}
```

错误规则：

- 平台层负责把 Win32 HRESULT、macOS NSError、Linux portal/dbus 错误转换成稳定 code。
- UI 显示 `message`，日志可以记录平台诊断细节。
- 权限、缺组件、用户取消、平台不支持必须用不同 code。
- 不把文件路径、API key、OCR 文本或截图内容放进普通错误日志。

## 现有调用点迁移

当前需要从 `platform_win32` 迁出的主要调用点：

```text
crates/ocr_engine/src/router.rs
  system OCR 直接调用 platform_win32。

apps/tauri_desktop/src/main.rs
  HotkeyListener 类型直接来自 platform_win32。

apps/tauri_desktop/src/capture/snapshot.rs
  virtual_screen_bounds、CapturedFrame、capture_region 等直接来自 platform_win32。

apps/tauri_desktop/src/capture/launcher.rs
  共享内存、剪贴板读取等直接来自 platform_win32。

apps/tauri_desktop/src/commands/tauri.rs
  文件夹选择直接来自 platform_win32。

apps/egui_overlay/src/capture/window.rs
apps/egui_overlay/src/capture/snapshot_io.rs
apps/egui_overlay/src/capture/app.rs
apps/egui_overlay/src/pin/app.rs
  窗口操作、共享内存和保存对话框直接来自 platform_win32。
```

迁移目标：

- 上述文件不再直接调用 `platform_win32::...`。
- app 启动处创建 `AppPlatform`，再把需要的能力 adapter 注入模块。
- 短期允许在 app 层保留 `platform_runtime` 接线，业务逻辑不得持有具体平台类型。

## 迁移阶段

### Phase A：平台 API 先行

- 新增 `crates/platform_api`。
- 搬迁 `PlatformError`、`CaptureRequest`、`CapturedFrame`、`ClipboardPayload`、`HotkeyRegistration` 等通用类型。
- 定义 `CapabilityStatus`、`PlatformCapabilities` 和核心 trait。
- 暂不删除 `platform_win32` 旧函数，先让它们作为兼容包装存在。

验收：

- `platform_api` 不依赖任何平台实现 crate。
- `platform_api` 可在 Windows/macOS/Linux target 上编译。
- 文档里的依赖方向可以通过 `Cargo.toml` 检查。

### Phase B：Windows 实现适配

- 让 `platform_win32` 依赖 `platform_api` 并实现 trait。
- 把 `WindowsCaptureBackend` 改成 `ScreenCapture` 实现细节。
- 把 WinRT OCR 暴露为 `SystemOcr` 实现。
- 把热键 listener、剪贴板、窗口操作、文件对话框和共享内存包进对应 trait。

验收：

- Windows 行为与迁移前一致。
- 所有 Windows-only 类型只出现在 `platform_win32` 内部或平台接线处。

### Phase C：运行时组装

- 新增 `crates/platform_runtime`。
- 实现 `create_platform()`。
- Windows target 返回 `Win32Platform`。
- 非 Windows target 返回 stub platform，能力状态必须明确。

验收：

- 上层可以通过 `AppPlatform` 查询能力。
- 非 Windows target 至少能得到清晰的 `Unavailable` / `NeedsSetup` 状态。

### Phase D：移除上层 Windows 依赖

- 从 `ocr_engine` 移除 `platform_win32` 依赖。
- `core_service` 通过 provider 调度 `SystemOcr`。
- `apps/tauri_desktop` 和 `apps/egui_overlay` 将直接调用改为平台 trait 调用。
- README 和架构文档不再把 `platform_win32` 描述为唯一平台边界。

验收：

- `rg "platform_win32" apps crates/ocr_engine crates/core_service` 只剩平台接线或 Cargo 条件依赖。
- `ocr_engine` 的 `Cargo.toml` 不再依赖 `platform_win32`。

### Phase E：其他平台 stub 与编译门禁

- 新增 `platform_macos` / `platform_linux` stub，或在 `platform_runtime` 内提供 stub adapter。
- 为 macOS/Linux 记录能力状态、权限提示和待实现 backend。
- CI 或本地脚本增加至少 `cargo check --workspace --no-default-features` 的平台 API 检查。

验收：

- 其他平台接入时主要新增实现 crate，不需要重写业务层。
- 每个缺失能力都有用户可理解的状态和错误 code。

## 平台实现路线

### Windows

优先级：

- WGC 截图，DXGI/GDI fallback。
- WinRT OCR。
- Win32 全局热键。
- Win32/通用剪贴板。
- 透明、置顶、穿透和 modal suspend/restore。
- 命名共享内存。

### macOS

优先级：

- ScreenCaptureKit 截图。
- 屏幕录制权限检测。
- Vision OCR。
- NSPasteboard 剪贴板。
- 快捷键和窗口置顶能力。
- 共享内存或文件映射替代方案。

风险：

- 屏幕录制权限必须主动引导。
- 透明 overlay 与鼠标穿透行为需要单独验证。

### Linux

优先级：

- xdg-desktop-portal 截图优先。
- Wayland/X11 backend 区分。
- 剪贴板能力按桌面环境处理。
- 全局快捷键按 portal/桌面环境能力降级。
- OCR 默认依赖本地模型或外部 API，系统 OCR 先标记不可用。

风险：

- Wayland 安全模型限制全局截图和热键。
- 不同桌面环境行为差异大，必须把 `Degraded` 和 `NeedsSetup` 做成正常路径。

## 文档和代码门禁

任何新增平台能力必须同时更新：

- 本文档。
- [ARCHITECTURE_RULES.md](ARCHITECTURE_RULES.md) 的依赖规则。
- 相关设置页文案和能力状态展示。
- 对应 provider 或 command 的失败路径测试。

提交前建议检查：

```powershell
rg "platform_win32" apps crates
rg "HWND|Win32|DXGI|WGC|windows::|windows_sys" apps crates/core_service crates/ocr_engine crates/translate_engine
```

期望：

- Windows-only token 只出现在 `platform_win32` 或平台实现文档中。
- app 层只在启动/接线边界接触 `platform_runtime`。
- 业务模块只依赖 `platform_api`。
