import { AppWindow, Braces, MessageSquareText, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button, IconButton, Section, Select, SettingRow, Switch, TextField, useToast } from "@/components/ui";
import { api, errorMessage, type AppCategory, type AppRule, type AppTone, type ToneSettings } from "@/lib/api";
import { useStore } from "@/lib/store";

const CATEGORY_LABELS: Record<AppCategory, string> = {
  chat: "Chat",
  email: "Email",
  terminal: "Terminal",
  code: "Code editor",
  other: "Documents and other",
};

const CATEGORY_OPTIONS = (Object.keys(CATEGORY_LABELS) as AppCategory[]).map((value) => ({
  value,
  label: CATEGORY_LABELS[value],
}));

/** "Slack, Discord, Messages" for the built-in apps in one category. */
function builtInNames(builtIn: AppTone["builtIn"], category: AppCategory) {
  return builtIn
    .filter((app) => app.category === category)
    .map((app) => app.name)
    .join(", ");
}

/** Matching dictation to the app it goes into, and writing code as code. */
export function AppToneSection() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const tone = settings.tone;
  const save = useCallback(
    (patch: Partial<ToneSettings>) =>
      updateSettings({ tone: { ...tone, ...patch } }).catch((e) => toast(errorMessage(e), "error")),
    [tone, updateSettings, toast],
  );
  const [info, setInfo] = useState<AppTone>();
  useEffect(() => {
    api.getAppTone().then(setInfo).catch(() => {});
  }, []);

  return (
    <Section
      title="Tone that fits the app"
      description="Wordfall notices which app you're dictating into and writes to suit it."
    >
      <SettingRow
        icon={MessageSquareText}
        label="Match the app's tone"
        description={
          settings.cleanup.enabled
            ? "AI cleanup keeps chat casual, writes emails in full sentences and turns terminal dictation into commands."
            : "Chat messages lose their final period, and terminal commands their capital letter and punctuation. Turn on AI cleanup for more."
        }
      >
        <Switch label="Match the app's tone" checked={tone.enabled} onChange={(enabled) => save({ enabled })} />
      </SettingRow>
      <SettingRow
        icon={Braces}
        label="Write code as code"
        description="In code editors and terminals, “main dot rs” becomes main.rs, “camel case get user” becomes getUser, and “dash dash verbose” becomes --verbose."
      >
        <Switch label="Write code as code" checked={tone.formatCode} onChange={(formatCode) => save({ formatCode })} />
      </SettingRow>
      {(tone.enabled || tone.formatCode) && <AppRules rules={tone.apps} info={info} onChange={(apps) => save({ apps })} />}
    </Section>
  );
}

function AppRules({
  rules,
  info,
  onChange,
}: {
  rules: AppRule[];
  info: AppTone | undefined;
  onChange: (rules: AppRule[]) => void;
}) {
  // Edited here and saved on blur, so typing doesn't save on every key.
  const [names, setNames] = useState(rules.map((r) => r.app));
  useEffect(() => setNames(rules.map((r) => r.app)), [rules]);

  const update = (index: number, patch: Partial<AppRule>) =>
    onChange(rules.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)));
  const last = info?.lastApp;
  const lastName = last?.name || last?.id;
  const lastHasRule = !!lastName && rules.some((r) => r.app.trim().toLowerCase() === lastName.toLowerCase());

  return (
    <div className="py-4">
      <div className="flex items-start gap-3.5">
        <AppWindow size={18} strokeWidth={1.75} className="mt-px shrink-0 text-ink-muted" />
        <div className="min-w-0 flex-1">
          <div className="type-label text-ink">Your apps</div>
          <div className="mt-0.5 type-small text-ink-secondary">
            Choose how an app is treated, by its name or id. These come before the built-in list.
          </div>
          {info && (
            <ul className="mt-2 space-y-0.5 type-small text-ink-muted">
              {(["chat", "email", "terminal", "code"] as AppCategory[]).map((category) => (
                <li key={category}>
                  <span className="text-ink-secondary">{CATEGORY_LABELS[category]}:</span>{" "}
                  {builtInNames(info.builtIn, category)}
                </li>
              ))}
              <li>Gmail, Outlook, Slack and other web apps count too when they're open in a browser.</li>
            </ul>
          )}

          {rules.length > 0 && (
            <div className="mt-3 space-y-2">
              {rules.map((rule, index) => (
                <div key={index} className="flex items-center gap-2">
                  <TextField
                    value={names[index] ?? ""}
                    onChange={(e) => setNames(names.map((n, i) => (i === index ? e.target.value : n)))}
                    onBlur={() => names[index] !== rule.app && update(index, { app: names[index].trim() })}
                    onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
                    placeholder="App name or id"
                    spellCheck={false}
                    aria-label="App"
                    className="min-w-0 flex-1"
                  />
                  <div className="w-[200px] shrink-0">
                    <Select
                      value={rule.category}
                      options={CATEGORY_OPTIONS}
                      onChange={(category) => update(index, { category })}
                      aria-label="Treat as"
                    />
                  </div>
                  <IconButton
                    icon={Trash2}
                    label="Remove"
                    size="sm"
                    tone="danger"
                    onClick={() => onChange(rules.filter((_, i) => i !== index))}
                  />
                </div>
              ))}
            </div>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button size="sm" variant="ghost" icon={Plus} onClick={() => onChange([...rules, { app: "", category: "chat" }])}>
              Add app
            </Button>
            {last && lastName && !lastHasRule && (
              <Button
                size="sm"
                variant="ghost"
                icon={Plus}
                onClick={() => onChange([...rules, { app: lastName, category: last.category }])}
              >
                Add {lastName}
              </Button>
            )}
          </div>
          {last && (
            <p className="mt-2 type-small text-ink-muted">
              Last app you dictated into: {lastName}
              {last.id && last.id !== lastName ? ` (${last.id})` : ""}, treated as {CATEGORY_LABELS[last.category].toLowerCase()}.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
