# OCR 策略

本文档记录 snap_pin 的 OCR 选型。后续实现 OCR 功能时，优先按这里的模型、后端、外部 API 和模块边界落地。

## 1. 总体选择

默认本地 OCR 路线选择：

```text
RapidOCR 模型生态 + PaddleOCR PP-OCRv5 mobile 模型 + Rust 本地推理后端
```

不要把选择理解成 “RapidOCR 模型 vs PaddleOCR 模型”。更准确的分工是：

- PaddleOCR 提供模型体系、训练生态和 PP-OCR 系列模型。
- RapidOCR 更适合桌面端部署，提供整理好的模型格式、模型清单和轻量推理路径。

snap_pin 默认应选择 RapidOCR 生态整理的 PaddleOCR mobile 模型。PaddleOCR 官方后端保留为高级/自定义/训练相关能力，不作为默认桌面运行时。

## 2. 默认模型

默认推荐：

```text
检测模型：PP-OCRv5 mobile det
识别模型：PP-OCRv5 mobile rec
字典文件：ppocr_keys_v5.txt
方向分类：默认关闭，按需开启
```

低配优先：

```text
PP-OCRv5 mobile FP16 MNN
```

兼容兜底：

```text
PP-OCRv4 mobile MNN 或 ONNX
```

高级可选：

```text
PP-OCRv5 server
PP-OCRv4 server
用户导入的自定义 det / rec / cls 模型
远端 OCR API
```

默认 UI 不要暴露过多专业选项。建议用以下模式包装：

- `轻量模式`：PP-OCRv5 mobile FP16 MNN，适合低配电脑。
- `标准模式`：PP-OCRv5 mobile MNN，作为默认推荐。
- `兼容模式`：PP-OCRv4 mobile MNN 或 ONNX，用于旧机器或模型异常回退。
- `高级模式`：用户手动选择 det、rec、cls、字典和推理后端。
- `云端模式`：用户选择外部 OCR API，用于低配机器、无本地模型或需要更高识别质量的场景。

## 3. 用户本地模型导入

本地模型导入必须支持用户从指定位置选择模型文件，而不是强制内置所有模型。

一个完整模型包至少包含：

```text
det model       // 文本检测
rec model       // 文本识别
keys file       // 字典文件
```

可选包含：

```text
cls model       // 文本方向分类
model manifest  // snap_pin 自定义模型清单
```

推荐模型包目录：

```text
models/
  ppocr-v5-mobile-mnn/
    manifest.toml
    det.mnn
    rec.mnn
    ppocr_keys_v5.txt
  ppocr-v5-mobile-onnx/
    manifest.toml
    det.onnx
    rec.onnx
    ppocr_keys_v5.txt
```

`manifest.toml` 建议字段：

```toml
id = "ppocr-v5-mobile-mnn"
name = "PP-OCRv5 Mobile MNN"
family = "ppocr"
version = "v5"
backend = "mnn"
precision = "fp16"
language = ["zh", "en"]

[files]
det = "det.mnn"
rec = "rec.mnn"
keys = "ppocr_keys_v5.txt"
cls = ""
```

导入时必须校验：

- 文件存在。
- backend 与文件扩展名匹配。
- det / rec / keys 完整。
- 字典版本与 rec 模型匹配。
- 不把模型路径、API key 或敏感配置写入普通日志。

## 4. 模型下载源

内置下载器只做模型包管理，不把下载逻辑写进 OCR 推理后端。

推荐后续新增：

```text
crates/model_registry
```

职责：

- 保存可用模型清单。
- 支持指定下载源。
- 支持用户自定义镜像地址。
- 下载后校验 sha256。
- 管理模型版本、启用状态和本地路径。

默认下载源可使用 RapidOCR 官方模型列表中整理的模型地址；如果网络环境不稳定，允许用户配置镜像或手动导入本地模型包。

## 5. 外部 OCR API

snap_pin 必须支持用户选择外部 OCR API。它是本地 OCR 的并列 provider，不是翻译 API 的附属功能。

适用场景：

- 用户电脑配置较低，本地模型运行慢。
- 用户不想下载或管理本地模型。
- 企业环境已有 OCR 服务。
- 特定语言、手写体、票据、表格或复杂文档需要云端模型。

推荐 provider：

```text
OpenAI Vision / OCR 能力
Azure AI Vision
Google Cloud Vision
Baidu OCR
Tencent OCR
Custom HTTP OCR
```

UI 建议：

- `本地 OCR`：默认推荐，保护隐私，可离线。
- `系统 OCR`：如果平台可用，作为轻量备选。
- `外部 API`：用户主动配置后启用。
- `关闭 OCR`：只截图和贴图。

外部 API 配置建议分为两类：

```text
Provider profile       // provider 类型、endpoint、模型名、语言、超时、重试策略
Secret credential      // api key、token、secret id、secret key
```

安全规则：

- API key、token、secret 不允许进入普通配置文件、日志、历史记录或 IPC 事件。
- Tauri 设置页可以写入密钥，但密钥必须进入系统安全存储或加密存储。
- core_service 创建任务时只携带 provider profile id，不在 `OcrJob` 里携带明文密钥。
- 外部 API OCR 默认要显示隐私提示，说明图片会发送到第三方服务。
- 历史记录保存外部 API 的 OCR 结果前仍要走用户隐私设置。

