import {
  Camera,
  ClipboardList,
  History,
  Keyboard,
  Languages,
  Pin,
  Settings2,
} from "lucide-react";
import { AppStatus } from "@/lib/tauri";
import { TranslationKey } from "@/i18n";

export type ViewId =
  | "interface"
  | "capture"
  | "pin"
  | "ocr"
  | "translation"
  | "models"
  | "hotkeys"
  | "history";

// Navigation and provider option tables stay outside App so the shell focuses on orchestration.
export const navItems = [
  { id: "interface", labelKey: "view.interface", icon: Settings2 },
  { id: "capture", labelKey: "view.capture", icon: Camera },
  { id: "pin", labelKey: "view.pin", icon: Pin },
  { id: "ocr", labelKey: "view.ocr", icon: ClipboardList },
  { id: "translation", labelKey: "view.translation", icon: Languages },
  { id: "models", labelKey: "view.models", icon: Settings2 },
  { id: "hotkeys", labelKey: "view.hotkeys", icon: Keyboard },
  { id: "history", labelKey: "view.history", icon: History },
] satisfies Array<{ id: ViewId; labelKey: TranslationKey; icon: typeof Camera }>;

export const completionActions = [
  { value: "pin", labelKey: "completion.pin" },
  { value: "copy", labelKey: "completion.copy" },
  { value: "save", labelKey: "completion.save" },
  { value: "editor", labelKey: "completion.editor" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const ocrProviders = [
  { value: "local-mnn", labelKey: "provider.localMnn" },
  { value: "local-onnx", labelKey: "provider.localOnnx" },
  { value: "local-paddle", labelKey: "provider.localPaddle" },
  { value: "system", labelKey: "provider.systemOcr" },
  { value: "api-openai", labelKey: "provider.openAi" },
  { value: "api-azure", labelKey: "provider.azureVision" },
  { value: "api-google", labelKey: "provider.googleVision" },
  { value: "api-baidu", labelKey: "provider.baiduOcr" },
  { value: "api-tencent", labelKey: "provider.tencentOcr" },
  { value: "api-custom", labelKey: "provider.customHttp" },
  { value: "disabled", labelKey: "provider.disabled" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const ocrModes = [
  { value: "standard", labelKey: "ocrMode.standard" },
  { value: "lightweight", labelKey: "ocrMode.lightweight" },
  { value: "compatible", labelKey: "ocrMode.compatible" },
  { value: "advanced", labelKey: "ocrMode.advanced" },
  { value: "cloud", labelKey: "ocrMode.cloud" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const translationProviders = [
  { value: "local-ct2", labelKey: "provider.localCt2" },
  { value: "api-deepl", labelKey: "provider.deepL" },
  { value: "api-google", labelKey: "provider.googleTranslate" },
  { value: "api-azure", labelKey: "provider.azureTranslate" },
  { value: "api-openai", labelKey: "provider.openAi" },
  { value: "api-baidu", labelKey: "provider.baiduTranslate" },
  { value: "api-tencent", labelKey: "provider.tencentTranslate" },
  { value: "api-custom", labelKey: "provider.customHttp" },
  { value: "experimental-rust-bert", labelKey: "provider.rustBert" },
  { value: "experimental-candle", labelKey: "provider.candle" },
  { value: "disabled", labelKey: "provider.disabled" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const targetLanguages = [
  { value: "zh-CN", label: "zh-CN" },
  { value: "en", label: "en" },
  { value: "ja", label: "ja" },
  { value: "ko", label: "ko" },
  { value: "fr", label: "fr" },
  { value: "de", label: "de" },
];

export const defaultStatus: AppStatus = {
  bootSummary: "Starting desktop shell...",
  modelSummary: "Loading...",
  historySummary: "Loading...",
};

export type SaveStatusKey =
  | "state.ready"
  | "state.loaded"
  | "state.saving"
  | "state.saved"
  | "state.loadFailed"
  | "state.saveFailed";
