# 启动、截图、OCR 和翻译优化顺序

本文档记录 snap_pin 当前阶段的性能优化顺序。目标不是一次性做完所有优化，而是先消除体感最明显、风险最低、最容易验证的延迟来源，再处理更深的截图和推理链路。

当前结论：

```text
先量化 -> 再启动 -> 再截图链路 -> 再 OCR 缓存 -> 再翻译缓存和批处理 -> 最后收敛 pin/core 架构
```

## 0. 优化原则

- 先埋点再优化。没有耗时数据时，只做明显的结构性修正。
- 优先优化用户体感路径：应用启动、热键响应、截图框出现、贴图窗口显示、第一次 OCR、第一次翻译。
- 重型 runtime 只能加载一次或按模型缓存，不允许每个任务重新构造。
- 截图链路优先减少全屏大 buffer 拷贝，其次再考虑更换底层 backend。
- OCR 和翻译任务必须在后台执行，不能阻塞 overlay 渲染、pin window 交互和 Tauri command。
- 低配默认选择轻量路径：DXGI/GDI fallback、本地 mobile OCR、CTranslate2 int8、串行或低并发 worker。

## 1. P0：建立性能基线

优先级：最高。

目标：

- 明确每段耗时，避免凭感觉优化。
- 后续每次性能改动都可以比较前后数据。

建议埋点：

```text
app_start_to_tauri_setup
tauri_setup_to_tray_ready
tauri_setup_to_hotkey_ready
tauri_setup_to_overlay_resident_ready
hotkey_to_capture_command_sent
capture_backend_duration
capture_bgra_to_rgba_duration
capture_shared_memory_create_duration
overlay_shared_memory_open_duration
overlay_texture_upload_duration
selection_to_pin_spawn_duration
pin_image_load_duration
pin_ocr_model_load_duration
pin_ocr_inference_duration
pin_translate_model_load_duration
pin_translate_inference_duration
```

主要落点：

- `apps/tauri_desktop/src/main.rs`
- `apps/tauri_desktop/src/capture/launcher.rs`
- `apps/tauri_desktop/src/capture/snapshot.rs`
- `apps/egui_overlay/src/capture/app.rs`
- `apps/egui_overlay/src/capture/snapshot_io.rs`
- `apps/egui_overlay/src/pin/app.rs`
- `crates/ocr_engine/src/ocr_rs_backend.rs`
- `crates/translate_engine/src/ct2_backend.rs`

验收标准：

- release 和 dev 模式下都能在日志里看到关键阶段耗时。
- 一次普通截图、贴图、OCR、翻译流程能拆出每段耗时。
- 日志不记录模型路径、API key、截图内容、OCR 原文和翻译原文。

当前实现：

- 已新增 `crates/perf_trace`，统一输出 `perf span=<name> duration_ms=<ms>` 日志。
- desktop 日志默认写入系统临时目录的 `snap_pin_desktop.log`。
- overlay 日志默认写入系统临时目录的 `snap_pin_overlay.log`。
- 可通过搜索 `perf span=` 汇总耗时。
- Tauri setup 已改为后台预热 resident overlay，启动不再同步等待 overlay ready。
- 开发态 overlay 优先使用 `SNAP_PIN_OVERLAY_BIN` 或已编译的 `target/debug/egui_overlay.exe`，找不到时才 fallback 到 cargo。
- shared snapshot 已携带 `rgba8/bgra8` 格式字段，desktop 侧不再强制把 BGRA 整屏转换成 RGBA。
- resident overlay 的截图命令已改为排队即 ACK，desktop 侧不再等待 overlay 完成共享内存读取和纹理构建。

Windows 示例：

```powershell
Select-String -Path "$env:TEMP\snap_pin_desktop.log" -Pattern "perf span="
Select-String -Path "$env:TEMP\snap_pin_overlay.log" -Pattern "perf span="
```

## 2. P1：优化启动和开发态 overlay 启动

优先级：最高。

当前问题：

- Tauri setup 里会同步启动 resident overlay。overlay ready 慢时，应用启动也慢。
- 开发态 overlay fallback 会走 `pwsh -> check script -> cargo run -p egui_overlay --features ...`，第一次和增量启动都很重。
- `platform_runtime::create_platform()` 在多个入口重复创建，虽然单次不一定重，但会让启动和热键路径不稳定。

建议顺序：

1. 把 resident overlay warm-up 从 Tauri setup 同步路径改成后台任务。
2. 启动时先完成 tray、settings、hotkey 注册，再异步预热 overlay。
3. 热键触发时如果 overlay 未 ready，再同步等待或启动一次。
4. 开发态优先寻找已构建的 `target/debug/egui_overlay.exe`，找不到时才 fallback 到 cargo。
5. 支持环境变量覆盖 overlay 路径，例如 `SNAP_PIN_OVERLAY_BIN`。
6. release 包把 `egui_overlay.exe` 作为 sidecar，不允许用户路径 fallback 到 cargo。
7. 在 Tauri app state 中复用同一个 `Arc<dyn AppPlatform>`。

