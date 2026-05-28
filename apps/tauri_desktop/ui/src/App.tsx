import { FormEvent, KeyboardEvent, useEffect, useMemo, useState } from "react";
import {
  Camera,
  ClipboardList,
  Gauge,
  History,
  Keyboard,
  Languages,
  Loader2,
  Pin,
  RefreshCw,
  Save,
  Settings2,
  Sparkles,
  Wand2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import {
  AppSettings,
  AppStatus,
  getAppStatus,
  getSettings,
  runMvpFlow,
  saveSettings,
  startCapture,
} from "@/lib/tauri";
import {
  getInitialLocale,
  localeOptions,
  Locale,
  translate,
  translateStatusSummary,
  TranslationKey,
  Translator,
} from "@/i18n";

type ViewId =
  | "interface"
  | "capture"
  | "pin"
  | "ocr"
  | "translation"
  | "hotkeys"
  | "history"
  | "models";

const navItems = [
  { id: "interface", labelKey: "view.interface", icon: Settings2 },
  { id: "capture", labelKey: "view.capture", icon: Camera },
  { id: "pin", labelKey: "view.pin", icon: Pin },
  { id: "ocr", labelKey: "view.ocr", icon: ClipboardList },
  { id: "translation", labelKey: "view.translation", icon: Languages },
  { id: "hotkeys", labelKey: "view.hotkeys", icon: Keyboard },
  { id: "history", labelKey: "view.history", icon: History },
  { id: "models", labelKey: "view.models", icon: Gauge },
] satisfies Array<{ id: ViewId; labelKey: TranslationKey; icon: typeof Camera }>;

const completionActions = [
  { value: "pin", labelKey: "completion.pin" },
  { value: "copy", labelKey: "completion.copy" },
  { value: "save", labelKey: "completion.save" },
  { value: "editor", labelKey: "completion.editor" },
] satisfies Array<{ value: string; labelKey: TranslationKey }>;

const ocrProviders = [
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

const translationProviders = [
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

const targetLanguages = [
  { value: "zh-CN", label: "zh-CN" },
  { value: "en", label: "en" },
  { value: "ja", label: "ja" },
  { value: "ko", label: "ko" },
  { value: "fr", label: "fr" },
  { value: "de", label: "de" },
];

const defaultStatus: AppStatus = {
  bootSummary: "Starting desktop shell...",
  modelSummary: "Loading...",
  historySummary: "Loading...",
};

type SaveStatusKey =
  | "state.ready"
  | "state.loaded"
  | "state.saving"
  | "state.saved"
  | "state.loadFailed"
  | "state.saveFailed";

export function App() {
  const [locale, setLocale] = useState<Locale>(() => getInitialLocale());
  const [activeView, setActiveView] = useState<ViewId>("capture");
  const [status, setStatus] = useState<AppStatus>(defaultStatus);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [eventLog, setEventLog] = useState<string[]>([]);
  const [saveStatus, setSaveStatus] = useState<SaveStatusKey>("state.ready");
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);

  const t = useMemo<Translator>(
    () => (key, values) => translate(locale, key, values),
    [locale],
  );

  const activeTitle = useMemo(
    () =>
      t(
        navItems.find((item) => item.id === activeView)?.labelKey ??
          "status.settings",
      ),
    [activeView, t],
  );

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = t("app.name");
  }, [locale, t]);

  useEffect(() => {
    if (settings?.interface.language && settings.interface.language !== locale) {
      setLocale(settings.interface.language as Locale);
    }
  }, [settings?.interface.language, locale]);

  async function refreshStatus() {
    const nextStatus = await getAppStatus();
    setStatus(nextStatus);
  }

  async function loadSettings() {
    const nextSettings = await getSettings();
    setSettings(nextSettings);
    setLocale(nextSettings.interface.language as Locale);
    setSaveStatus("state.loaded");
  }

  async function refreshAll() {
    setError(null);
    setBusyAction("refresh");
    try {
      await Promise.all([refreshStatus(), loadSettings()]);
    } catch (caught) {
      setError(String(caught));
      setSaveStatus("state.loadFailed");
    } finally {
      setBusyAction(null);
    }
  }

  useEffect(() => {
    void refreshAll();
  }, []);

  function updateSettings<K extends keyof AppSettings>(
    section: K,
    values: Partial<AppSettings[K]>,
  ) {
    setSettings((current) =>
      current
        ? {
            ...current,
            [section]: {
              ...current[section],
              ...values,
            },
          }
        : current,
    );
  }

  async function persistSettings(nextSettings: AppSettings) {
    setError(null);
    setSaveStatus("state.saving");
    setBusyAction("save");
    try {
      const saved = await saveSettings(nextSettings);
      setSettings(saved);
      setSaveStatus("state.saved");
    } catch (caught) {
      setError(String(caught));
      setSaveStatus("state.saveFailed");
    } finally {
      setBusyAction(null);
    }
  }

  async function handleLocaleChange(nextLocale: Locale) {
    setLocale(nextLocale);
    if (!settings) {
      updateSettings("interface", { language: nextLocale });
      return;
    }

    const nextSettings = {
      ...settings,
      interface: {
        ...settings.interface,
        language: nextLocale,
      },
    };
    setSettings(nextSettings);
    await persistSettings(nextSettings);
  }

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!settings) {
      return;
    }

    await persistSettings(settings);
  }

  async function handleStartCapture() {
    setError(null);
    setBusyAction("capture");
    try {
      const response = await startCapture();
      setEventLog(response.events);
      await refreshStatus();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleRunMvpFlow() {
    setError(null);
    setBusyAction("mvp");
    try {
      const response = await runMvpFlow();
      setEventLog(response.events);
      if (response.historySummary) {
        setStatus((current) => ({
          ...current,
          historySummary: response.historySummary ?? current.historySummary,
        }));
      }
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  const isBusy = busyAction !== null;

  return (
    <main className="min-h-screen min-w-[860px] bg-background text-foreground">
      <header className="flex h-[72px] items-center justify-between border-b bg-card px-5">
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-lg border bg-primary text-primary-foreground shadow-sm">
            <Sparkles className="size-5" />
          </div>
          <div>
            <h1 className="text-xl font-semibold leading-tight tracking-normal">
              {t("app.name")}
            </h1>
            <p className="mt-1 max-w-[520px] truncate text-sm text-muted-foreground">
              {translateStatusSummary(status.bootSummary, locale, t)}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <LanguageSelect locale={locale} onLocaleChange={handleLocaleChange} t={t} />
          <Button
            type="button"
            variant="outline"
            onClick={handleStartCapture}
            disabled={isBusy}
          >
            {busyAction === "capture" ? (
              <Loader2 className="animate-spin" />
            ) : (
              <Camera />
            )}
            {t("button.startCapture")}
          </Button>
          <Button type="button" onClick={handleRunMvpFlow} disabled={isBusy}>
            {busyAction === "mvp" ? (
              <Loader2 className="animate-spin" />
            ) : (
              <Wand2 />
            )}
            {t("button.runOcrTranslate")}
          </Button>
        </div>
      </header>

      <div className="grid min-h-[calc(100vh-72px)] grid-cols-[212px_1fr]">
        <aside className="border-r bg-muted/35 p-3">
          <nav className="grid gap-1" aria-label={t("nav.settings")}>
            {navItems.map((item) => {
              const Icon = item.icon;
              const selected = activeView === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={cn(
                    "flex h-9 w-full items-center gap-3 rounded-md px-3 text-left text-sm font-medium text-muted-foreground transition-colors hover:bg-background hover:text-foreground",
                    selected &&
                      "bg-background text-foreground shadow-sm ring-1 ring-border",
                  )}
                  onClick={() => setActiveView(item.id)}
                >
                  <Icon className="size-4" />
                  <span>{t(item.labelKey)}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        <section className="grid content-start gap-4 p-4">
          <div className="grid grid-cols-[1.2fr_1fr_0.8fr] gap-3">
            <StatusCard
              label={t("status.models")}
              value={translateStatusSummary(status.modelSummary, locale, t)}
            />
            <StatusCard
              label={t("status.history")}
              value={translateStatusSummary(status.historySummary, locale, t)}
            />
            <StatusCard
              label={t("status.settings")}
              value={t(saveStatus)}
              tone={
                saveStatus === "state.loadFailed" ||
                saveStatus === "state.saveFailed"
                  ? "danger"
                  : "default"
              }
            />
          </div>

          {error ? (
            <div className="rounded-md border border-destructive/25 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          ) : null}

          <form onSubmit={handleSave}>
            <Card className="min-h-[360px]">
              <CardHeader className="flex-row items-center justify-between space-y-0">
                <div>
                  <CardTitle>{activeTitle}</CardTitle>
                  <CardDescription>
                    {settings ? t("state.ready") : t("state.loading")}
                  </CardDescription>
                </div>
                {activeView === "models" ? (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={refreshAll}
                    disabled={isBusy}
                  >
                    {busyAction === "refresh" ? (
                      <Loader2 className="animate-spin" />
                    ) : (
                      <RefreshCw />
                    )}
                    {t("button.refresh")}
                  </Button>
                ) : (
                  <Button type="submit" disabled={!settings || isBusy}>
                    {busyAction === "save" ? (
                      <Loader2 className="animate-spin" />
                    ) : (
                      <Save />
                    )}
                    {t("button.saveSettings")}
                  </Button>
                )}
              </CardHeader>
              <CardContent>
                {settings ? (
                  <SettingsPanel
                    activeView={activeView}
                    settings={settings}
                    updateSettings={updateSettings}
                    t={t}
                  />
                ) : (
                  <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
                    <Loader2 className="mr-2 size-4 animate-spin" />
                    {t("state.loadingSettings")}
                  </div>
                )}
              </CardContent>
            </Card>
          </form>

          <Card>
            <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
              <CardTitle>{t("events.title")}</CardTitle>
              <Badge variant="outline">
                {t("events.entries", { count: eventLog.length })}
              </Badge>
            </CardHeader>
            <CardContent>
              <pre className="max-h-52 min-h-32 overflow-auto rounded-md bg-zinc-950 p-3 text-xs leading-6 text-zinc-100">
                {eventLog.length > 0 ? eventLog.join("\n") : t("events.none")}
              </pre>
            </CardContent>
          </Card>
        </section>
      </div>
    </main>
  );
}

interface SettingsPanelProps {
  activeView: ViewId;
  settings: AppSettings;
  updateSettings: <K extends keyof AppSettings>(
    section: K,
    values: Partial<AppSettings[K]>,
  ) => void;
  t: Translator;
}

function SettingsPanel({
  activeView,
  settings,
  updateSettings,
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
    return (
      <FieldGrid>
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

  return (
    <div className="grid grid-cols-2 gap-3">
      <ModelTile
        label={t("model.ocrDefault")}
        value="ppocr-v5-mobile-mnn"
        configuredLabel={t("model.configured")}
      />
      <ModelTile
        label={t("model.translationDefault")}
        value="opus-mt-en-zh-ct2-int8"
        configuredLabel={t("model.configured")}
      />
    </div>
  );
}

function StatusCard({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "danger";
}) {
  return (
    <div className="rounded-lg border bg-card p-3 shadow-sm">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium uppercase text-muted-foreground">
          {label}
        </span>
        <span
          className={cn(
            "size-2 rounded-full bg-emerald-500",
            tone === "danger" && "bg-destructive",
          )}
        />
      </div>
      <strong className="block truncate text-sm font-semibold">{value}</strong>
    </div>
  );
}

function FieldGrid({ children }: { children: React.ReactNode }) {
  return <div className="grid grid-cols-2 gap-4">{children}</div>;
}

function FieldFrame({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-md border bg-background/70 p-3">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <div className="mt-2">{children}</div>
    </div>
  );
}

function SwitchField({
  label,
  checked,
  onCheckedChange,
}: {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex h-[62px] items-center justify-between gap-3 rounded-md border bg-background/70 px-3">
      <Label className="text-sm">{label}</Label>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}

function TextField({
  label,
  value,
  placeholder,
  onValueChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onValueChange: (value: string) => void;
}) {
  return (
    <FieldFrame label={label}>
      <Input
        value={value}
        placeholder={placeholder}
        onChange={(event) => onValueChange(event.target.value)}
      />
    </FieldFrame>
  );
}

function HotkeyField({
  label,
  value,
  onValueChange,
  t,
}: {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  t: Translator;
}) {
  const [recording, setRecording] = useState(false);

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (!recording) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (
      event.key === "Escape" &&
      !event.ctrlKey &&
      !event.shiftKey &&
      !event.altKey &&
      !event.metaKey
    ) {
      setRecording(false);
      return;
    }

    const hotkey = formatHotkey(event);
    if (!hotkey) {
      return;
    }

    onValueChange(hotkey);
    setRecording(false);
  }

  return (
    <FieldFrame label={label}>
      <Button
        type="button"
        variant="outline"
        aria-pressed={recording}
        title={t("hotkey.clickToRecord")}
        className={cn(
          "w-full justify-start font-mono",
          recording &&
            "border-primary bg-accent text-accent-foreground ring-2 ring-ring",
        )}
        onClick={(event) => {
          setRecording(true);
          event.currentTarget.focus();
        }}
        onBlur={() => setRecording(false)}
        onKeyDown={handleKeyDown}
      >
        <Keyboard className="size-4 text-muted-foreground" />
        <span>{recording ? t("hotkey.recording") : value}</span>
      </Button>
    </FieldFrame>
  );
}

function formatHotkey(event: KeyboardEvent<HTMLElement>) {
  const key = normalizeHotkeyKey(event);
  if (!key) {
    return null;
  }

  const modifiers = [
    event.ctrlKey ? "Ctrl" : null,
    event.shiftKey ? "Shift" : null,
    event.altKey ? "Alt" : null,
    event.metaKey ? "Win" : null,
  ].filter(Boolean);

  return [...modifiers, key].join("+");
}

function normalizeHotkeyKey(event: KeyboardEvent<HTMLElement>) {
  const { code, key } = event;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) {
    return null;
  }

  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3);
  }
  if (/^Digit[0-9]$/.test(code)) {
    return code.slice(5);
  }
  if (/^Numpad[0-9]$/.test(code)) {
    return code.slice(6);
  }
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) {
    return code;
  }

  switch (code) {
    case "PrintScreen":
      return "PrintScreen";
    case "Escape":
      return "Esc";
    case "Enter":
    case "NumpadEnter":
      return "Enter";
    case "Space":
      return "Space";
    case "Tab":
      return "Tab";
    default:
      break;
  }

  const upper = key.toUpperCase();
  return /^[A-Z0-9]$/.test(upper) ? upper : null;
}

function NumberField({
  label,
  value,
  min,
  max,
  step,
  onValueChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onValueChange: (value: number) => void;
}) {
  return (
    <FieldFrame label={label}>
      <Input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onValueChange(Number(event.target.value))}
      />
    </FieldFrame>
  );
}

function RangeField({
  label,
  value,
  min,
  max,
  step,
  onValueChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onValueChange: (value: number) => void;
}) {
  return (
    <FieldFrame label={label}>
      <div className="flex items-center gap-3">
        <input
          className="accented-range"
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onValueChange(Number(event.target.value))}
        />
        <Badge variant="secondary" className="min-w-12 justify-center">
          {value}
        </Badge>
      </div>
    </FieldFrame>
  );
}

function ColorField({
  label,
  value,
  onValueChange,
}: {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
}) {
  return (
    <FieldFrame label={label}>
      <div className="flex items-center gap-3">
        <input
          type="color"
          value={value}
          className="h-9 w-14 rounded-md border border-input bg-background p-1"
          onChange={(event) => onValueChange(event.target.value)}
        />
        <span className="text-sm font-medium text-muted-foreground">{value}</span>
      </div>
    </FieldFrame>
  );
}

function LanguageSelect({
  locale,
  onLocaleChange,
  t,
}: {
  locale: Locale;
  onLocaleChange: (locale: Locale) => void;
  t: Translator;
}) {
  return (
    <div className="flex items-center gap-2">
      <Label className="sr-only">{t("language.label")}</Label>
      <Select value={locale} onValueChange={(value) => onLocaleChange(value as Locale)}>
        <SelectTrigger className="w-[132px]">
          <Languages className="size-4 text-muted-foreground" />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {localeOptions.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function SelectField({
  label,
  value,
  options,
  onValueChange,
}: {
  label: string;
  value: string;
  options: ReadonlyArray<{ value: string; label: string }>;
  onValueChange: (value: string) => void;
}) {
  return (
    <FieldFrame label={label}>
      <Select value={value} onValueChange={onValueChange}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FieldFrame>
  );
}

function ModelTile({
  label,
  value,
  configuredLabel,
}: {
  label: string;
  value: string;
  configuredLabel: string;
}) {
  return (
    <div className="rounded-md border bg-background/70 p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-xs font-medium text-muted-foreground">{label}</p>
          <p className="mt-2 text-sm font-semibold">{value}</p>
        </div>
        <Settings2 className="size-4 text-muted-foreground" />
      </div>
      <Separator className="my-4" />
      <Badge variant="secondary">{configuredLabel}</Badge>
    </div>
  );
}

function localizedOptions(
  options: Array<{ value: string; labelKey: TranslationKey }>,
  t: Translator,
) {
  return options.map((option) => ({
    value: option.value,
    label: t(option.labelKey),
  }));
}
