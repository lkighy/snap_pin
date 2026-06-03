# Pin OCR TODO

本文档把 pin 图 OCR 的两条后续工作合在一起：

- 本地 PP-OCRv5 MNN 模型的导入、可用性状态和下载器。
- OCR 后 pin 图文本的选中、复制和右键菜单。

目标是让用户既能稳定启用本地 OCR，也能把 pin 图上的 OCR 文本当成可操作内容。

## 1. 本地 OCR 模型落地

### 1.1 模型目录

本地 OCR 模型统一放在用户数据目录下，例如 Windows：

```text
%APPDATA%/snap_pin/models/ocr/ppocr-v5-mobile-mnn/
  manifest.toml
  det.mnn
  rec.mnn
  ppocr_keys_v5.txt
```

项目里 `model_registry::DEFAULT_OCR_MODELS_DIR` 当前是：

```text
models/ocr
```

因此桌面端应把 app data root 与该目录组合成最终模型存储根目录。

### 1.2 模型包格式

第一阶段只支持手动导入一个合法模型包：

```text
det.mnn
rec.mnn
ppocr_keys_v5.txt
manifest.toml
```

`manifest.toml` 示例：

```toml
id = "ppocr-v5-mobile-mnn"
name = "PP-OCRv5 Mobile MNN"
family = "ppocr"
version = "v5"
backend = "mnn"
precision = "fp16"
language = ["zh", "en"]
low_spec_friendly = true

[files]
det = "det.mnn"
rec = "rec.mnn"
keys = "ppocr_keys_v5.txt"
cls = ""
```

导入后由 `model_registry::ModelStorage` 复制到 app data 模型目录，并把 `ModelManifest.source` 设为：

```text
ModelSource::LocalPath("<app-data>/models/ocr/<model-id>")
```

### 1.3 手动导入流程

当前项目已经有 `import_model` 命令和设置页入口。第一阶段需要完成并验证：

- 用户选择 `manifest.toml`。
- `model_registry` 校验 `det` / `rec` / `keys` 是否存在。
- 可选校验 `checksums` 里的 sha256。
- 将模型包复制到 app data 模型目录。
- 注册到 `ModelRegistry`。
- 设置页里能选择该模型作为默认 OCR model。
- 用户开启“截图后自动 OCR”后，使用该模型执行本地 OCR。

### 1.4 模型可用性状态

UI 必须区分 runtime 和模型两种状态，避免用户误以为 build 过 feature 就一定能识别。

需要展示的状态：

```text
runtime 已启用：local-ocr-rs-enabled
runtime 未启用：local_ocr_runtime_disabled
模型未导入：缺少本地 OCR 模型
模型已导入：允许开启自动 OCR
```

建议规则：

- `local-ocr-rs-enabled` 且默认 OCR model 存在，才允许启用本地自动 OCR。
- 选择系统 OCR 时，不要求本地模型存在。
- 选择外部 OCR API 时，只校验 provider profile 和隐私确认。
- 本地 runtime 未启用时，隐藏或禁用本地模型下载按钮，并提示需要带 feature 构建。

### 1.5 下载器

下载器后续放在 `model_registry`，不要塞进 `ocr_engine`。

下载器职责：

- 下载 `det.mnn`。
- 下载 `rec.mnn`。
- 下载 `ppocr_keys_v5.txt`。
- 校验 sha256。
- 写入 app data 模型目录。
- 生成 `manifest.toml`。
- 注册到 registry。
- 设置为默认 OCR model。

建议下载完成后的目录：

```text
%APPDATA%/snap_pin/models/ocr/ppocr-v5-mobile-mnn/
  manifest.toml
  det.mnn
  rec.mnn
  ppocr_keys_v5.txt
```

### 1.6 模型来源

PaddleOCR 官方提供 PP-OCRv5 mobile det/rec 模型体系；RapidOCR 也已经支持 PP-OCRv5 路线。snap_pin 当前本地运行时是 MNN，因此最终需要 `.mnn` 文件，而不是 Paddle 原始 inference model 或 ONNX。

如果没有稳定公开的 MNN 模型包，需要增加转换/打包步骤：

- 从 PaddleOCR/RapidOCR 来源取得模型。
- 转换为 MNN。
- 固定文件名为 `det.mnn` / `rec.mnn`。
- 生成 `ppocr_keys_v5.txt`。
- 计算 sha256。
- 生成 snap_pin `manifest.toml`。

参考：

- https://www.paddleocr.ai/main/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5.html
- https://rapidai.github.io/RapidOCRDocs/main/install_usage/rapidocr/how_to_use_ppocrv5/

## 2. Pin OCR 文本选中与复制

### 2.1 目标行为

普通 pin 图：

```text
Ctrl+C
  -> 复制图片
```

OCR 识别后：

