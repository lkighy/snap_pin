import {
  AppSettings,
  AppStatus,
  CapabilityStatus,
  ModelDownloadStatus,
  ModelStorageInfo,
  ModelSummary,
  PlatformCapabilities,
} from "@/lib/tauri";
import { localeOptions, Translator } from "@/i18n";
import {
  AlertCircle,
  CheckCircle2,
  Download,
  FolderOpen,
  Loader2,
  Square,
  Upload,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  completionActions,
  ocrProviders,
  targetLanguages,
  translationSegmentationModes,
  translationProviders,
  ViewId,
} from "@/app-data";
import {
  ColorField,
  HotkeyField,
  localizedOptions,
  ModelTile,
  NumberField,
  RangeField,
  SelectField,
  SettingsSection,
  SettingsStack,
  SwitchField,
  TextField,
} from "@/settings-fields";

interface SettingsPanelProps {
  activeView: ViewId;
  settings: AppSettings;
  updateSettings: <K extends keyof AppSettings>(
    section: K,
    values: Partial<AppSettings[K]>,
  ) => void;
  modelManifestPath: string;
  onModelManifestPathChange: (value: string) => void;
  modelStorageInfo: ModelStorageInfo | null;
  modelStoragePath: string;
  onModelStoragePathChange: (value: string) => void;
  onImportModel: () => void;
  onDownloadOcrModel: (modelId: string) => void;
  onCancelModelDownload: () => void;
  onChooseModelStorageDir: () => void;
  onApplyModelStorageDir: () => void;
  onOpenModelStorageDir: () => void;
  onOpenModels: () => void;
  importingModel: boolean;
  managingModelStorage: boolean;
  downloadingOcrModel: boolean;
  modelDownloadStatus: ModelDownloadStatus | null;
  models: ModelSummary[];
  status: AppStatus;
  t: Translator;
}

const builtinOcrModelPackages = [
  {
    id: "ppocr-v5-mobile-mnn",
    domain: "OCR",
    name: "PP-OCRv5 Mobile MNN",
    profile: "标准",
    detail: "默认推荐，适合常规截图 OCR。",
  },
  {
    id: "ppocr-v5-mobile-fp16-mnn",
    domain: "OCR",
    name: "PP-OCRv5 Mobile FP16 MNN",
    profile: "轻量",
    detail: "低配机器优先，体积和运行压力更小。",
  },
  {
    id: "ppocr-v4-mobile-mnn",
    domain: "OCR",
    name: "PP-OCRv4 Mobile MNN",
    profile: "兼容",
    detail: "用于 v5 模型异常或旧环境回退。",
  },
];

const builtinTranslationModelPackages = [
  {
    id: "opus-mt-en-zh-ct2-int8",
    domain: "翻译",
    name: "OPUS-MT English to Chinese CTranslate2",
    profile: "英中",
    detail: "Hugging Face gaudi/opus-mt-en-zh-ctranslate2，约 160 MB。",
    source: "gaudi/opus-mt-en-zh-ctranslate2",
  },
];

const localOcrProviders = new Set(["local-mnn", "local-onnx", "local-paddle"]);
const localTranslationProviders = new Set(["local-ct2"]);
const apiTranslationProviders = new Set([
  "api-deepl",
  "api-google",
  "api-azure",
  "api-openai",
  "api-baidu",
  "api-tencent",
  "api-custom",
]);
const apiOcrProviders = new Set([
  "api-openai",
  "api-azure",
  "api-google",
  "api-baidu",
  "api-tencent",
  "api-custom",
]);
const recommendedOcrModelId = "ppocr-v5-mobile-mnn";
const showModelRegistryDetails = import.meta.env.DEV;

function optionValueOrDefault(
  value: string,
  options: ReadonlyArray<{ value: string }>,
  fallback: string,
) {
  return options.some((option) => option.value === value) ? value : fallback;
}

