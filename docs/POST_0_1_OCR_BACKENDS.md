# Post-0.1 OCR Backends

This note records OCR backend work intentionally deferred until after the 0.1
release.

## 0.1 Scope

The 0.1 release should expose only these OCR providers in the UI:

- Local MNN
- System OCR
- Custom HTTP OCR
- Disabled

Local MNN is the only local bundled runtime targeted for 0.1. It uses the
`local-ocr-rs` feature, MNN runtime libraries, and downloaded `.mnn` model
bundles.

The codebase may keep enum and routing placeholders for ONNX Runtime and Paddle
Runtime so older settings can still parse and future work has stable names, but
these providers should not be user-facing before their runtimes are implemented.

The codebase may also keep enum and routing placeholders for named cloud OCR
vendors, but 0.1 should expose only Custom HTTP for external OCR. Named vendor
adapters should not be user-facing until each API contract, authentication
scheme, privacy notice, and response mapper is implemented.

## Deferred: Local ONNXRuntime

Goal: run PaddleOCR/RapidOCR ONNX model bundles locally through ONNX Runtime.

Expected model bundle shape:

- `det.onnx`
- `rec.onnx`
- optional `cls.onnx`
- `ppocr_keys_v5.txt` or version-matched dictionary

Implementation tasks:

- Add a gated `local-onnxruntime` Cargo feature.
- Add ONNX Runtime dependencies, likely `ort` plus tensor helpers.
- Add `ocr_engine::onnx_backend`.
- Implement PaddleOCR preprocessing for detection and recognition.
- Implement DB text detection postprocessing.
- Implement text-line crop and perspective transform.
- Implement recognition CTC decoding.
- Package `onnxruntime.dll` for Windows builds.
- Add model download/import sources with `backend = "onnxruntime"`.
- Add focused tests with tiny fixtures or golden outputs.

Risks:

- The work is mostly OCR pipeline implementation, not just session loading.
- Runtime DLL packaging is similar to MNN but with different distribution rules.
- Performance and memory behavior need separate tuning.

## Deferred: Local PaddleRuntime

Goal: run Paddle native inference models locally.

Expected model bundle shape:

- `det/inference.pdmodel`
- `det/inference.pdiparams`
- `rec/inference.pdmodel`
- `rec/inference.pdiparams`
- optional classifier model files
- version-matched dictionary

Possible implementation paths:

- Direct Rust FFI to Paddle Inference C/C++ APIs.
- A sidecar worker process using PaddleOCR/Paddle Inference and IPC.

Risks:

- Larger runtime footprint.
- More complex Windows packaging.
- C++ ABI and runtime dependency issues.
- Slower cold start if implemented as a sidecar.

## Deferred: Named Cloud OCR Providers

Goal: provide first-class adapters for specific OCR APIs instead of requiring
users to wrap them behind Custom HTTP.

Deferred providers:

- OpenAI OCR or vision-capable model adapter
- Azure Vision OCR
- Google Vision OCR
- Baidu OCR
- Tencent OCR

Implementation tasks:

- Define provider-specific credential storage and validation.
- Map each request format from snap_pin image data to the provider API.
- Map each provider response into `OcrResult` blocks, bounds, confidence, and
  plain text.
- Add provider-specific timeout and retry behavior.
- Add privacy notices that clearly describe external image upload.
- Add tests with redacted fixtures or mocked HTTP responses.

Risks:

- Each provider has different authentication, response geometry, language hints,
  quotas, and error semantics.
- API behavior and pricing can change independently of the app.
- Shipping named cloud providers increases privacy and support burden before the
  0.1 local OCR flow is stable.

## Revisit Criteria

Reconsider these backends after 0.1 when:

- Local MNN download, runtime loading, and pin-window OCR are stable.
- The app has a repeatable Windows release packaging path.
- Settings and model management are settled enough to support backend-specific
  warnings.
- There is a concrete need that MNN does not cover, such as cross-platform
  deployment, GPU acceleration, or ONNX model ecosystem compatibility.
- Custom HTTP OCR is stable enough to use as the generic external-provider
  escape hatch.
