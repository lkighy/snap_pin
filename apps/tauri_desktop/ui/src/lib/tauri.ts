import { invoke } from "@tauri-apps/api/core";

export interface AppStatus {
  bootSummary: string;
  modelSummary: string;
  historySummary: string;
}

export interface InterfaceSettings {
  language: string;
}

export interface CaptureSettings {
  includeCursor: boolean;
  autoCopyToClipboard: boolean;
  freezeScreenOnCapture: boolean;
  showSizeLabel: boolean;
  showToolbar: boolean;
  captureDelayMs: number;
  maskOpacity: number;
  borderColor: string;
  completionAction: string;
}

export interface OverlaySettings {
  showMagnifier: boolean;
  magnifierScale: number;
}

export interface PinSettings {
  defaultOpacity: number;
  clickThrough: boolean;
  alwaysOnTop: boolean;
  rememberPosition: boolean;
  zoomStep: number;
  showOcrText: boolean;
  showTranslationText: boolean;
}

export interface OcrSettings {
  provider: string;
  languageHint: string;
  autoRunAfterCapture: boolean;
}

export interface TranslationSettings {
  provider: string;
  targetLanguage: string;
  autoTranslateAfterOcr: boolean;
}

export interface HotkeySettings {
  capture: string;
  togglePinsClickThrough: string;
  showHistory: string;
}

export interface HistorySettings {
  enabled: boolean;
  maxEntries: number;
}

export interface AppSettings {
  interface: InterfaceSettings;
  capture: CaptureSettings;
  overlay: OverlaySettings;
  pin: PinSettings;
  ocr: OcrSettings;
  translation: TranslationSettings;
  hotkeys: HotkeySettings;
  history: HistorySettings;
}

export interface EventResponse {
  events: string[];
  historySummary?: string;
}

export function getAppStatus() {
  return invoke<AppStatus>("app_status");
}

export function getSettings() {
  return invoke<AppSettings>("get_settings");
}

export function saveSettings(settings: AppSettings) {
  return invoke<AppSettings>("save_settings", { settings });
}

export function startCapture() {
  return invoke<EventResponse>("start_capture");
}

export function runMvpFlow() {
  return invoke<EventResponse>("run_mvp_flow");
}