// Each branch mirrors one navigation tab, keeping field wiring close to the settings shape.
export function SettingsPanel({
  activeView,
  settings,
  updateSettings,
  modelManifestPath,
  onModelManifestPathChange,
  modelStorageInfo,
  modelStoragePath,
  onModelStoragePathChange,
  onImportModel,
  onDownloadOcrModel,
  onCancelModelDownload,
  onChooseModelStorageDir,
  onApplyModelStorageDir,
  onOpenModelStorageDir,
  onOpenModels,
  importingModel,
  managingModelStorage,
  downloadingOcrModel,
  modelDownloadStatus,
  models,
  status,
  t,
}: SettingsPanelProps) {
  if (activeView === "interface") {
    return (
      <SettingsStack>
        <SettingsSection title={t("section.interface.general")}>
          <SelectField
            label={t("field.interfaceLanguage")}
            value={settings.interface.language}
            options={localeOptions}
            onValueChange={(language) => {
              updateSettings("interface", { language });
            }}
          />
        </SettingsSection>
        <SettingsSection title={t("section.interface.platform")}>
          <PlatformCapabilitiesPanel
            capabilities={status.platformCapabilities}
            t={t}
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  if (activeView === "capture") {
    return (
      <SettingsStack>
        <SettingsSection title={t("section.capture.behavior")}>
          <SwitchField
            label={t("field.includeCursor")}
            checked={settings.capture.includeCursor}
            onCheckedChange={(includeCursor) =>
              updateSettings("capture", { includeCursor })
            }
          />
          <SwitchField
            label={t("field.freezeScreenOnCapture")}
            checked={settings.capture.freezeScreenOnCapture}
            onCheckedChange={(freezeScreenOnCapture) =>
              updateSettings("capture", { freezeScreenOnCapture })
            }
          />
          <NumberField
            label={t("field.captureDelay")}
            min={0}
            max={30000}
            step={100}
            value={settings.capture.captureDelayMs}
            onValueChange={(captureDelayMs) =>
              updateSettings("capture", { captureDelayMs })
            }
          />
        </SettingsSection>
        <SettingsSection title={t("section.capture.assist")}>
          <SwitchField
            label={t("field.showSizeLabel")}
            checked={settings.capture.showSizeLabel}
            onCheckedChange={(showSizeLabel) =>
              updateSettings("capture", { showSizeLabel })
            }
          />
          <SwitchField
            label={t("field.showToolbar")}
            checked={settings.capture.showToolbar}
            onCheckedChange={(showToolbar) =>
              updateSettings("capture", { showToolbar })
            }
          />
          <SwitchField
            label={t("field.showMagnifier")}
            checked={settings.overlay.showMagnifier}
            onCheckedChange={(showMagnifier) =>
              updateSettings("overlay", { showMagnifier })
            }
          />
          <NumberField
            label={t("field.magnifierScale")}
            min={1}
            max={6}
            step={0.25}
            value={settings.overlay.magnifierScale}
            onValueChange={(magnifierScale) =>
              updateSettings("overlay", { magnifierScale })
            }
          />
          <RangeField
            label={t("field.maskOpacity")}
            min={0}
            max={0.9}
            step={0.01}
            value={settings.capture.maskOpacity}
            onValueChange={(maskOpacity) =>
              updateSettings("capture", { maskOpacity })
            }
          />
          <ColorField
            label={t("field.borderColor")}
            value={settings.capture.borderColor}
            onValueChange={(borderColor) =>
              updateSettings("capture", { borderColor })
            }
          />
        </SettingsSection>
        <SettingsSection title={t("section.capture.output")}>
          <SwitchField
            label={t("field.autoCopyToClipboard")}
            checked={settings.capture.autoCopyToClipboard}
            onCheckedChange={(autoCopyToClipboard) =>
              updateSettings("capture", { autoCopyToClipboard })
            }
          />
          <SelectField
            label={t("field.afterCapture")}
            value={settings.capture.completionAction}
            options={localizedOptions(completionActions, t)}
            onValueChange={(completionAction) =>
              updateSettings("capture", { completionAction })
            }
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  if (activeView === "pin") {
    return (
      <SettingsStack>
        <SettingsSection title={t("section.pin.behavior")}>
          <SwitchField
            label={t("field.alwaysOnTop")}
            checked={settings.pin.alwaysOnTop}
            onCheckedChange={(alwaysOnTop) =>
              updateSettings("pin", { alwaysOnTop })
            }
          />
          <SwitchField
            label={t("field.clickThrough")}
            checked={settings.pin.clickThrough}
            onCheckedChange={(clickThrough) =>
              updateSettings("pin", { clickThrough })
            }
          />
          <SwitchField
            label={t("field.rememberPosition")}
            checked={settings.pin.rememberPosition}
            onCheckedChange={(rememberPosition) =>
              updateSettings("pin", { rememberPosition })
            }
          />
        </SettingsSection>
        <SettingsSection title={t("section.pin.display")}>
          <SwitchField
            label={t("field.showOcrText")}
            checked={settings.pin.showOcrText}
            onCheckedChange={(showOcrText) =>
              updateSettings("pin", { showOcrText })
            }
          />
          <SwitchField
            label={t("field.showTranslationText")}
            checked={settings.pin.showTranslationText}
            onCheckedChange={(showTranslationText) =>
              updateSettings("pin", { showTranslationText })
            }
          />
          <RangeField
            label={t("field.defaultOpacity")}
            min={0.2}
            max={1}
            step={0.01}
            value={settings.pin.defaultOpacity}
            onValueChange={(defaultOpacity) =>
              updateSettings("pin", { defaultOpacity })
            }
          />
          <NumberField
            label={t("field.zoomStep")}
            min={0.05}
            max={0.5}
            step={0.05}
            value={settings.pin.zoomStep}
            onValueChange={(zoomStep) => updateSettings("pin", { zoomStep })}
          />
        </SettingsSection>
        <SettingsSection title={t("section.pin.size")}>
          <NumberField
            label={t("field.pinMinWidth")}
            min={16}
            max={2048}
            step={1}
            value={settings.pin.minWidth}
            onValueChange={(minWidth) => updateSettings("pin", { minWidth })}
          />
          <NumberField
            label={t("field.pinMinHeight")}
            min={16}
            max={2048}
            step={1}
            value={settings.pin.minHeight}
            onValueChange={(minHeight) => updateSettings("pin", { minHeight })}
          />
        </SettingsSection>
        <SettingsSection title={t("section.pin.ocrText")}>
          <NumberField
            label={t("field.ocrTextFontRatio")}
            min={0.1}
            max={2}
            step={0.01}
            value={settings.pin.ocrText.fontHeightRatio}
            onValueChange={(fontHeightRatio) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, fontHeightRatio },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextMinFontSize")}
            min={4}
            max={96}
            step={1}
            value={settings.pin.ocrText.minFontSize}
            onValueChange={(minFontSize) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, minFontSize },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextMaxFontSize")}
            min={4}
            max={128}
            step={1}
            value={settings.pin.ocrText.maxFontSize}
            onValueChange={(maxFontSize) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, maxFontSize },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextPaddingX")}
            min={0}
            max={32}
            step={0.5}
            value={settings.pin.ocrText.paddingX}
            onValueChange={(paddingX) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, paddingX },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextPaddingY")}
            min={0}
            max={32}
            step={0.5}
            value={settings.pin.ocrText.paddingY}
            onValueChange={(paddingY) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, paddingY },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextInteractionPaddingX")}
            min={0}
            max={48}
            step={0.5}
            value={settings.pin.ocrText.interactionPaddingX}
            onValueChange={(interactionPaddingX) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, interactionPaddingX },
              })
            }
          />
          <NumberField
            label={t("field.ocrTextInteractionPaddingY")}
            min={0}
            max={48}
            step={0.5}
            value={settings.pin.ocrText.interactionPaddingY}
            onValueChange={(interactionPaddingY) =>
              updateSettings("pin", {
                ocrText: { ...settings.pin.ocrText, interactionPaddingY },
              })
            }
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  if (activeView === "ocr") {
    const localOcrRuntimeEnabled =
      status.localOcrRuntimeStatus === "local-ocr-rs-enabled";
    const ocrModels = models.filter((model) => model.domain === "ocr");
    const readyLocalOcrModels = models.filter(
      (model) =>
        model.domain === "ocr" &&
        model.source === "local-path" &&
        model.availability === "ready",
    );
    const hasReadyLocalOcrModel = readyLocalOcrModels.length > 0;
    const ocrProvider = optionValueOrDefault(
      settings.ocr.provider,
      ocrProviders,
      "local-mnn",
    );
    const usingLocalOcr = localOcrProviders.has(ocrProvider);
    const usingApiOcr = apiOcrProviders.has(ocrProvider);
    const autoModelId =
      readyLocalOcrModels.find((model) => model.id === recommendedOcrModelId)?.id ??
      readyLocalOcrModels[0]?.id ??
      recommendedOcrModelId;
    const selectedModelId = settings.ocr.defaultModelId || autoModelId;
    const selectedModel = ocrModels.find((model) => model.id === selectedModelId);
    const manualModelSelected = Boolean(settings.ocr.defaultModelId);
    const providerRequiredBackend =
      ocrProvider === "local-mnn"
        ? "mnn"
        : ocrProvider === "local-onnx"
          ? "onnx"
          : ocrProvider === "local-paddle"
            ? "paddle"
            : null;
    const modelMatchesProvider =
      !usingLocalOcr ||
      !selectedModel ||
      !providerRequiredBackend ||
      selectedModel.backend === providerRequiredBackend;
    const selectedModelReady =
      selectedModelId.length > 0 &&
      readyLocalOcrModels.some((model) => model.id === selectedModelId);
    const autoRunDisabled =
      usingLocalOcr &&
      (!localOcrRuntimeEnabled || !selectedModelReady || !modelMatchesProvider);
    const autoRunChecked =
      settings.ocr.autoRunAfterCapture && !autoRunDisabled;
    const currentProviderProfileId =
      settings.ocr.defaultProviderProfileId || `${ocrProvider}-default`;
    const apiProfile =
      settings.ocr.providerProfiles.find(
        (profile) => profile.id === currentProviderProfileId,
      ) ?? {
        id: currentProviderProfileId,
        provider: ocrProvider,
        endpoint: "",
        model: "",
        languageHint: "",
        timeoutMs: 15000,
        retryLimit: 0,
        privacyNoticeAcknowledged: false,
      };
    const apiReady = !usingApiOcr
      ? true
      : ocrProvider === "api-custom"
        ? Boolean(apiProfile.endpoint.trim()) &&
          apiProfile.privacyNoticeAcknowledged
        : apiProfile.privacyNoticeAcknowledged;
    const providerOptions = localizedOptions(ocrProviders, t);
    const ocrModelOptions = [
      {
        value: "auto",
        label: `${t("model.auto")} (${autoModelId})`,
      },
      ...ocrModels.map((model) => ({
        value: model.id,
        label: `${model.name} (${model.backend}, ${model.availability})`,
      })),
    ];
    const updateCustomProfile = (
      values: Partial<typeof apiProfile>,
    ) => {
      const nextProfile = {
        ...apiProfile,
        ...values,
        id: values.id ?? apiProfile.id,
        provider: ocrProvider,
      };
      const others = settings.ocr.providerProfiles.filter(
        (profile) => profile.id !== apiProfile.id && profile.id !== nextProfile.id,
      );
      updateSettings("ocr", {
        providerProfiles: [...others, nextProfile],
        defaultProviderProfileId: nextProfile.id,
      });
    };
    const handleProviderChange = (provider: string) => {
      updateSettings("ocr", {
        provider,
        defaultProviderProfileId: apiOcrProviders.has(provider)
          ? `${provider}-default`
          : settings.ocr.defaultProviderProfileId,
      });
    };

    return (
      <SettingsStack>
        <SettingsSection title={t("section.ocr.general")}>
          <SelectField
            label={t("field.provider")}
            value={ocrProvider}
            options={providerOptions}
            fallbackValue="local-mnn"
            onValueChange={handleProviderChange}
          />
          {usingLocalOcr ? (
            <SelectField
              label={t("field.defaultOcrModel")}
              value={settings.ocr.defaultModelId || "auto"}
              options={ocrModelOptions}
              onValueChange={(value) => {
                updateSettings("ocr", {
                  defaultModelId: value === "auto" ? "" : value,
                });
              }}
            />
          ) : null}
          <TextField
            label={t("field.languageHint")}
            value={settings.ocr.languageHint}
            placeholder={t("placeholder.auto")}
            onValueChange={(languageHint) =>
              updateSettings("ocr", { languageHint })
            }
          />
          <SwitchField
            label={t("field.runOcrAfterCapture")}
            checked={autoRunChecked}
            disabled={autoRunDisabled}
            onCheckedChange={(autoRunAfterCapture) =>
              updateSettings("ocr", { autoRunAfterCapture })
            }
          />
        </SettingsSection>
        <SettingsSection title={t("section.ocr.default")}>
          <ModelTile
            label="OCR provider status"
            value={
              ocrProvider === "disabled"
                ? "OCR disabled"
                : usingApiOcr
                  ? apiReady
                    ? "API profile ready"
                    : "API profile missing"
                  : usingLocalOcr
                    ? selectedModelReady &&
                      localOcrRuntimeEnabled &&
                      modelMatchesProvider
                      ? "Local OCR ready"
                      : "Local OCR needs setup"
                    : "System OCR selected"
            }
            configuredLabel={
              usingApiOcr
                ? apiReady
                  ? t("model.configured")
                  : "configure API profile"
                : usingLocalOcr
                  ? selectedModelReady && modelMatchesProvider
                    ? t("model.configured")
                    : !modelMatchesProvider
                      ? "model/backend mismatch"
                      : "download model first"
                  : t("model.configured")
            }
          />
          <ModelTile
            label={t("model.ocrDefault")}
            value={
              usingApiOcr
                ? apiProfile.model || "api profile default"
                : usingLocalOcr
                  ? selectedModelId || "auto"
                  : ocrProvider
            }
            configuredLabel={
              usingApiOcr
                ? apiReady
                  ? t("model.configured")
                  : "missing API profile"
                : usingLocalOcr
                  ? selectedModelReady && modelMatchesProvider
                    ? t("model.configured")
                    : "missing local files"
                  : t("model.configured")
            }
          />
        </SettingsSection>
        {usingLocalOcr ? (
          <SettingsSection title={t("section.ocr.local")}>
            <ModelTile
              label="Local OCR runtime"
              value={status.localOcrRuntimeStatus}
              configuredLabel={
                localOcrRuntimeEnabled
                  ? "runtime enabled"
                  : "local_ocr_runtime_disabled"
              }
            />
            <ModelTile
              label="Local OCR model"
              value={
                selectedModel
                  ? `${selectedModel.name} / ${selectedModel.availability}`
                  : hasReadyLocalOcrModel
                    ? readyLocalOcrModels.map((model) => model.id).join(", ")
                    : "缺少本地 OCR 模型"
              }
              configuredLabel={
                selectedModelReady && modelMatchesProvider
                  ? manualModelSelected
                    ? "manual model ready"
                    : "auto model ready"
                  : !modelMatchesProvider
                    ? "provider backend mismatch"
                    : "go to Models to download or import"
              }
            />
            <Button
              type="button"
              variant="outline"
              className="min-h-[62px] min-w-0 justify-center"
              onClick={onOpenModels}
            >
              <Download className="size-4" />
              <span className="truncate">
                {selectedModelReady ? "管理 OCR 模型" : "去模型页下载"}
              </span>
            </Button>
          </SettingsSection>
        ) : null}
        {usingApiOcr ? (
          <SettingsSection title={t("section.ocr.api")}>
            <TextField
              label={t("field.ocrProfileId")}
              value={apiProfile.id}
              placeholder={`${settings.ocr.provider}-default`}
              onValueChange={(id) => updateCustomProfile({ id })}
            />
            <TextField
              label={t("field.customHttpEndpoint")}
              value={apiProfile.endpoint}
              placeholder={
                ocrProvider === "api-custom"
                  ? "http://127.0.0.1:8080/ocr"
                  : "optional endpoint override"
              }
              onValueChange={(endpoint) => updateCustomProfile({ endpoint })}
            />
            <TextField
              label="OCR API model"
              value={apiProfile.model}
              placeholder={t("placeholder.auto")}
              onValueChange={(model) => updateCustomProfile({ model })}
            />
            <NumberField
              label={t("field.ocrTimeout")}
              min={1000}
              max={120000}
              step={1000}
              value={apiProfile.timeoutMs}
              onValueChange={(timeoutMs) => updateCustomProfile({ timeoutMs })}
            />
            <SwitchField
              label={t("field.externalOcrPrivacy")}
              checked={apiProfile.privacyNoticeAcknowledged}
              onCheckedChange={(privacyNoticeAcknowledged) =>
                updateCustomProfile({ privacyNoticeAcknowledged })
              }
            />
          </SettingsSection>
        ) : null}
      </SettingsStack>
    );
  }

  if (activeView === "translation") {
    const localTranslateRuntimeEnabled =
      status.localTranslateRuntimeStatus === "local-translate-ct2-enabled";
    const translationModels = models.filter(
      (model) => model.domain === "translation",
    );
    const readyLocalTranslationModels = translationModels.filter(
      (model) =>
        model.source === "local-path" &&
        model.availability === "ready" &&
        model.backend === "ctranslate2",
    );
    const translationProvider = optionValueOrDefault(
      settings.translation.provider,
      translationProviders,
      "local-ct2",
    );
    const translationSegmentationMode = optionValueOrDefault(
      settings.translation.segmentationMode,
      translationSegmentationModes,
      "smart-merge",
    );
    const usingLocalTranslation =
      localTranslationProviders.has(translationProvider);
    const usingApiTranslation = apiTranslationProviders.has(translationProvider);
    const autoTranslationModelId =
      readyLocalTranslationModels.find((model) =>
        model.id.includes(settings.translation.targetLanguage),
      )?.id ??
      readyLocalTranslationModels[0]?.id ??
      "opus-mt-en-zh-ct2-int8";
    const selectedTranslationModelId =
      settings.translation.defaultModelId || autoTranslationModelId;
    const selectedTranslationModel = translationModels.find(
      (model) => model.id === selectedTranslationModelId,
    );
    const selectedTranslationModelReady = readyLocalTranslationModels.some(
      (model) => model.id === selectedTranslationModelId,
    );
    const autoTranslateDisabled =
      usingLocalTranslation &&
      (!localTranslateRuntimeEnabled || !selectedTranslationModelReady);
    const autoTranslateChecked =
      settings.translation.autoTranslateAfterOcr && !autoTranslateDisabled;
    const translationModelOptions = [
      {
        value: "auto",
        label: `${t("model.auto")} (${autoTranslationModelId})`,
      },
      ...translationModels.map((model) => ({
        value: model.id,
        label: `${model.name} (${model.backend}, ${model.availability})`,
      })),
    ];

    return (
      <SettingsStack>
        <SettingsSection title={t("section.translation.general")}>
          <SelectField
            label={t("field.provider")}
            value={translationProvider}
            options={localizedOptions(translationProviders, t)}
            fallbackValue="local-ct2"
            onValueChange={(provider) =>
              updateSettings("translation", { provider })
            }
          />
          <SelectField
            label={t("field.targetLanguage")}
            value={settings.translation.targetLanguage}
            options={targetLanguages}
            onValueChange={(targetLanguage) =>
              updateSettings("translation", { targetLanguage })
            }
          />
          <SelectField
            label={t("field.translationSegmentation")}
            value={translationSegmentationMode}
            options={localizedOptions(translationSegmentationModes, t)}
            fallbackValue="smart-merge"
            onValueChange={(segmentationMode) =>
              updateSettings("translation", { segmentationMode })
            }
          />
          {usingLocalTranslation ? (
            <SelectField
              label="Default translation model"
              value={settings.translation.defaultModelId || "auto"}
              options={translationModelOptions}
              onValueChange={(value) => {
                updateSettings("translation", {
                  defaultModelId: value === "auto" ? "" : value,
                });
              }}
            />
          ) : null}
          <SwitchField
            label={t("field.translateAfterOcr")}
            checked={autoTranslateChecked}
            disabled={autoTranslateDisabled}
            onCheckedChange={(autoTranslateAfterOcr) =>
              updateSettings("translation", { autoTranslateAfterOcr })
            }
          />
        </SettingsSection>
        {translationSegmentationMode === "smart-merge" ? (
          <SettingsSection title={t("section.translation.smartMerge")}>
            <NumberField
              label={t("field.smartMergeEdgeTolerance")}
              min={0.2}
              max={6}
              step={0.05}
              value={settings.translation.smartMerge.edgeToleranceLines}
              onValueChange={(edgeToleranceLines) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    edgeToleranceLines,
                  },
                })
              }
            />
            <NumberField
              label={t("field.smartMergeLooseEdgeTolerance")}
              min={0.2}
              max={8}
              step={0.05}
              value={settings.translation.smartMerge.looseEdgeToleranceLines}
              onValueChange={(looseEdgeToleranceLines) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    looseEdgeToleranceLines,
                  },
                })
              }
            />
            <NumberField
              label={t("field.smartMergeHeightRatio")}
              min={1}
              max={4}
              step={0.05}
              value={settings.translation.smartMerge.heightRatioLimit}
              onValueChange={(heightRatioLimit) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    heightRatioLimit,
                  },
                })
              }
            />
            <NumberField
              label={t("field.smartMergeLongerLineRatio")}
              min={1}
              max={4}
              step={0.05}
              value={settings.translation.smartMerge.longerLineRatio}
              onValueChange={(longerLineRatio) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    longerLineRatio,
                  },
                })
              }
            />
            <NumberField
              label={t("field.smartMergeShortLastLineRatio")}
              min={0.1}
              max={1}
              step={0.05}
              value={settings.translation.smartMerge.shortLastLineRatio}
              onValueChange={(shortLastLineRatio) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    shortLastLineRatio,
                  },
                })
              }
            />
            <NumberField
              label={t("field.smartMergeInlineLabelMaxChars")}
              min={1}
              max={120}
              step={1}
              value={settings.translation.smartMerge.inlineLabelMaxChars}
              onValueChange={(inlineLabelMaxChars) =>
                updateSettings("translation", {
                  smartMerge: {
                    ...settings.translation.smartMerge,
                    inlineLabelMaxChars,
                  },
                })
              }
            />
          </SettingsSection>
        ) : null}
        {usingApiTranslation ? (
          <SettingsSection title={t("section.translation.api")}>
            <ModelTile
              label="Translation API"
              value="External API provider"
              configuredLabel="scheduled after local CTranslate2 MVP"
            />
          </SettingsSection>
        ) : null}
        {usingLocalTranslation ? (
          <SettingsSection title={t("section.translation.local")}>
            <ModelTile
              label="Local translation runtime"
              value={status.localTranslateRuntimeStatus}
              configuredLabel={
                localTranslateRuntimeEnabled
                  ? "runtime enabled"
                  : "rebuild with local-translate-ct2"
              }
            />
            <ModelTile
              label="Local translation model"
              value={
                selectedTranslationModel
                  ? `${selectedTranslationModel.name} / ${selectedTranslationModel.availability}`
                  : "缺少本地翻译模型"
              }
              configuredLabel={
                selectedTranslationModelReady
                  ? "local CTranslate2 model ready"
                  : "import CTranslate2 model first"
              }
            />
            <Button
              type="button"
              variant="outline"
              className="min-h-[62px] min-w-0 justify-center"
              onClick={onOpenModels}
            >
              <Upload className="size-4" />
              <span className="truncate">
                {selectedTranslationModelReady ? "管理翻译模型" : "导入翻译模型"}
              </span>
            </Button>
          </SettingsSection>
        ) : null}
        <SettingsSection title={t("section.translation.default")}>
          <ModelTile
            label={t("model.translationDefault")}
            value={selectedTranslationModelId}
            configuredLabel={
              usingLocalTranslation
                ? selectedTranslationModelReady
                  ? t("model.configured")
                  : "missing local files"
                : "not available in local-first MVP"
            }
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  if (activeView === "models") {
    const downloadRunning = Boolean(modelDownloadStatus?.running);

    return (
      <SettingsStack>
        <SettingsSection title={t("section.models.storage")}>
          <ModelStorageTile
            info={modelStorageInfo}
            path={modelStoragePath}
            busy={managingModelStorage}
            downloadRunning={downloadRunning}
            onPathChange={onModelStoragePathChange}
            onChoose={onChooseModelStorageDir}
            onApply={onApplyModelStorageDir}
            onOpen={onOpenModelStorageDir}
          />
        </SettingsSection>
        <SettingsSection title={t("section.models.downloads")}>
          {builtinOcrModelPackages.map((modelPackage) => {
            const model = models.find((item) => item.id === modelPackage.id);
            const ready = model?.availability === "ready";
            const runningThisModel =
              downloadRunning && modelDownloadStatus?.modelId === modelPackage.id;

            return (
              <DownloadableModelTile
                key={modelPackage.id}
                domain={modelPackage.domain}
                title={modelPackage.name}
                profile={modelPackage.profile}
                detail={modelPackage.detail}
                status={
                  ready
                    ? "ready"
                    : model?.availability ?? "not-downloaded"
                }
                source={model?.packageSource ?? "ocr-rs PaddleOCR MNN model package"}
                path={model?.path}
                disabled={ready || downloadRunning || downloadingOcrModel}
                running={runningThisModel}
                onDownload={() => onDownloadOcrModel(modelPackage.id)}
              />
            );
          })}
          {builtinTranslationModelPackages.map((modelPackage) => {
            const model = models.find((item) => item.id === modelPackage.id);
            const ready = model?.availability === "ready";
            const runningThisModel =
              downloadRunning && modelDownloadStatus?.modelId === modelPackage.id;

            return (
              <DownloadableModelTile
                key={modelPackage.id}
                domain={modelPackage.domain}
                title={modelPackage.name}
                profile={modelPackage.profile}
                detail={modelPackage.detail}
                status={ready ? "ready" : model?.availability ?? "not-downloaded"}
                source={model?.packageSource ?? modelPackage.source}
                path={model?.path}
                disabled={ready || downloadRunning || downloadingOcrModel}
                running={runningThisModel}
                onDownload={() => onDownloadOcrModel(modelPackage.id)}
              />
            );
          })}
        </SettingsSection>
        <SettingsSection title={t("section.models.progress")}>
          <ModelDownloadTile
            status={modelDownloadStatus}
            onCancel={onCancelModelDownload}
          />
        </SettingsSection>
        <SettingsSection title={t("section.models.import")}>
          <TextField
            label={t("field.modelManifestPath")}
            value={modelManifestPath}
            placeholder="D:\\models\\ppocr-v5-mobile-mnn\\manifest.toml"
            onValueChange={onModelManifestPathChange}
          />
          <Button
            type="button"
            variant="outline"
            className="min-h-[62px] min-w-0 justify-center"
            disabled={!modelManifestPath.trim() || importingModel}
            onClick={onImportModel}
          >
            {importingModel ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Upload className="size-4" />
            )}
            <span className="truncate">{t("button.importModel")}</span>
          </Button>
        </SettingsSection>
        {showModelRegistryDetails ? (
          <SettingsSection title={t("section.models.dev")}>
            {models.map((model) => (
              <ModelTile
                key={model.id}
                label={model.domain}
                value={`${model.name} / ${model.id}`}
                configuredLabel={`${model.backend} / ${model.source} / ${model.availability}${model.packageSource ? ` / ${model.packageSource}` : ""}`}
              />
            ))}
            {models.length === 0 ? (
              <ModelTile
                label={t("view.models")}
                value={t("events.none")}
                configuredLabel={t("model.auto")}
              />
            ) : null}
          </SettingsSection>
        ) : null}
      </SettingsStack>
    );
  }

  if (activeView === "hotkeys") {
    return (
      <SettingsStack>
        <SettingsSection title={t("section.hotkeys.general")}>
          <HotkeyField
            label={t("field.captureHotkey")}
            value={settings.hotkeys.capture}
            onValueChange={(capture) => updateSettings("hotkeys", { capture })}
            t={t}
          />
          <HotkeyField
            label={t("field.pinSelection")}
            value={settings.hotkeys.pinSelection}
            onValueChange={(pinSelection) =>
              updateSettings("hotkeys", { pinSelection })
            }
            t={t}
          />
          <HotkeyField
            label={t("field.toggleClickThrough")}
            value={settings.hotkeys.togglePinsClickThrough}
            onValueChange={(togglePinsClickThrough) =>
              updateSettings("hotkeys", { togglePinsClickThrough })
            }
            t={t}
          />
          <HotkeyField
            label={t("field.showHistory")}
            value={settings.hotkeys.showHistory}
            onValueChange={(showHistory) =>
              updateSettings("hotkeys", { showHistory })
            }
            t={t}
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  if (activeView === "history") {
    return (
      <SettingsStack>
        <SettingsSection title={t("section.history.general")}>
          <SwitchField
            label={t("field.enableHistory")}
            checked={settings.history.enabled}
            onCheckedChange={(enabled) => updateSettings("history", { enabled })}
          />
          <NumberField
            label={t("field.maxEntries")}
            min={10}
            max={10000}
            step={10}
            value={settings.history.maxEntries}
            onValueChange={(maxEntries) =>
              updateSettings("history", { maxEntries })
            }
          />
        </SettingsSection>
      </SettingsStack>
    );
  }

  return null;
}

function ModelStorageTile({
  info,
  path,
  busy,
  downloadRunning,
  onPathChange,
  onChoose,
  onApply,
  onOpen,
}: {
  info: ModelStorageInfo | null;
  path: string;
  busy: boolean;
  downloadRunning: boolean;
  onPathChange: (value: string) => void;
  onChoose: () => void;
  onApply: () => void;
  onOpen: () => void;
}) {
  const currentPath = info?.currentOcrModelsDir ?? path;
  const defaultPath = info?.defaultOcrModelsDir ?? "";
  const applyDisabled =
    busy || downloadRunning || !path.trim() || path.trim() === currentPath;

  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-4 md:col-span-2">
      <div className="flex min-w-0 flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="break-words text-xs font-medium text-muted-foreground">
            模型默认下载位置
          </p>
          <p className="mt-2 break-all text-sm font-semibold">
            {currentPath || "加载中..."}
          </p>
          <p className="mt-2 break-all text-xs text-muted-foreground">
            默认位置：{defaultPath || "加载中..."}
          </p>
        </div>
        <span className="rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">
          {info?.usingDefault ? "默认" : "自定义"}
        </span>
      </div>
      <div className="mt-4 flex min-w-0 flex-col gap-2 lg:flex-row">
        <Input
          value={path}
          placeholder={defaultPath}
          onChange={(event) => onPathChange(event.target.value)}
        />
        <div className="flex shrink-0 flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            className="min-w-0"
            disabled={busy || downloadRunning}
            onClick={onChoose}
          >
            {busy ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <FolderOpen className="size-4" />
            )}
            <span className="truncate">选择位置</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            className="min-w-0"
            disabled={applyDisabled}
            onClick={onApply}
          >
            <span className="truncate">应用</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            className="min-w-0"
            disabled={busy || !currentPath}
            onClick={onOpen}
          >
            <FolderOpen className="size-4" />
            <span className="truncate">打开位置</span>
          </Button>
        </div>
      </div>
      <p className="mt-3 break-words text-xs text-muted-foreground">
        更改位置时，已下载模型会迁移到新目录；下载进行中不能修改。
      </p>
    </div>
  );
}

