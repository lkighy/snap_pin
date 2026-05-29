import { en, messages } from "@/i18n/messages";
export { localeOptions } from "@/i18n/messages";
export type { Locale } from "@/i18n/messages";
import type { Locale, Messages } from "@/i18n/messages";

export type TranslationKey = keyof Messages;
export type Translator = (
  key: TranslationKey,
  values?: Record<string, string | number>,
) => string;

export function getInitialLocale(): Locale {
  const languages =
    typeof navigator === "undefined" ? [] : navigator.languages ?? [navigator.language];
  for (const language of languages) {
    const locale = normalizeLocale(language);
    if (locale) {
      return locale;
    }
  }

  return "zh-CN";
}

export function translate(
  locale: Locale,
  key: TranslationKey,
  values: Record<string, string | number> = {},
) {
  const template = messages[locale][key] ?? en[key] ?? key;
  return Object.entries(values).reduce((text, [name, value]) => {
    return text.split(`{${name}}`).join(String(value));
  }, template);
}

export function translateStatusSummary(
  value: string,
  locale: Locale,
  t: Translator,
) {
  if (value === "Starting desktop shell...") {
    return t("app.starting");
  }

  if (value === "Loading...") {
    return t("state.loading");
  }

  if (value.startsWith("snap pin desktop shell ready: ")) {
    return t("summary.bootReady", {
      capabilities: value.replace("snap pin desktop shell ready: ", ""),
    });
  }

  if (value.startsWith("models: ")) {
    return t("summary.models", {
      models: value.replace("models: ", ""),
    });
  }

  const history = value.match(
    /^history: (\d+) ocr result\(s\), (\d+) translation\(s\)$/,
  );
  if (history) {
    return t("summary.history", {
      ocr: history[1],
      translations: history[2],
    });
  }

  return messages[locale] ? value : value;
}

function normalizeLocale(value: string | null | undefined): Locale | null {
  if (!value) {
    return null;
  }

  const normalized = value.toLowerCase();
  if (normalized === "zh-cn" || normalized === "zh" || normalized.startsWith("zh-")) {
    return "zh-CN";
  }
  if (normalized.startsWith("ja")) {
    return "ja";
  }
  if (normalized.startsWith("ko")) {
    return "ko";
  }
  if (normalized.startsWith("fr")) {
    return "fr";
  }
  if (normalized.startsWith("de")) {
    return "de";
  }
  if (normalized.startsWith("en")) {
    return "en";
  }

  return null;
}
