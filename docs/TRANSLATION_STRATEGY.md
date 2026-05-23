# 翻译策略

本文档记录 snap_pin 的翻译功能选型。后续开始实现翻译功能时，优先按这里的模型、后端、外部 API 和模块边界落地。

## 1. 总体选择

翻译模块必须和 OCR 解耦。OCR 负责从截图得到结构化文本，翻译模块消费 OCR 结果或用户选中的文本。

推荐流程：

```text
CaptureFinished
  -> OcrJob
  -> OcrResult { blocks, plain_text }
  -> TranslationJob
  -> LocalTranslateEngine 或 ExternalTranslateClient
  -> TranslationResult
  -> egui_overlay 更新翻译 TextOverlay
```

默认翻译路线：

```text
低配默认：ct2rs + CTranslate2 + OPUS-MT/MarianMT int8
多语言可选：ct2rs + CTranslate2 + NLLB distilled / M2M100
实验后端：rust-bert / Candle
外部 API：DeepL / Google / Azure / OpenAI / Baidu / Tencent / Custom HTTP
```

## 2. 本地模型选择

低配默认：

```text
CTranslate2 + OPUS-MT/MarianMT int8
```

理由：

- 按语言对下载，例如 `en-zh`、`ja-zh`、`zh-en`。
- 用户只下载自己需要的方向，磁盘和内存压力小。
- int8 量化适合低配电脑。
- 对 OCR 后的短文本翻译足够实用。

标准模式：

```text
CTranslate2 + OPUS-MT/MarianMT int8_float32 或 float32
```

适合希望质量略好、机器配置尚可的用户。

多语言可选：

```text
CTranslate2 + NLLB distilled
CTranslate2 + M2M100
```

适合语言种类多、不想逐个下载语言对模型的用户。默认不要自动下载多语言大模型，必须由用户主动选择。

实验后端：

```text
rust-bert
Candle
```

`rust-bert` 支持翻译 pipeline，但依赖 libtorch，部署体积和运行成本较高，不适合作为默认低配方案。`Candle` 更 Rust 原生，但需要维护更多模型适配、tokenizer 和推理细节。二者只作为实验/高级后端保留接口。

不建议默认：

```text
本地大语言模型
```

本地 LLM 可用于润色或 AI Chat，但模型体积、延迟和低配体验不适合作为截图翻译默认方案。

## 3. 模型下载与导入

翻译模型必须支持用户从指定位置下载或导入，不强制内置所有模型。

推荐模型包目录：

```text
models/
  translate-opus-mt-en-zh-ct2-int8/
    manifest.toml
    model.bin
    config.json
    source.spm
    target.spm
    shared_vocabulary.json
  translate-nllb-distilled-ct2-int8/
    manifest.toml
    model.bin
    tokenizer.json
    sentencepiece.bpe.model
```

`manifest.toml` 建议字段：

```toml
id = "opus-mt-en-zh-ct2-int8"
name = "OPUS-MT English to Chinese"
family = "opus-mt"
backend = "ctranslate2"
source_language = "en"
target_language = "zh"
quantization = "int8"
low_spec_friendly = true
multilingual = false

[files]
model = "model.bin"
config = "config.json"
source_tokenizer = "source.spm"
target_tokenizer = "target.spm"
vocabulary = "shared_vocabulary.json"
```

导入时必须校验：

- 文件存在。
- backend 与模型格式匹配。
- tokenizer / vocabulary 完整。
- source language 和 target language 与模型方向匹配。
- 量化信息与运行后端兼容。
- 不把模型路径、API key 或敏感配置写入普通日志。

模型下载职责应放在后续的 `model_registry`，不要写进翻译推理后端。

## 4. Rust 接入选择

第一阶段推荐：

```text
ct2rs + CTranslate2
```

职责：

- 加载 CTranslate2 格式模型。
- 支持 OPUS-MT/MarianMT 语言对模型。
- 支持 NLLB distilled / M2M100 等多语言模型。
- 优先支持 int8 量化模型。

注意：

- 需要处理 CTranslate2 native runtime 的打包。
- 上层不得直接依赖 `ct2rs` 类型，必须通过 snap_pin 自己的 trait 隔离。

第二阶段实验：

```text
rust-bert
Candle
```

使用条件：

- 只在用户主动启用实验后端时加载。
- 不作为默认下载项。
- 不阻塞标准 CTranslate2 路线。
- 遇到依赖体积、GPU/CPU runtime 或模型适配问题时，可以随时降级为隐藏功能。