function PlatformCapabilitiesPanel({
  capabilities,
  t,
}: {
  capabilities: PlatformCapabilities;
  t: Translator;
}) {
  const entries = [
    [t("capability.screenCapture"), capabilities.screenCapture],
    [t("capability.overlayWindow"), capabilities.overlayWindow],
    [t("capability.pinWindow"), capabilities.pinWindow],
    [t("capability.systemOcr"), capabilities.systemOcr],
    [t("capability.clipboardRead"), capabilities.clipboardRead],
    [t("capability.clipboardWrite"), capabilities.clipboardWrite],
    [t("capability.globalHotkey"), capabilities.globalHotkey],
    [t("capability.fileDialog"), capabilities.fileDialog],
    [t("capability.sharedMemory"), capabilities.sharedMemory],
    [t("capability.secureStorage"), capabilities.secureStorage],
  ] satisfies Array<[string, CapabilityStatus]>;
  const supportedCount = entries.filter(
    ([, capability]) => capability.status === "supported",
  ).length;

  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-4 md:col-span-2">
      <div className="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="break-words text-xs font-medium text-muted-foreground">
            {t("platform.capabilitiesTitle")}
          </p>
          <p className="mt-2 break-words text-sm font-semibold">
            {t("platform.runtimeStatus")}
          </p>
        </div>
        <span className="rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">
          {t("platform.supportedCount", {
            supported: supportedCount,
            total: entries.length,
          })}
        </span>
      </div>
      <div className="mt-4 grid min-w-0 gap-2 md:grid-cols-2">
        {entries.map(([label, capability]) => (
          <CapabilityRow
            key={label}
            label={label}
            capability={capability}
            t={t}
          />
        ))}
      </div>
    </div>
  );
}

