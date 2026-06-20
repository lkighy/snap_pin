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
  { value: "system", labelKey: "provider.systemOcr" },
  { value: "api-custom", labelKey: "provider.customHttp" },
  { value: "disabled", labelKey: "provider.disabled" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const translationProviders = [
  { value: "local-ct2", labelKey: "provider.localCt2" },
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

export const translationSegmentationModes = [
  { value: "smart-merge", labelKey: "translationSegmentation.smartMerge" },
  { value: "block-replace", labelKey: "translationSegmentation.blockReplace" },
  { value: "full-region", labelKey: "translationSegmentation.fullRegion" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

export const defaultStatus: AppStatus = {
  bootSummary: "Starting desktop shell...",
  modelSummary: "Loading...",
  historySummary: "Loading...",
  localOcrRuntimeStatus: "local-ocr-rs-disabled",
  localTranslateRuntimeStatus: "local-translate-ct2-disabled",
  platformCapabilities: {
    screenCapture: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    overlayWindow: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    pinWindow: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    systemOcr: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    clipboardRead: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    clipboardWrite: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    globalHotkey: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    fileDialog: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    sharedMemory: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
    secureStorage: {
      status: "unavailable",
      reasonCode: "platform_not_loaded",
      reason: "platform capabilities have not loaded yet",
    },
  },
};

export type SaveStatusKey =
  | "state.ready"
  | "state.loaded"
  | "state.saving"
  | "state.saved"
  | "state.loadFailed"
  | "state.saveFailed";
