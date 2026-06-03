import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  Camera,
  Loader2,
  RefreshCw,
  Save,
  Sparkles,
  Wand2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import {
  AppSettings,
  AppStatus,
  cancelModelDownload,
  chooseOcrModelStorageDir,
  drainEvents,
  getModelStorageInfo,
  getModelDownloadStatus,
  getAppStatus,
  getSettings,
  importModel,
  listModels,
  ModelDownloadStatus,
  ModelStorageInfo,
  ModelSummary,
  openOcrModelStorageDir,
  runMvpFlow,
  saveSettings,
  setOcrModelStorageDir,
  startBuiltinOcrModelDownload,
  startCapture,
} from "@/lib/tauri";
import {
  getInitialLocale,
  Locale,
  translate,
  translateStatusSummary,
  Translator,
} from "@/i18n";
import { defaultStatus, navItems, SaveStatusKey, ViewId } from "@/app-data";
import { LanguageSelect } from "@/settings-fields";
import { SettingsPanel } from "@/settings-panel";

export function App() {
  const [locale, setLocale] = useState<Locale>(() => getInitialLocale());
  const [activeView, setActiveView] = useState<ViewId>("capture");
  const [status, setStatus] = useState<AppStatus>(defaultStatus);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [models, setModels] = useState<ModelSummary[]>([]);
  const [eventLog, setEventLog] = useState<string[]>([]);
  const [saveStatus, setSaveStatus] = useState<SaveStatusKey>("state.ready");
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [modelManifestPath, setModelManifestPath] = useState("");
  const [modelStorageInfo, setModelStorageInfo] =
    useState<ModelStorageInfo | null>(null);
  const [modelStoragePath, setModelStoragePath] = useState("");
  const [modelDownloadStatus, setModelDownloadStatus] =
    useState<ModelDownloadStatus | null>(null);

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

  async function loadModels() {
    setModels(await listModels());
  }

  async function loadModelStorageInfo() {
    const info = await getModelStorageInfo();
    setModelStorageInfo(info);
    setModelStoragePath(info.currentOcrModelsDir);
  }

  async function refreshAll() {
    setError(null);
    setBusyAction("refresh");
    try {
      await Promise.all([
        refreshStatus(),
        loadSettings(),
        loadModels(),
        loadModelStorageInfo(),
      ]);
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

  useEffect(() => {
    if (busyAction !== null) {
      return;
    }

    const interval = window.setInterval(() => {
      void drainEvents()
        .then((response) => {
          if (response.events.length === 0) {
            return;
          }
          setEventLog(response.events);
          if (response.historySummary) {
            setStatus((current) => ({
              ...current,
              historySummary: response.historySummary ?? current.historySummary,
            }));
          }
        })
        .catch((caught) => {
          setError(String(caught));
        });
    }, 1000);

    return () => window.clearInterval(interval);
  }, [busyAction]);

  useEffect(() => {
    if (!modelDownloadStatus?.running) {
      return;
    }

    const interval = window.setInterval(() => {
      void refreshModelDownloadStatus();
    }, 500);

    return () => window.clearInterval(interval);
  }, [modelDownloadStatus?.running]);

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

  async function handleDrainEvents() {
    setError(null);
    setBusyAction("drain");
    try {
      const response = await drainEvents();
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

  async function handleImportModel() {
    const manifestPath = modelManifestPath.trim();
    if (!manifestPath) {
      return;
    }

    setError(null);
    setBusyAction("import-model");
    try {
      const response = await importModel(manifestPath);
      setEventLog(response.events);
      setStatus((current) => ({
        ...current,
        modelSummary: response.modelSummary,
      }));
      setModels(response.models);
      setSettings(response.settings);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  function applyModelDownloadResult(response: NonNullable<ModelDownloadStatus["result"]>) {
    setEventLog(response.events);
    setStatus((current) => ({
      ...current,
      modelSummary: response.modelSummary,
    }));
    setModels(response.models);
    setSettings(response.settings);
  }

  async function refreshModelDownloadStatus() {
    try {
      const nextStatus = await getModelDownloadStatus();
      setModelDownloadStatus(nextStatus);
      if (nextStatus?.result) {
        applyModelDownloadResult(nextStatus.result);
        setBusyAction(null);
      }
      if (nextStatus?.error) {
        setError(nextStatus.error);
        setBusyAction(null);
      }
      if (nextStatus && !nextStatus.running) {
        setBusyAction(null);
      }
    } catch (caught) {
      setError(String(caught));
      setBusyAction(null);
    }
  }

  async function handleDownloadOcrModel(modelId: string) {
    setError(null);
    setBusyAction("download-ocr-model");
    try {
      const nextStatus = await startBuiltinOcrModelDownload(modelId);
      setModelDownloadStatus(nextStatus);
    } catch (caught) {
      setError(String(caught));
      setBusyAction(null);
    }
  }

  async function handleCancelModelDownload() {
    try {
      const nextStatus = await cancelModelDownload();
      setModelDownloadStatus(nextStatus);
      if (nextStatus?.error) {
        setError(nextStatus.error);
      }
    } finally {
      await refreshModelDownloadStatus();
    }
  }

  async function handleChooseModelStorageDir() {
    setError(null);
    setBusyAction("model-storage");
    try {
      const info = await chooseOcrModelStorageDir();
      if (info) {
        setModelStorageInfo(info);
        setModelStoragePath(info.currentOcrModelsDir);
        await loadModels();
      }
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleApplyModelStorageDir() {
    const path = modelStoragePath.trim();
    if (!path) {
      return;
    }

    setError(null);
    setBusyAction("model-storage");
    try {
      const info = await setOcrModelStorageDir(path);
      setModelStorageInfo(info);
      setModelStoragePath(info.currentOcrModelsDir);
      await loadModels();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleOpenModelStorageDir() {
    setError(null);
    setBusyAction("open-model-storage");
    try {
      await openOcrModelStorageDir();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBusyAction(null);
    }
  }

  const isBusy = busyAction !== null;
  const showEventLog = activeView === "history";

  return (
    <main className="flex h-screen overflow-hidden bg-background text-foreground">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex min-h-[72px] shrink-0 flex-wrap items-center justify-between gap-3 border-b bg-card px-5 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg border bg-primary text-primary-foreground shadow-sm">
            <Sparkles className="size-5" />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-xl font-semibold leading-tight tracking-normal">
              {t("app.name")}
            </h1>
            <p className="mt-1 max-w-[520px] truncate text-sm text-muted-foreground">
              {translateStatusSummary(status.bootSummary, locale, t)}
            </p>
          </div>
        </div>

        <div className="flex min-w-0 flex-wrap items-center justify-end gap-2">
          <div
            className={cn(
              "flex h-9 min-w-0 items-center gap-2 rounded-md border bg-background px-3 text-sm shadow-sm",
              (saveStatus === "state.loadFailed" ||
                saveStatus === "state.saveFailed") &&
                "border-destructive/35 bg-destructive/10 text-destructive",
            )}
          >
            <span className="font-medium">{t("status.settings")}</span>
            <span className="text-muted-foreground">/</span>
            <span className="truncate">{t(saveStatus)}</span>
          </div>
          <LanguageSelect locale={locale} onLocaleChange={handleLocaleChange} t={t} />
          <Button
            type="button"
            variant="outline"
            className="min-w-0 shrink"
            onClick={handleStartCapture}
            disabled={isBusy}
          >
            {busyAction === "capture" ? (
              <Loader2 className="animate-spin" />
            ) : (
              <Camera />
            )}
            <span className="truncate">{t("button.startCapture")}</span>
          </Button>
          <Button
            type="button"
            className="min-w-0 shrink"
            onClick={handleRunMvpFlow}
            disabled={isBusy}
          >
            {busyAction === "mvp" ? (
              <Loader2 className="animate-spin" />
            ) : (
              <Wand2 />
            )}
            <span className="truncate">{t("button.runOcrTranslate")}</span>
          </Button>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(150px,184px)_minmax(0,1fr)] lg:grid-cols-[212px_minmax(0,1fr)]">
        <aside className="min-h-0 min-w-0 overflow-y-auto border-r bg-muted/35 p-3">
          <nav className="grid gap-1" aria-label={t("nav.settings")}>
            {navItems.map((item) => {
              const Icon = item.icon;
              const selected = activeView === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={cn(
                    "flex h-9 w-full min-w-0 items-center gap-3 rounded-md px-3 text-left text-sm font-medium text-muted-foreground transition-colors hover:bg-background hover:text-foreground",
                    selected &&
                      "bg-background text-foreground shadow-sm ring-1 ring-border",
                  )}
                  onClick={() => setActiveView(item.id)}
                >
                  <Icon className="size-4 shrink-0" />
                  <span className="truncate">{t(item.labelKey)}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        <section className="min-h-0 min-w-0 overflow-y-auto p-4">
          <div className="grid content-start gap-4">
          {error ? (
            <div className="rounded-md border border-destructive/25 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          ) : null}

          <form onSubmit={handleSave}>
            <Card className="min-h-[360px] min-w-0 overflow-hidden">
              <CardHeader className="flex-row flex-wrap items-center justify-between gap-3 space-y-0">
                <div className="min-w-0">
                  <CardTitle className="truncate">{activeTitle}</CardTitle>
                </div>
                <div className="flex min-w-0 flex-wrap justify-end gap-2">
                  {activeView === "history" ? (
                    <Button
                      type="button"
                      variant="outline"
                      className="min-w-0 shrink"
                      onClick={handleDrainEvents}
                      disabled={isBusy}
                    >
                      {busyAction === "drain" ? (
                        <Loader2 className="animate-spin" />
                      ) : (
                        <RefreshCw />
                      )}
                      <span className="truncate">{t("button.drainEvents")}</span>
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    variant="outline"
                    className="min-w-0 shrink"
                    onClick={refreshAll}
                    disabled={isBusy}
                  >
                    {busyAction === "refresh" ? (
                      <Loader2 className="animate-spin" />
                    ) : (
                      <RefreshCw />
                    )}
                    <span className="truncate">{t("button.refresh")}</span>
                  </Button>
                  <Button
                    type="submit"
                    className="min-w-0 shrink"
                    disabled={!settings || isBusy}
                  >
                    {busyAction === "save" ? (
                      <Loader2 className="animate-spin" />
                    ) : (
                      <Save />
                    )}
                    <span className="truncate">{t("button.saveSettings")}</span>
                  </Button>
                </div>
              </CardHeader>
              <CardContent>
                {settings ? (
                  <SettingsPanel
                    activeView={activeView}
                    settings={settings}
                    updateSettings={updateSettings}
                    modelManifestPath={modelManifestPath}
                    onModelManifestPathChange={setModelManifestPath}
                    modelStorageInfo={modelStorageInfo}
                    modelStoragePath={modelStoragePath}
                    onModelStoragePathChange={setModelStoragePath}
                    onImportModel={handleImportModel}
                    onDownloadOcrModel={handleDownloadOcrModel}
                    onCancelModelDownload={handleCancelModelDownload}
                    onChooseModelStorageDir={handleChooseModelStorageDir}
                    onApplyModelStorageDir={handleApplyModelStorageDir}
                    onOpenModelStorageDir={handleOpenModelStorageDir}
                    onOpenModels={() => setActiveView("models")}
                    importingModel={busyAction === "import-model"}
                    managingModelStorage={
                      busyAction === "model-storage" ||
                      busyAction === "open-model-storage"
                    }
                    downloadingOcrModel={busyAction === "download-ocr-model"}
                    modelDownloadStatus={modelDownloadStatus}
                    models={models}
                    status={status}
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

          {showEventLog ? (
            <Card className="min-w-0 overflow-hidden">
              <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
                <CardTitle className="min-w-0 truncate">
                  {t("events.title")}
                </CardTitle>
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
          ) : null}
          </div>
        </section>
      </div>
      </div>
    </main>
  );
}
