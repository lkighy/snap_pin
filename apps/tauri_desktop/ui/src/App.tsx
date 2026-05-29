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
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  Locale,
  translate,
  translateStatusSummary,
  Translator,
} from "@/i18n";
import { defaultStatus, navItems, SaveStatusKey, ViewId } from "@/app-data";
import { LanguageSelect, StatusCard } from "@/settings-fields";
import { SettingsPanel } from "@/settings-panel";

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
