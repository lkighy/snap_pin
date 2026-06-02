import { AppSettings, ModelSummary } from "@/lib/tauri";
import { localeOptions, Translator } from "@/i18n";
import { Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  completionActions,
  ocrModes,
  ocrProviders,
  targetLanguages,
  translationProviders,
  ViewId,
} from "@/app-data";
import {
  ColorField,
  FieldGrid,
  HotkeyField,
  localizedOptions,
  ModelTile,
  NumberField,
  RangeField,
  SelectField,
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
  onImportModel: () => void;
  importingModel: boolean;
  models: ModelSummary[];
  t: Translator;
}

// Each branch mirrors one navigation tab, keeping field wiring close to the settings shape.
export function SettingsPanel({
  activeView,
  settings,
  updateSettings,
  modelManifestPath,
  onModelManifestPathChange,
  onImportModel,
  importingModel,
  models,
  t,
}: SettingsPanelProps) {
  if (activeView === "interface") {
    return (
      <FieldGrid>
        <SelectField
          label={t("field.interfaceLanguage")}
          value={settings.interface.language}
          options={localeOptions}
          onValueChange={(language) => {
            updateSettings("interface", { language });
          }}
        />
      </FieldGrid>
    );
  }

  if (activeView === "capture") {
    return (
      <FieldGrid>
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
        <SwitchField
          label={t("field.autoCopyToClipboard")}
          checked={settings.capture.autoCopyToClipboard}
          onCheckedChange={(autoCopyToClipboard) =>
            updateSettings("capture", { autoCopyToClipboard })
          }
        />
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
        <SelectField
          label={t("field.afterCapture")}
          value={settings.capture.completionAction}
          options={localizedOptions(completionActions, t)}
          onValueChange={(completionAction) =>
            updateSettings("capture", { completionAction })
          }
        />
      </FieldGrid>
    );
  }

  if (activeView === "pin") {
    return (
      <FieldGrid>
        <SwitchField
          label={t("field.alwaysOnTop")}
          checked={settings.pin.alwaysOnTop}
          onCheckedChange={(alwaysOnTop) => updateSettings("pin", { alwaysOnTop })}
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
      </FieldGrid>
    );
  }

  if (activeView === "ocr") {
    const ocrModelOptions = [
      { value: "auto", label: t("model.auto") },
      ...models
        .filter((model) => model.domain === "ocr")
        .map((model) => ({
          value: model.id,
          label: `${model.name} (${model.backend}, ${model.availability})`,
        })),
    ];
    const defaultProfileId =
      settings.ocr.defaultProviderProfileId || "custom-http";
    const customProfile =
      settings.ocr.providerProfiles.find(
        (profile) => profile.id === defaultProfileId,
      ) ?? {
        id: defaultProfileId,
        provider: "api-custom",
        endpoint: "",
        model: "",
        languageHint: "",
        timeoutMs: 15000,
        retryLimit: 0,
        privacyNoticeAcknowledged: false,
      };
    const updateCustomProfile = (
      values: Partial<typeof customProfile>,
    ) => {
      const nextProfile = {
        ...customProfile,
        ...values,
        id: values.id ?? customProfile.id,
        provider: "api-custom",
      };
      const others = settings.ocr.providerProfiles.filter(
        (profile) => profile.id !== customProfile.id && profile.id !== nextProfile.id,
      );
      updateSettings("ocr", {
        providerProfiles: [...others, nextProfile],
        defaultProviderProfileId: nextProfile.id,
      });
    };

    return (
      <FieldGrid>
        <SelectField
          label={t("field.ocrMode")}
          value={settings.ocr.mode}
          options={localizedOptions(ocrModes, t)}
          onValueChange={(mode) => updateSettings("ocr", { mode })}
        />
        <SelectField
          label={t("field.provider")}
          value={settings.ocr.provider}
          options={localizedOptions(ocrProviders, t)}
          onValueChange={(provider) => updateSettings("ocr", { provider })}
        />
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
          checked={settings.ocr.autoRunAfterCapture}
          onCheckedChange={(autoRunAfterCapture) =>
            updateSettings("ocr", { autoRunAfterCapture })
          }
        />
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
        <TextField
          label={t("field.ocrProfileId")}
          value={customProfile.id}
          placeholder="custom-http"
          onValueChange={(id) => updateCustomProfile({ id })}
        />
        <TextField
          label={t("field.customHttpEndpoint")}
          value={customProfile.endpoint}
          placeholder="http://127.0.0.1:8080/ocr"
          onValueChange={(endpoint) => updateCustomProfile({ endpoint })}
        />
        <NumberField
          label={t("field.ocrTimeout")}
          min={1000}
          max={120000}
          step={1000}
          value={customProfile.timeoutMs}
          onValueChange={(timeoutMs) => updateCustomProfile({ timeoutMs })}
        />
        <SwitchField
          label={t("field.externalOcrPrivacy")}
          checked={customProfile.privacyNoticeAcknowledged}
          onCheckedChange={(privacyNoticeAcknowledged) =>
            updateCustomProfile({ privacyNoticeAcknowledged })
          }
        />
        <ModelTile
          label={t("model.ocrDefault")}
          value="ppocr-v5-mobile-mnn"
          configuredLabel={t("model.configured")}
        />
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
      </FieldGrid>
    );
  }

  if (activeView === "translation") {
    return (
      <FieldGrid>
        <SelectField
          label={t("field.provider")}
          value={settings.translation.provider}
          options={localizedOptions(translationProviders, t)}
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
        <SwitchField
          label={t("field.translateAfterOcr")}
          checked={settings.translation.autoTranslateAfterOcr}
          onCheckedChange={(autoTranslateAfterOcr) =>
            updateSettings("translation", { autoTranslateAfterOcr })
          }
        />
        <ModelTile
          label={t("model.translationDefault")}
          value="opus-mt-en-zh-ct2-int8"
          configuredLabel={t("model.configured")}
        />
      </FieldGrid>
    );
  }

  if (activeView === "models") {
    return (
      <FieldGrid>
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
      </FieldGrid>
    );
  }

  if (activeView === "hotkeys") {
    return (
      <FieldGrid>
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
      </FieldGrid>
    );
  }

  if (activeView === "history") {
    return (
      <FieldGrid>
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
      </FieldGrid>
    );
  }

  return null;
}