主要落点：

- `apps/tauri_desktop/src/main.rs`
- `apps/tauri_desktop/src/capture/launcher.rs`
- `apps/tauri_desktop/src/capture/overlay_launch.rs`
- `apps/tauri_desktop/src/shell_state.rs`

验收标准：

- 冷启动后主程序能更快进入托盘可用状态。
- dev 模式重复截图不再每次触发 `cargo run`。
- 热键触发时如果 resident overlay 已 ready，截图框出现时间只包含截图和 IPC 耗时。
- release 环境找不到 overlay sidecar 时返回清晰错误，不静默走开发态 cargo fallback。

## 3. P2：优化截图链路

优先级：高。

当前问题：

- 当前截图是全屏 virtual screen capture，然后通过共享内存交给 overlay。
- GDI/DXGI 捕获后如果是 BGRA，会转成 RGBA，整屏 buffer 会重新分配一次。
- overlay 读取共享内存后会再构建 `RgbaImage` 和 texture tiles。
- pin 操作会 crop 到临时 PNG，再由 pin window 重新读图，存在磁盘写入和 PNG 编解码成本。

建议顺序：

1. `BestAvailable` 暂时跳过未实现的 WGC，优先 DXGI，失败后 GDI，并缓存最近成功 backend。
2. 扩展 shared snapshot 协议，携带 `ImageFormat`，允许 overlay 接收 `Bgra8`。
3. 避免在 Tauri 侧强制 BGRA -> RGBA 整屏转换，尽量把转换推迟到 texture 上传前或用更低拷贝方式处理。
4. 小屏使用单 texture，大屏再 tile；tile 临时 buffer 尽量复用。
5. pin window 优先支持 raw crop 共享内存或内存 IPC，减少临时 PNG 路径。
6. 保留 PNG 临时文件作为兼容 fallback，尤其用于跨进程失败或调试。

主要落点：

- `crates/platform_win32/src/app_platform.rs`
- `crates/platform_win32/src/capture/win32_dxgi.rs`
- `crates/platform_win32/src/capture/win32_gdi.rs`
- `apps/tauri_desktop/src/capture/snapshot.rs`
- `apps/egui_overlay/src/runtime/control.rs`
- `apps/egui_overlay/src/capture/snapshot_io.rs`
- `apps/egui_overlay/src/pin/launch.rs`

当前实现：

- `BestAvailable` 中的 DXGI 遇到 `dxgi_empty_frame`、`dxgi_frame_timeout` 或 `dxgi_capture_empty` 后会进入短暂冷却，后续默认截图先走 GDI，避免每次都重复等待失败的 DXGI 路径。
- shared snapshot 支持 `rgba8/bgra8`，desktop 侧直接共享原始 BGRA，overlay 侧在读取后转换。
- 截图 command 已在 overlay control 线程排队后立即返回，desktop 侧先保存共享内存句柄再发命令，保证异步加载期间 mapping 仍有效。

验收标准：

- 普通截图路径减少一次整屏 buffer 复制。
- DXGI 可用机器优先使用 DXGI，失败后稳定 fallback 到 GDI。
- 多显示器和高分辨率场景仍能正确显示 overlay。
- pin window 启动不再强依赖 PNG 编解码；fallback 路径仍可用。

## 4. P3：优化 OCR

优先级：高。

当前问题：

- 本地 OCR runtime 启用后，`ocr_rs_backend` 在识别时创建 `ocr_rs::OcrEngine`，这会让每次 OCR 都承担模型加载成本。
- pin window 内部会重新创建 `RoutedOcrEngine`、重新读图、重新加载模型。
- core_service 任务使用临时线程，缺少统一 worker、队列和并发控制。

建议顺序：

1. 修改 `PaddleOcrLocalEngine`，让 `load_model` 真正加载 native OCR engine。
2. `recognize_local` 只执行推理，不再构造 OCR engine。
3. 增加 loaded model id 判断，同一模型重复请求时跳过加载。
4. 增加 OCR worker，默认串行或低并发，替代每次任务直接 `thread::spawn`。
5. 支持取消任务时丢弃结果，避免 canceled job 继续污染 UI。
6. pin window OCR 逐步迁回 core_service 或 resident worker，通过 IPC 请求 OCR。
7. 输入预处理支持最大边长、只识别选区、方向分类按需开启。

主要落点：

- `crates/ocr_engine/src/local.rs`
- `crates/ocr_engine/src/ocr_rs_backend.rs`
- `crates/ocr_engine/src/router.rs`
- `crates/core_service/src/service.rs`
- `crates/core_service/src/ocr.rs`
- `apps/egui_overlay/src/pin/ocr.rs`
- `apps/egui_overlay/src/pin/app.rs`

