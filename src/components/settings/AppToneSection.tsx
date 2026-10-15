import { AppWindow, Braces, ChevronDown, MessageSquareText, Plus, Trash2 } from "lucide-react";
import { Fragment, useCallback, useEffect, useState } from "react";
import { Button, IconButton, Section, Select, SettingRow, Switch, TextField, useToast } from "@/components/ui";
import { cn } from "@/lib/cn";
import { api, errorMessage, type AppCategory, type AppRule, type AppTone, type ToneSettings } from "@/lib/api";
import { useStore } from "@/lib/store";

/** Each category is named after what it does to the text, with the kind of app it's for. */
const STYLES: Record<AppCategory, { label: string; apps: string; withAi: string; withoutAi: string }> = {
  chat: {
    label: "Casual",
    apps: "chat apps",
    withAi: "Short and relaxed: lowercase is fine and there's no period at the end.",
    withoutAi: "No period at the end.",
  },
  email: {
    label: "Formal",
    apps: "email",
    withAi: "Full sentences and paragraphs, with a greeting or sign-off on its own line.",
    withoutAi: "Unchanged without AI cleanup.",
  },
  terminal: {
    label: "Command",
    apps: "terminals",
    withAi: "Only the command, as you'd type it: no capital letter, quotes or period.",
    withoutAi: "No capital letter or punctuation at the end.",
  },
  code: {
    label: "Code",
    apps: "code editors",
    withAi: "Written as a code comment or commit message.",
    withoutAi: "Unchanged, apart from writing code as code.",
  },
  other: {
    label: "Standard",
    apps: "everything else",
    withAi: "Ordinary sentences and punctuation.",
    withoutAi: "Ordinary sentences and punctuation.",
  },
};

const CATEGORY_OPTIONS = (Object.keys(STYLES) as AppCategory[]).map((value) => ({
  value,
  label: STYLES[value].label,
  detail: STYLES[value].apps,
}));

/** How many built-in apps each style names before "and 10 more". */
const PREVIEW_APPS = 3;

/** The built-in apps in one category, by name. */
function builtInNames(builtIn: AppTone["builtIn"], category: AppCategory) {
  return builtIn.filter((app) => app.category === category).map((app) => app.name);
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
      description="WordFlow notices which app you're dictating into and writes to suit it."
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
      {(tone.enabled || tone.formatCode) && (
        <AppRules rules={tone.apps} info={info} ai={settings.cleanup.enabled} onChange={(apps) => save({ apps })} />
      )}
    </Section>
  );
}

function AppRules({
  rules,
  info,
  ai,
  onChange,
}: {
  rules: AppRule[];
  info: AppTone | undefined;
  /** AI cleanup is on, which changes what each style does. */
  ai: boolean;
  onChange: (rules: AppRule[]) => void;
}) {
  // Edited here and saved on blur, so typing doesn't save on every key.
  const [names, setNames] = useState(rules.map((r) => r.app));
  const [allApps, setAllApps] = useState(false);
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
          <div className="type-label text-ink">Styles</div>
          <div className="mt-0.5 type-small text-ink-secondary">
            Each app gets one of these styles. Apps not listed get Standard.
          </div>
          <dl className="mt-3 grid grid-cols-[88px_minmax(0,1fr)] gap-x-4 gap-y-2.5 type-small">
            {(Object.keys(STYLES) as AppCategory[]).map((category) => {
              const builtIn = info ? builtInNames(info.builtIn, category) : [];
              return (
                <Fragment key={category}>
                  <dt className="text-ink">{STYLES[category].label}</dt>
                  <dd>
                    <div className="text-ink-secondary">{ai ? STYLES[category].withAi : STYLES[category].withoutAi}</div>
                    <div className="text-ink-muted">
                      {builtIn.length === 0
                        ? STYLES[category].apps
                        : allApps || builtIn.length <= PREVIEW_APPS
                          ? builtIn.join(", ")
                          : `${builtIn.slice(0, PREVIEW_APPS).join(", ")} and ${builtIn.length - PREVIEW_APPS} more`}
                    </div>
                  </dd>
                </Fragment>
              );
            })}
          </dl>
          {allApps && (
            <p className="mt-2 type-small text-ink-muted">Web apps like Gmail and Slack count too in a browser.</p>
          )}
          {info && info.builtIn.length > 0 && (
            <button
              type="button"
              onClick={() => setAllApps(!allApps)}
              aria-expanded={allApps}
              className="mt-2 flex items-center gap-1 type-small text-ink-muted hover:text-ink"
            >
              {allApps ? "Show fewer apps" : "Show every built-in app"}
              <ChevronDown size={14} className={cn("transition-transform duration-200", allApps && "rotate-180")} />
            </button>
          )}

          <div className="mt-4 type-label text-ink">Your apps</div>
          <div className="mt-0.5 type-small text-ink-secondary">
            Pick a style for an app by its name or id. Yours come before the built-in list.
          </div>

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
                      aria-label="Style"
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
              {last.id && last.id !== lastName ? ` (${last.id})` : ""}, given the {STYLES[last.category].label} style.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