Custom HTTP OCR 推荐协议：

```text
POST /ocr
Content-Type: multipart/form-data 或 application/json

image: png/jpeg bytes 或 base64
language_hint: optional
region: optional
return_blocks: true
```

返回值应归一化为 snap_pin 内部结构：

```text
text blocks -> OcrTextBlock
plain text  -> OcrResult::plain_text
confidence  -> Option<f32>
bounds      -> Rect
language    -> Option<String>
```

失败策略：

- 网络错误、限流、鉴权失败必须返回可显示错误。
- 支持取消请求。
- 支持超时配置。
- 支持用户选择失败后回退本地 OCR 或只返回错误。
- 不自动把截图重试发送给多个外部 provider，除非用户明确开启。

## 6. Rust 接入选择

第一阶段推荐接入：

```text
ocr-rs / rust-paddle-ocr + MNN 后端
```

原因：

- 更贴近 PaddleOCR 本地推理流程。
- 支持 PP-OCRv4 / PP-OCRv5。
- 适合低配电脑，MNN 部署包通常比完整 PaddlePaddle runtime 更轻。
- 支持从文件路径加载模型，适合用户本地导入。

注意：

- `ocr-rs` / `rust-paddle-ocr` 生态成熟度不如 ONNX Runtime。
- 如果 crate API 或发布节奏不稳定，优先把它包在 snap_pin 自己的 trait 后面，不要让上层依赖具体库类型。

第二阶段可选接入：

```text
ort + ONNXRuntime
```

适用场景：

- 用户选择 ONNX 模型。
- 需要更通用的推理生态。
- 后续想接 OpenVINO、DirectML、CUDA 等执行 provider。

注意：

- 使用 `ort` 时，需要自己维护 PaddleOCR 的前处理、检测后处理、裁剪、识别解码和文本块合并流程。
- 不要让 ONNXRuntime 成为默认唯一后端，否则低配和免配置体验可能变重。

暂不推荐默认接入：

```text
PaddlePaddle 官方 runtime
```

原因：

- 桌面端安装和打包负担更重。
- 对开箱即用不如 MNN/ONNX 路线友好。
- 更适合训练、自定义模型开发、高级服务端部署。

外部 API 第一阶段建议使用：

```text
reqwest + tokio
```

后续封装在 `ocr_engine` 的 remote provider 中，不让 Tauri UI 直接调用外部 API。

## 7. 代码架构

后续建议新增 crate：

```text
crates/ocr_engine
```

核心 trait：

```rust
pub trait LocalOcrEngine {
    fn backend(&self) -> OcrBackend;
    fn load_model(&mut self, model: OcrModelBundle) -> Result<(), OcrError>;
    fn recognize(&self, image: OcrInputImage) -> Result<OcrOutput, OcrError>;
}
```

外部 API trait：

```rust
pub trait ExternalOcrClient {
    fn provider(&self) -> OcrExternalProvider;
    async fn recognize_remote(
        &self,
        profile: OcrProviderProfile,
        image: OcrInputImage,
    ) -> Result<OcrOutput, OcrError>;
}
```

建议模块：

```text
crates/ocr_engine
  src/lib.rs
  src/model_bundle.rs
  src/engine.rs
  src/mnn_backend.rs
  src/onnx_backend.rs
  src/external_api.rs
  src/http_client.rs
  src/preprocess.rs
  src/postprocess.rs
```

依赖方向：

```text
core_service -> ocr_engine -> shared_models
ocr_engine -> model_registry, shared_models
model_registry -> shared_models
```

`apps/tauri_desktop` 只负责设置页、模型选择 UI、下载/导入命令入口。`apps/egui_overlay` 只消费 OCR 结果并渲染 `TextOverlay`，不得直接调用 OCR engine。

## 8. 运行策略

OCR 必须异步运行，不得阻塞截图 overlay 和 pin window 操作。

推荐流程：

```text
CaptureFinished
  -> core_service 创建 OcrJob
  -> ocr_engine 在后台执行
  -> CoreEvent::OcrCompleted
  -> egui_overlay 更新 TextOverlay
  -> history 按隐私设置决定是否保存
```

低配策略：

- 默认关闭方向分类。
- 默认使用 mobile 模型。
- 支持取消 OCR 任务。
- 支持只识别用户选区。
- 支持降低最大输入边长。
- OCR 队列默认串行或低并发，避免抢占 overlay 渲染。

外部 API 策略：

- 默认不启用，用户配置后才可用。
- 每次调用必须走超时、取消和错误归一化。
- 外部 API 结果与本地 OCR 结果使用同一个 `OcrResult`。
- overlay 不关心 OCR 来自本地还是云端。

## 9. 决策摘要

当前决策：

```text
默认：PP-OCRv5 mobile MNN
低配：PP-OCRv5 mobile FP16 MNN
兼容：PP-OCRv4 mobile MNN/ONNX
Rust 第一后端：ocr-rs / rust-paddle-ocr
Rust 第二后端：ort
外部 API：OpenAI/Azure/Google/Baidu/Tencent/Custom HTTP
PaddleOCR 官方 runtime：高级可选，不做默认
```

后续如果模型生态或 Rust crate 发生变化，可以更新本文档，并保持 `core_service` 只依赖抽象接口。