验收标准：

- 第一次 OCR 可能加载模型，第二次同模型 OCR 不再重复加载模型。
- 连续触发 OCR 时不会创建无限线程。
- overlay 和 pin window 在 OCR 运行时仍可拖动、缩放、关闭。
- 本地 runtime 未编译时仍返回 `local_ocr_runtime_disabled`，错误文案清晰。

## 5. P4：优化翻译

优先级：高。

当前问题：

- 本地翻译 runtime 启用后，`ct2_backend` 在每次翻译时创建 tokenizer 和 `Translator`。
- pin window 翻译会按 block 循环调用 translate，没有充分利用 batch。
- core_service 每次翻译请求都会走默认模型准备逻辑，缺少已加载模型快速路径。

建议顺序：

1. 修改 `CTranslate2LocalEngine`，让 `load_model` 加载并缓存 tokenizer 和 `Translator`。
2. `translate` 只做语言对校验和推理。
3. 增加 loaded model id 判断，同一模型重复请求时跳过加载。
4. pin window 翻译从逐 block 调用改为一次 `translate_batch`。
5. 默认使用智能合并后的段落作为翻译单位，避免 OCR 小块逐条翻译。
6. 增加翻译 worker，默认串行，避免低配机器上与 OCR 同时抢 CPU。
7. 后续再加外部翻译 API，不抢在本地缓存和 batch 之前。

主要落点：

- `crates/translate_engine/src/local.rs`
- `crates/translate_engine/src/ct2_backend.rs`
- `crates/translate_engine/src/router.rs`
- `crates/core_service/src/service.rs`
- `crates/core_service/src/translate.rs`
- `apps/egui_overlay/src/pin/translate.rs`
- `apps/egui_overlay/src/pin/app.rs`

验收标准：

- 第一次翻译可能加载模型，第二次同模型翻译不再重复加载 translator。
- 多个 OCR block 翻译时只进行一次 batch 请求。
- 翻译结果仍能映射回正确的 OCR block 或合并段落 bounds。
- 本地 runtime 未编译时仍返回 `local_translate_runtime_disabled`。

## 6. P5：收敛 pin window 与 core_service 的职责

优先级：中。

当前问题：

- pin window 内部有独立 OCR/翻译执行路径。
- core_service 也有 OCR/翻译调度路径。
- 两条路径会导致模型缓存、worker、错误处理和历史策略重复。

建议顺序：

1. core_service 成为唯一 OCR/翻译调度入口。
2. pin window 只负责显示图片、发起请求、消费结果、渲染 overlay。
3. OCR/翻译请求通过 IPC 发给 resident core 或 Tauri shell。
4. 历史、隐私、取消、错误归一化都在 core_service 处理。
5. pin window 保留本地直跑能力作为临时 fallback，待 IPC 稳定后删除。

主要落点：

- `crates/core_service`
- `crates/ipc`
- `apps/tauri_desktop/src/ipc`
- `apps/egui_overlay/src/pin`

验收标准：

- 同一模型只在统一 worker 中加载一次。
- pin window 不再直接依赖具体 OCR/翻译 runtime。
- 关闭 pin window 能取消或忽略对应任务结果。
- OCR/翻译历史策略只在 core_service 内决定。

## 7. P6：外部 API 和高级 runtime

优先级：中低。

前置条件：

- 本地 OCR 和本地翻译的缓存、worker、错误状态已经稳定。
- 安全存储方案已经可用。
- 设置页已有隐私提示。

建议顺序：

1. 外部 OCR API。
2. 外部翻译 API。
3. 自定义 HTTP provider。
4. ONNX/OpenVINO/DirectML/CUDA 等高级本地 runtime。
5. 多 provider fallback，但必须由用户明确启用。

验收标准：

- API key 不进入普通配置、日志、历史和 IPC 明文事件。
- 外部请求支持超时、取消和错误归一化。
- 不自动把截图或文本发送给多个第三方 provider。

## 推荐执行表

```text
第一轮：
P0 埋点
P1 异步 overlay warm-up
P1 dev overlay 优先 exe

第二轮：
P2 DXGI/GDI backend 选择和缓存
P2 shared snapshot 支持格式字段
P2 减少 BGRA/RGBA 整屏复制

第三轮：
P3 OCR native engine 缓存
P3 OCR worker 和取消策略
P4 CTranslate2 translator 缓存
P4 翻译 batch

第四轮：
P5 pin window 请求收敛到 core_service
P6 外部 API 和高级 runtime
```

## 不建议提前做的事

- 在没有埋点前大改截图 backend。
- 在本地 OCR/翻译还没有缓存前接多个外部 API。
- 默认启用多语言大模型或服务端 OCR 模型。
- 为了快而绕过 `platform_api`、`core_service` 和 engine trait 边界。
- 把 API key、模型路径、截图内容或识别文本写入普通日志。
