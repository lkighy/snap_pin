import { KeyboardEvent, ReactNode, useState } from "react";
import { Keyboard, Languages, Settings2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  localeOptions,
  Locale,
  TranslationKey,
  Translator,
} from "@/i18n";

export function FieldGrid({ children }: { children: ReactNode }) {
  return (
    <div className="grid min-w-0 grid-cols-1 gap-4 md:grid-cols-2">
      {children}
    </div>
  );
}

export function SettingsStack({ children }: { children: ReactNode }) {
  return <div className="grid min-w-0 gap-5">{children}</div>;
}

export function SettingsSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="min-w-0">
      <div className="mb-3 min-w-0">
        <h3 className="break-words text-sm font-semibold leading-6">{title}</h3>
        {description ? (
          <p className="mt-1 break-words text-xs leading-5 text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>
      <FieldGrid>{children}</FieldGrid>
    </section>
  );
}

function FieldFrame({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-3">
      <Label className="break-words text-xs text-muted-foreground">{label}</Label>
      <div className="mt-2 min-w-0">{children}</div>
    </div>
  );
}

export function SwitchField({
  label,
  checked,
  disabled = false,
  onCheckedChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex min-h-[62px] min-w-0 items-center justify-between gap-3 rounded-md border bg-background/70 px-3 py-2">
      <Label className="min-w-0 break-words text-sm">{label}</Label>
      <Switch
        className="shrink-0"
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
      />
    </div>
  );
}

export function TextField({
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

export function HotkeyField({
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
          "w-full min-w-0 justify-start font-mono",
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
        <span className="truncate">{recording ? t("hotkey.recording") : value}</span>
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

export function NumberField({
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

export function RangeField({
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
      <div className="flex min-w-0 items-center gap-3">
        <input
          className="accented-range"
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onValueChange(Number(event.target.value))}
        />
        <Badge variant="secondary" className="min-w-12 shrink-0 justify-center">
          {value}
        </Badge>
      </div>
    </FieldFrame>
  );
}

export function ColorField({
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
      <div className="flex min-w-0 items-center gap-3">
        <input
          type="color"
          value={value}
          className="h-9 w-14 shrink-0 rounded-md border border-input bg-background p-1"
          onChange={(event) => onValueChange(event.target.value)}
        />
        <span className="min-w-0 truncate text-sm font-medium text-muted-foreground">
          {value}
        </span>
      </div>
    </FieldFrame>
  );
}

export function LanguageSelect({
  locale,
  onLocaleChange,
  t,
}: {
  locale: Locale;
  onLocaleChange: (locale: Locale) => void;
  t: Translator;
}) {
  return (
    <div className="min-w-0">
      <Label className="sr-only">{t("language.label")}</Label>
      <Select value={locale} onValueChange={(value) => onLocaleChange(value as Locale)}>
        <SelectTrigger className="w-[132px] max-w-full">
          <Languages className="size-4 shrink-0 text-muted-foreground" />
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

export function SelectField({
  label,
  value,
  options,
  onValueChange,
  fallbackValue,
}: {
  label: string;
  value: string;
  options: ReadonlyArray<{ value: string; label: string; disabled?: boolean }>;
  onValueChange: (value: string) => void;
  fallbackValue?: string;
}) {
  const selectedValue = options.some((option) => option.value === value)
    ? value
    : fallbackValue && options.some((option) => option.value === fallbackValue)
      ? fallbackValue
      : options[0]?.value ?? "";

  return (
    <FieldFrame label={label}>
      <Select value={selectedValue} onValueChange={onValueChange}>
        <SelectTrigger className="min-w-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem
              key={option.value}
              value={option.value}
              disabled={option.disabled}
            >
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FieldFrame>
  );
}

export function ModelTile({
  label,
  value,
  configuredLabel,
}: {
  label: string;
  value: string;
  configuredLabel: string;
}) {
  return (
    <div className="min-w-0 rounded-md border bg-background/70 p-4">
      <div className="flex min-w-0 items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="break-words text-xs font-medium text-muted-foreground">{label}</p>
          <p className="mt-2 break-all text-sm font-semibold">{value}</p>
        </div>
        <Settings2 className="size-4 shrink-0 text-muted-foreground" />
      </div>
      <Separator className="my-4" />
      <Badge variant="secondary">{configuredLabel}</Badge>
    </div>
  );
}

export function localizedOptions(
  options: Array<{ value: string; labelKey: TranslationKey }>,
  t: Translator,
) {
  return options.map((option) => ({
    value: option.value,
    label: t(option.labelKey),
  }));
}