## 5. 外部翻译 API

外部翻译 API 和本地模型是并列 provider。

推荐 provider：

```text
DeepL
Google Cloud Translation
Azure Translator
OpenAI
Baidu Translate
Tencent Translate
Custom HTTP
```

适用场景：

- 用户希望更高质量翻译。
- 用户机器配置低，不想跑本地模型。
- 企业环境已有翻译服务。
- 需要术语表、上下文、风格控制或长文本处理。

安全规则：

- API key、token、secret 不允许进入普通配置文件、日志、历史记录或 IPC 事件。
- Tauri 设置页可以写入密钥，但密钥必须进入系统安全存储或加密存储。
- core_service 创建任务时只携带 provider profile id，不在 `TranslationRequest` 里携带明文密钥。
- 外部 API 翻译默认要显示隐私提示，说明文本会发送到第三方服务。
- 历史记录保存翻译结果前仍要走用户隐私设置。

Custom HTTP 推荐协议：

```text
POST /translate
Content-Type: application/json

{
  "text": "source text",
  "source_language": "auto",
  "target_language": "zh-CN",
  "context": "optional OCR surrounding text"
}
```

返回值应归一化为 snap_pin 内部结构：

```text
translated_text -> TranslationResult::translated_text
source_language -> Option<LanguageCode>
target_language -> LanguageCode
provider        -> TranslateProvider
```

失败策略：

- 网络错误、限流、鉴权失败必须返回可显示错误。
- 支持取消请求。
- 支持超时配置。
- 支持用户选择失败后回退本地翻译或只返回错误。
- 不自动把文本重试发送给多个外部 provider，除非用户明确开启。

## 6. 代码架构

后续建议新增 crate：

```text
crates/translate_engine
```

核心 trait：

```rust
pub trait LocalTranslateEngine {
    fn backend(&self) -> TranslateLocalBackend;
    fn load_model(&mut self, model: TranslationModelBundle) -> Result<(), TranslateError>;
    fn translate(&self, request: TranslationInput) -> Result<TranslationOutput, TranslateError>;
}
```

外部 API trait：

```rust
pub trait ExternalTranslateClient {
    fn provider(&self) -> TranslateExternalProvider;
    async fn translate_remote(
        &self,
        profile: TranslateProviderProfile,
        input: TranslationInput,
    ) -> Result<TranslationOutput, TranslateError>;
}
```

建议模块：

```text
crates/translate_engine
  src/lib.rs
  src/model_bundle.rs
  src/engine.rs
  src/ct2_backend.rs
  src/rust_bert_backend.rs
  src/candle_backend.rs
  src/external_api.rs
  src/http_client.rs
  src/tokenization.rs
```

依赖方向：

```text
core_service -> translate_engine -> shared_models
translate_engine -> model_registry, shared_models
model_registry -> shared_models
```

`apps/tauri_desktop` 只负责设置页、模型选择 UI、下载/导入命令入口和外部 API 配置入口。`apps/egui_overlay` 只消费翻译结果并渲染 `TextOverlay`，不得直接调用翻译 engine。

## 7. 运行策略

翻译必须异步运行，不得阻塞截图 overlay 和 pin window 操作。

推荐流程：

```text
OcrCompleted
  -> core_service 按设置创建 TranslationRequest
  -> translate_engine 在后台执行
  -> CoreEvent::TranslationCompleted
  -> egui_overlay 更新 Translation TextOverlay
  -> history 按隐私设置决定是否保存
```

低配策略：

- 默认使用语言对模型，不默认加载多语言大模型。
- 默认优先 int8。
- 支持取消翻译任务。
- 支持只翻译用户选区或当前 pin 的文本。
- 翻译队列默认串行或低并发，避免抢占 overlay 渲染。
- 如果用户未下载对应语言对模型，提示下载、切换外部 API 或选择多语言模型。

## 8. 决策摘要

当前决策：

```text
低配默认：ct2rs + CTranslate2 + OPUS-MT/MarianMT int8
多语言可选：ct2rs + CTranslate2 + NLLB distilled / M2M100
实验后端：rust-bert / Candle
外部 API：DeepL / Google / Azure / OpenAI / Baidu / Tencent / Custom HTTP
```

后续如果模型生态或 Rust crate 发生变化，可以更新本文档，并保持 `core_service` 只依赖抽象接口。
