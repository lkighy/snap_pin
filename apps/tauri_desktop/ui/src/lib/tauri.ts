import { invoke } from "@tauri-apps/api/core";

export interface AppStatus {
  bootSummary: string;
  modelSummary: string;
  historySummary: string;
  localOcrRuntimeStatus: string;
  localTranslateRuntimeStatus: string;
  platformCapabilities: PlatformCapabilities;
}

export type CapabilityStatusKey =
  | "supported"
  | "degraded"
  | "needsSetup"
  | "permissionDenied"
  | "unavailable";

export interface CapabilityStatus {
  status: CapabilityStatusKey;
  reason?: string;
  action?: string;
}

export interface PlatformCapabilities {
  screenCapture: CapabilityStatus;
  overlayWindow: CapabilityStatus;
  pinWindow: CapabilityStatus;
  systemOcr: CapabilityStatus;
  clipboardRead: CapabilityStatus;
  clipboardWrite: CapabilityStatus;
  globalHotkey: CapabilityStatus;
  fileDialog: CapabilityStatus;
  sharedMemory: CapabilityStatus;
  secureStorage: CapabilityStatus;
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
  minWidth: number;
  minHeight: number;
  showOcrText: boolean;
  showTranslationText: boolean;
  ocrText: OcrTextOverlaySettings;
}

export interface OcrTextOverlaySettings {
  fontHeightRatio: number;
  minFontSize: number;
  maxFontSize: number;
  paddingX: number;
  paddingY: number;
  interactionPaddingX: number;
  interactionPaddingY: number;
}

export interface OcrSettings {
  mode: string;
  provider: string;
  languageHint: string;
  autoRunAfterCapture: boolean;
  defaultModelId: string;
  providerProfiles: OcrProviderProfile[];
  defaultProviderProfileId: string;
}

export interface OcrProviderProfile {
  id: string;
  provider: string;
  endpoint: string;
  model: string;
  languageHint: string;
  timeoutMs: number;
  retryLimit: number;
  privacyNoticeAcknowledged: boolean;
}

export interface TranslationSettings {
  provider: string;
  targetLanguage: string;
  segmentationMode: string;
  smartMerge: SmartMergeSettings;
  autoTranslateAfterOcr: boolean;
  defaultModelId: string;
}

export interface SmartMergeSettings {
  edgeToleranceLines: number;
  looseEdgeToleranceLines: number;
  heightRatioLimit: number;
  longerLineRatio: number;
  shortLastLineRatio: number;
  inlineLabelMaxChars: number;
}

export interface HotkeySettings {
  capture: string;
  pinSelection: string;
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

export interface ModelImportResponse {
  events: string[];
  modelSummary: string;
  models: ModelSummary[];
  settings: AppSettings;
}

export interface ModelSummary {
  id: string;
  name: string;
  domain: string;
  backend: string;
  source: string;
  availability: string;
  path?: string;
  packageSource?: string;
}

export interface ModelDownloadStatus {
  running: boolean;
  modelId: string;
  role: string;
  fileName: string;
  fileIndex: number;
  fileCount: number;
  downloadedBytes: number;
  totalBytes?: number;
  percent?: number;
  error?: string;
  result?: ModelImportResponse;
}

export interface ModelStorageInfo {
  defaultOcrModelsDir: string;
  currentOcrModelsDir: string;
  usingDefault: boolean;
}

export function getAppStatus() {
  return invoke<AppStatus>("app_status");
}

export function getPlatformCapabilities() {
  return invoke<PlatformCapabilities>("platform_capabilities");
}

export function getSettings() {
  return invoke<AppSettings>("get_settings");
}

export function listModels() {
  return invoke<ModelSummary[]>("list_models");
}

export function getModelStorageInfo() {
  return invoke<ModelStorageInfo>("model_storage_info");
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

export function drainEvents() {
  return invoke<EventResponse>("drain_events");
}

export function importModel(manifestPath: string) {
  return invoke<ModelImportResponse>("import_model", { manifestPath });
}

export function downloadBuiltinOcrModel(modelId: string) {
  return invoke<ModelImportResponse>("download_builtin_ocr_model", { modelId });
}

export function startBuiltinOcrModelDownload(modelId: string) {
  return invoke<ModelDownloadStatus>("start_builtin_ocr_model_download", { modelId });
}

export function getModelDownloadStatus() {
  return invoke<ModelDownloadStatus | null>("model_download_status");
}

export function cancelModelDownload() {
  return invoke<ModelDownloadStatus | null>("cancel_model_download");
}

export function chooseOcrModelStorageDir() {
  return invoke<ModelStorageInfo | null>("choose_ocr_model_storage_dir");
}

export function setOcrModelStorageDir(path: string) {
  return invoke<ModelStorageInfo>("set_ocr_model_storage_dir", { path });
}

export function openOcrModelStorageDir() {
  return invoke<void>("open_ocr_model_storage_dir");
}