function CapabilityRow({
  label,
  capability,
  t,
}: {
  label: string;
  capability: CapabilityStatus;
  t: Translator;
}) {
  return (
    <div className="min-w-0 rounded-md border bg-card px-3 py-2">
      <div className="flex min-w-0 items-center justify-between gap-3">
        <span className="min-w-0 truncate text-sm font-medium">{label}</span>
        <span className={capabilityStatusClassName(capability.status)}>
          {formatCapabilityStatus(capability.status, t)}
        </span>
      </div>
      {capability.reason ? (
        <p className="mt-2 break-words text-xs text-muted-foreground">
          {formatCapabilityReason(capability.reason, t)}
        </p>
      ) : null}
      {capability.action ? (
        <p className="mt-1 break-words text-xs font-medium text-primary">
          {capability.action}
        </p>
      ) : null}
    </div>
  );
}

function ModelDownloadTile({
  status,
  onCancel,
}: {
  status: ModelDownloadStatus | null;
  onCancel: () => void;
}) {
  const downloaded = formatBytes(status?.downloadedBytes ?? 0);
  const total = status?.totalBytes ? formatBytes(status.totalBytes) : "unknown";
  const percent =
    typeof status?.percent === "number" ? `${status.percent.toFixed(1)}%` : "";
  const filePosition =
    status && status.fileCount > 0
      ? `${Math.min(status.fileIndex + 1, status.fileCount)}/${status.fileCount}`
      : "";

  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-4">
      <div className="flex min-w-0 items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="break-words text-xs font-medium text-muted-foreground">
            模型下载
          </p>
          <p className="mt-2 break-all text-sm font-semibold">
            {status?.running
              ? `${filePosition} ${status.fileName || status.role}`
              : status?.error
                ? "下载失败，可重试"
                : status?.result
                  ? "下载完成"
                  : "可直接下载内置来源模型"}
          </p>
        </div>
        {status?.running ? (
          <Button
            type="button"
            variant="outline"
            size="icon"
            title="取消下载"
            onClick={onCancel}
          >
            <Square className="size-4" />
          </Button>
        ) : null}
      </div>
      <div className="mt-4 h-2 overflow-hidden rounded-sm bg-muted">
        <div
          className="h-full bg-primary transition-all"
          style={{ width: `${Math.max(0, Math.min(status?.percent ?? 0, 100))}%` }}
        />
      </div>
      <p className="mt-3 break-words text-xs text-muted-foreground">
        {status?.running
          ? `${downloaded} / ${total}${percent ? ` · ${percent}` : ""}`
          : "下载成功后会自动注册，并设为对应功能的默认模型。"}
      </p>
      {status?.error ? (
        <p className="mt-2 break-words text-xs text-destructive">{status.error}</p>
      ) : null}
    </div>
  );
}