```text
如果用户选中了文字
  Ctrl+C
    -> 复制选中文字

否则
  Ctrl+C
    -> 复制图片

Ctrl+Shift+C
  -> 强制复制全部 OCR 文本
```

右键菜单：

```text
复制图片
复制选中文本
复制全部文本
保存图片
重新 OCR
```

### 2.2 当前实现基线

当前 pin 图 OCR 文本已经支持块级选中复制：

- OCR 完成后自动选中第一块非空文字。
- 点击 OCR 文本块可选中该块。
- `Ctrl+C` 复制选中的 OCR 文本块。
- 没有选中块但有 OCR 结果时，复制完整 OCR 文本。

后续需要按目标行为调整：

- 没有选中文本时，`Ctrl+C` 应复制图片，而不是复制完整 OCR 文本。
- 新增 `Ctrl+Shift+C` 复制完整 OCR 文本。
- 新增右键菜单入口。

### 2.3 文本选中路线

推荐分三步推进。

#### 阶段 A：块级选中

每个 `OcrTextBlock` 是一个可选对象。

需要状态：

```rust
selected_ocr_block: Option<usize>
```

交互：

- 点击 OCR block 命中区域，选中该 block。
- 点击空白区域，取消选中。
- 选中 block 绘制高亮。
- `Ctrl+C` 在有选中 block 时复制该 block 文本。

这是最低成本的可用方案。

#### 阶段 B：词级拖选

推荐作为正式体验目标。OCR 引擎天然能提供 word bounds，尤其 Windows OCR 当前已经能拿到 `line.Words()`。

需要扩展共享模型：

```rust
pub struct OcrTextWord {
    pub text: String,
    pub bounds: Rect,
    pub confidence: Option<f32>,
}

pub struct OcrTextBlock {
    pub text: String,
    pub bounds: Rect,
    pub confidence: Option<f32>,
    pub language: Option<String>,
    pub words: Vec<OcrTextWord>,
}
```

需要状态：

```rust
struct PinTextCursor {
    block: usize,
    word: usize,
}

struct PinTextSelection {
    anchor: PinTextCursor,
    focus: PinTextCursor,
}
```

交互：

- 鼠标按下命中 word，设置 `anchor` 和 `focus`。
- 拖动时更新 `focus`。
- anchor 到 focus 范围内的 words 高亮。
- `Ctrl+C` 复制选中的 words。
- 本地 OCR 暂无 word bounds 时，把整个 block 当作一个 word fallback。

#### 阶段 C：字符级选中

字符级选择最接近浏览器文本选择，但实现成本最高。OCR 通常没有稳定字符坐标，需要依赖 egui 排版结果反推字符 index。

需要：

- 每个 OCR block 使用与绘制一致的 `Galley`。
- 鼠标坐标映射到 galley 内部坐标。
- 从 galley 光标/行信息得到字符 index。
- 保存 `block_index + char_index` selection。
- 按字符 range 绘制高亮和复制文本。

除非必须做到精确字符选择，否则优先做词级拖选。

### 2.4 Ctrl+C 规则

最终复制优先级：

```text
Ctrl+C:
  1. 如果有文本 selection，复制选中文本。
  2. 否则复制 pin 图片。

Ctrl+Shift+C:
  1. 如果有 OCR 结果，复制完整 OCR plain_text。
  2. 否则不处理，或显示“无 OCR 文本”。
```

普通 pin 图没有 OCR 结果时：

```text
Ctrl+C -> 复制图片
Ctrl+Shift+C -> 无操作
```

### 2.5 右键菜单

右键菜单应与 toolbar 分工清晰：

- toolbar 放 OCR、翻译、关闭等快速动作。
- 右键菜单放复制/保存/重新 OCR 等上下文动作。

菜单项：

```text
复制图片
复制选中文本
复制全部文本
保存图片
重新 OCR
```

启用规则：

- `复制图片`：有 pin 图片时可用。
- `复制选中文本`：有文本 selection 时可用。
- `复制全部文本`：有 OCR result 且 `plain_text` 非空时可用。
- `保存图片`：有 pin 图片时可用。
- `重新 OCR`：有 pin 图片且 OCR 未在运行时可用。

### 2.6 验收清单

- 普通 pin 图按 `Ctrl+C` 能复制图片。
- OCR 后未选中文字，按 `Ctrl+C` 仍复制图片。
- OCR 后选中文字，按 `Ctrl+C` 复制选中文字。
- OCR 后按 `Ctrl+Shift+C` 复制完整 OCR 文本。
- 右键菜单的五个动作可见且禁用状态正确。
- OCR 正在运行时，重复 OCR 不启动并发任务。
- 选中文本不触发 pin 窗口拖动。
- 点击空白区域能取消文本选中。
- 缩放 pin 图后，文本命中区域和高亮位置仍正确。