function DownloadableModelTile({
  domain,
  title,
  profile,
  detail,
  status,
  source,
  path,
  disabled,
  running,
  onDownload,
}: {
  domain: string;
  title: string;
  profile: string;
  detail: string;
  status: string;
  source: string;
  path?: string;
  disabled: boolean;
  running: boolean;
  onDownload: () => void;
}) {
  const ready = status === "ready";

  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-4">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="break-words text-xs font-medium text-muted-foreground">
            {domain} {profile}模型
          </p>
          <p className="mt-2 break-words text-sm font-semibold">{title}</p>
          <p className="mt-2 break-words text-xs text-muted-foreground">{detail}</p>
        </div>
        {ready ? (
          <CheckCircle2 className="size-4 shrink-0 text-emerald-600" />
        ) : (
          <AlertCircle className="size-4 shrink-0 text-muted-foreground" />
        )}
      </div>
      <div className="mt-4 flex min-w-0 flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="outline"
          className="min-w-0"
          disabled={disabled}
          onClick={onDownload}
        >
          {running ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Download className="size-4" />
          )}
          <span className="truncate">{ready ? "已就绪" : "下载"}</span>
        </Button>
        <span className="rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">
          {status}
        </span>
      </div>
      <p className="mt-3 break-words text-xs text-muted-foreground">{source}</p>
      {path ? (
        <p className="mt-2 break-all text-xs text-muted-foreground">{path}</p>
      ) : null}
    </div>
  );
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function formatCapabilityStatus(
  status: CapabilityStatus["status"],
  t: Translator,
) {
  switch (status) {
    case "supported":
      return t("capabilityStatus.supported");
    case "degraded":
      return t("capabilityStatus.degraded");
    case "needsSetup":
      return t("capabilityStatus.needsSetup");
    case "permissionDenied":
      return t("capabilityStatus.permissionDenied");
    case "unavailable":
      return t("capabilityStatus.unavailable");
    default:
      return status;
  }
}

function formatCapabilityReason(reason: string, t: Translator) {
  switch (reason) {
    case "platform capabilities have not loaded yet":
      return t("platform.reason.notLoaded");
    case "secure storage has not been implemented for Windows":
      return t("platform.reason.secureStorageWindowsMissing");
    case "Win32 platform capabilities are available only on Windows":
      return t("platform.reason.win32Only");
    case "Windows system OCR is available only on Windows":
      return t("platform.reason.windowsOcrOnly");
    default:
      break;
  }

  const unsupportedPlatform = reason.match(
    /^(.+) platform support has not been implemented yet$/,
  );
  if (unsupportedPlatform) {
    const platform = unsupportedPlatform[1];
    return platform === "This"
      ? t("platform.reason.currentPlatformMissing")
      : t("platform.reason.platformMissing", { platform });
  }

  return reason;
}

function capabilityStatusClassName(status: CapabilityStatus["status"]) {
  const base =
    "shrink-0 rounded-sm border px-2 py-1 text-xs font-medium leading-none";

  switch (status) {
    case "supported":
      return `${base} border-emerald-200 bg-emerald-50 text-emerald-700`;
    case "degraded":
      return `${base} border-amber-200 bg-amber-50 text-amber-700`;
    case "needsSetup":
      return `${base} border-sky-200 bg-sky-50 text-sky-700`;
    case "permissionDenied":
      return `${base} border-destructive/25 bg-destructive/10 text-destructive`;
    case "unavailable":
      return `${base} border-muted bg-muted text-muted-foreground`;
    default:
      return `${base} border-muted bg-muted text-muted-foreground`;
  }
}
