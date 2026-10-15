import { Brain, Check, Cloud, Cpu, KeyRound, PlugZap, Server, Timer, TriangleAlert } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Badge, Button, Section, SegmentedControl, Select, SettingRow, Switch, TextField, useToast } from "@/components/ui";
import { DeleteButton, DownloadAction } from "@/screens/models/ModelsScreen";
import { api, errorMessage, type CleanupSettings, type CleanupStatus, type ModelStatus, type ServerModels } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatModelSize } from "@/lib/format";
import { useStore } from "@/lib/store";
import { useTauriEvent } from "@/lib/useTauriEvent";

/** Settings for tidying transcripts with a local language model. */
export function CleanupSection() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const cleanup = settings.cleanup;
  const save = useCallback(
    (patch: Partial<CleanupSettings>) =>
      updateSettings({ cleanup: { ...cleanup, ...patch } }).catch((e) => toast(errorMessage(e), "error")),
    [cleanup, updateSettings, toast],
  );

  return (
    <Section
      title="AI cleanup"
      description="A language model tidies each dictation. If it's slow or fails, your transcript is pasted as it was."
    >
      <SettingRow
        icon={Brain}
        label="Clean up with an AI model"
        description="Fixes misheard words and grammar, keeps only your correction when you change your mind, and turns spoken lists into lists."
      >
        <Switch checked={cleanup.enabled} onChange={(enabled) => save({ enabled })} />
      </SettingRow>
      {cleanup.enabled && (
        <>
          <SettingRow
            icon={BACKENDS[cleanup.backend].icon}
            label="Model runs in"
            description={BACKENDS[cleanup.backend].description}
          >
            <SegmentedControl
              value={cleanup.backend}
              onChange={(backend) => save({ backend })}
              options={[
                { value: "builtin", label: "Wordflow" },
                { value: "server", label: "Local server" },
                { value: "api", label: "Cloud API" },
              ]}
              className="w-[300px]"
            />
          </SettingRow>
          {cleanup.backend === "builtin" && <BuiltinModels cleanup={cleanup} save={save} />}
          {cleanup.backend === "server" && <ServerSettings cleanup={cleanup} save={save} />}
          {cleanup.backend === "api" && <ApiSettings cleanup={cleanup} save={save} />}
        </>
      )}
    </Section>
  );
}

const BACKENDS = {
  builtin: { icon: Cpu, description: "Wordflow downloads a small model and runs it itself." },
  server: { icon: Server, description: "A model server you already run, such as Ollama or LM Studio." },
  api: { icon: Cloud, description: "A hosted model you choose, with your own API key." },
} as const;

type Save = (patch: Partial<CleanupSettings>) => void;

function useCleanupStatus() {
  const [status, setStatus] = useState<CleanupStatus>();
  const refresh = useCallback(() => {
    api.getCleanupStatus().then(setStatus).catch(() => {});
  }, []);
  useEffect(refresh, [refresh]);
  useTauriEvent("models-changed", refresh);
  return status;
}

function BuiltinModels({ cleanup, save }: { cleanup: CleanupSettings; save: Save }) {
  const status = useCleanupStatus();
  if (!status) return null;
  if (!status.builtinAvailable) {
    return (
      <Notice tone="warning">
        This copy of Wordflow doesn't include its own cleanup engine. Use a local server instead, or if you built
        Wordflow yourself, build cleanup-worker too.
      </Notice>
    );
  }
  return (
    <div className="divide-y divide-line-subtle">
      {status.models.map((model) => (
        <BuiltinModelRow
          key={model.id}
          model={model}
          active={model.downloaded && model.id === cleanup.builtinModel}
          onSelect={() => save({ builtinModel: model.id })}
        />
      ))}
    </div>
  );
}

function BuiltinModelRow({ model, active, onSelect }: { model: ModelStatus; active: boolean; onSelect: () => void }) {
  const { downloadErrors } = useStore();
  const selectable = model.downloaded && !active;
  const error = !model.downloaded && !model.downloading ? downloadErrors[model.id] : undefined;
  return (
    <div
      role={model.downloaded ? "radio" : undefined}
      aria-checked={model.downloaded ? active : undefined}
      tabIndex={selectable ? 0 : undefined}
      onClick={selectable ? onSelect : undefined}
      onKeyDown={(e) => {
        if (selectable && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onSelect();
        }
      }}
      className={cn("group/row px-5 py-3.5 transition-colors", selectable && "cursor-pointer hover:bg-hover/50")}
    >
      <div className="flex items-center gap-4">
        <div className="min-w-0 flex-1 pl-[46px]">
          <div className="type-label">{model.name}</div>
          <div className="mt-0.5 type-small text-ink-secondary">
            {model.description} {formatModelSize(model.sizeMb)}.
          </div>
        </div>
        <div className="flex w-[148px] shrink-0 items-center justify-end gap-1.5">
          {model.downloaded && !model.downloading ? (
            <>
              {active ? (
                <Badge tone="accent" icon={Check}>
                  In use
                </Badge>
              ) : (
                <span className="type-small font-medium text-ink-secondary opacity-0 transition-opacity group-hover/row:opacity-100">
                  Use
                </span>
              )}
              <DeleteButton model={model} />
            </>
          ) : (
            <DownloadAction model={model} />
          )}
        </div>
      </div>
      {error && (
        <p className="mt-2 flex items-start gap-2 pl-[46px] type-small text-danger">
          <TriangleAlert size={14} className="mt-0.5 shrink-0" />
          {error}
        </p>
      )}
    </div>
  );
}

function ServerSettings({ cleanup, save }: { cleanup: CleanupSettings; save: Save }) {
  const [url, setUrl] = useState(cleanup.serverUrl);
  const [server, setServer] = useState<ServerModels>();
  const [error, setError] = useState<string>();
  const [connecting, setConnecting] = useState(false);

  /** Lists the server's models. Returns them, or null if nothing answered. */
  const connect = useCallback(async (address: string) => {
    setConnecting(true);
    setError(undefined);
    try {
      const found = await api.listServerModels(address);
      setServer(found);
      return found.models;
    } catch (e) {
      setServer(undefined);
      setError(errorMessage(e));
      return null;
    } finally {
      setConnecting(false);
    }
  }, []);

  /** Picks the server's first model when none is chosen yet. */
  const firstModel = (models: string[] | null) =>
    !cleanup.serverModel && models?.length ? { serverModel: models[0] } : {};

  // Check the saved address once, so the model list is ready.
  useEffect(() => {
    connect(cleanup.serverUrl).then((models) => {
      const patch = firstModel(models);
      if (patch.serverModel) save(patch);
    });
  }, []);

  // One save once the server has answered: a save before it would be undone
  // by this one, which starts from the settings as they were.
  const commitUrl = async () => {
    const trimmed = url.trim();
    if (trimmed === cleanup.serverUrl) return;
    const models = await connect(trimmed);
    save({ serverUrl: trimmed, ...firstModel(models) });
  };

  const models = server?.models ?? [];
  const options = (models.includes(cleanup.serverModel) || !cleanup.serverModel ? models : [cleanup.serverModel, ...models]).map(
    (m) => ({ value: m, label: m }),
  );

  return (
    <>
      <SettingRow
        icon={PlugZap}
        label="Server address"
        description="Ollama is at http://localhost:11434, LM Studio at http://localhost:1234."
      >
        <div className="flex items-center gap-2">
          <TextField
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onBlur={commitUrl}
            onKeyDown={(e) => e.key === "Enter" && commitUrl()}
            spellCheck={false}
            aria-label="Server address"
            className="w-[240px]"
          />
          <Button size="sm" loading={connecting} onClick={() => (url.trim() === cleanup.serverUrl ? connect(url.trim()) : commitUrl())}>
            Connect
          </Button>
        </div>
      </SettingRow>
      <SettingRow
        icon={Brain}
        label="Model"
        description={
          server && models.length === 0
            ? "The server has no models yet. With Ollama, run: ollama pull llama3.2"
            : "Small models (1–4B) are quick enough for dictation."
        }
      >
        <Select
          value={cleanup.serverModel}
          options={options}
          onChange={(serverModel) => save({ serverModel })}
          placeholder={server ? "Choose a model" : "Connect first"}
          searchable={options.length > 8}
          aria-label="Server model"
          className="w-[240px]"
        />
      </SettingRow>
      <SettingRow
        icon={Timer}
        label="Keep model loaded"
        description="How long Ollama holds the model in memory after a dictation. It loads again while you speak, so shorter frees memory sooner."
      >
        <Select
          value={String(cleanup.serverKeepAliveMinutes)}
          options={KEEP_ALIVE_OPTIONS}
          onChange={(minutes) => save({ serverKeepAliveMinutes: Number(minutes) })}
          aria-label="Keep model loaded"
          className="w-[240px]"
        />
      </SettingRow>
      {error && <Notice tone="danger">{error}</Notice>}
      {server && !server.isLocal && (
        <Notice tone="warning">
          This address isn't this computer, so your transcripts will be sent over the network to it.
        </Notice>
      )}
    </>
  );
}

/** Hosted APIs that speak OpenAI's chat format, with a quick model to suggest. */
const PROVIDERS = [
  { value: "https://api.openai.com/v1", label: "OpenAI", model: "gpt-4.1-mini" },
  { value: "https://api.anthropic.com/v1", label: "Anthropic", model: "claude-haiku-5-5" },
  { value: "https://generativelanguage.googleapis.com/v1beta/openai", label: "Google Gemini", model: "gemini-2.5-flash" },
  { value: "https://api.groq.com/openai/v1", label: "Groq", model: "llama-3.1-8b-instant" },
  { value: "https://api.mistral.ai/v1", label: "Mistral", model: "mistral-small-latest" },
  { value: "https://openrouter.ai/api/v1", label: "OpenRouter", model: "openai/gpt-4.1-mini" },
];
const CUSTOM = "custom";

function ApiSettings({ cleanup, save }: { cleanup: CleanupSettings; save: Save }) {
  const preset = PROVIDERS.find((p) => p.value === cleanup.apiUrl);
  const [custom, setCustom] = useState(!preset);
  const [url, setUrl] = useState(cleanup.apiUrl);
  const [key, setKey] = useState(cleanup.apiKey);
  const [model, setModel] = useState(cleanup.apiModel);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string }>();

  // Text fields save when left, so a half-typed key is never used.
  const commit = (patch: Partial<CleanupSettings>) => {
    setResult(undefined);
    save(patch);
  };

  const chooseProvider = (value: string) => {
    setResult(undefined);
    if (value === CUSTOM) {
      setCustom(true);
      return;
    }
    setCustom(false);
    setUrl(value);
    save({ apiUrl: value });
  };

  const test = async () => {
    const draft = { ...cleanup, apiUrl: url.trim(), apiKey: key.trim(), apiModel: model.trim() };
    setTesting(true);
    setResult(undefined);
    try {
      const reply = await api.testCleanupApi(draft);
      setResult({ ok: true, text: `It works. The model replied: “${reply.trim()}”` });
    } catch (e) {
      setResult({ ok: false, text: errorMessage(e) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <SettingRow icon={Cloud} label="Provider" description="Any service with an OpenAI-compatible API works.">
        <Select
          value={custom ? CUSTOM : cleanup.apiUrl}
          options={[...PROVIDERS, { value: CUSTOM, label: "Other" }]}
          onChange={chooseProvider}
          aria-label="Provider"
          className="w-[240px]"
        />
      </SettingRow>
      {custom && (
        <SettingRow icon={PlugZap} label="API address" description="The base URL, ending before /chat/completions.">
          <TextField
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onBlur={() => url.trim() !== cleanup.apiUrl && commit({ apiUrl: url.trim() })}
            placeholder="https://api.example.com/v1"
            spellCheck={false}
            aria-label="API address"
            className="w-[240px]"
          />
        </SettingRow>
      )}
      <SettingRow icon={KeyRound} label="API key" description="Kept in Wordflow's settings file on this computer.">
        <TextField
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onBlur={() => key.trim() !== cleanup.apiKey && commit({ apiKey: key.trim() })}
          placeholder="Paste your key"
          spellCheck={false}
          autoComplete="off"
          aria-label="API key"
          className="w-[240px]"
        />
      </SettingRow>
      <SettingRow icon={Brain} label="Model" description="Small, fast models are best for dictation.">
        <div className="flex items-center gap-2">
          <TextField
            value={model}
            onChange={(e) => setModel(e.target.value)}
            onBlur={() => model.trim() !== cleanup.apiModel && commit({ apiModel: model.trim() })}
            onKeyDown={(e) => e.key === "Enter" && test()}
            placeholder={preset?.model ?? "Model name"}
            spellCheck={false}
            aria-label="API model"
            className="w-[176px]"
          />
          <Button size="sm" loading={testing} disabled={!key.trim() || !model.trim()} onClick={test}>
            Test
          </Button>
        </div>
      </SettingRow>
      {result && <Notice tone={result.ok ? "success" : "danger"}>{result.text}</Notice>}
      <Notice tone="warning">Your transcripts are sent to this provider to be cleaned up, and it may charge for each one.</Notice>
    </>
  );
}

const KEEP_ALIVE_OPTIONS = [
  { value: "1", label: "1 minute" },
  { value: "5", label: "5 minutes" },
  { value: "15", label: "15 minutes" },
  { value: "60", label: "1 hour" },
  { value: "-1", label: "Until Ollama quits" },
];

function Notice({ tone, children }: { tone: "warning" | "danger" | "success"; children: ReactNode }) {
  const Icon = tone === "success" ? Check : TriangleAlert;
  return (
    <p
      className={cn(
        "flex items-start gap-2 px-5 py-3.5 type-small",
        tone === "danger" ? "text-danger" : tone === "success" ? "text-success" : "text-warning",
      )}
    >
      <Icon size={14} className="mt-0.5 shrink-0" />
      <span>{children}</span>
    </p>
  );
}
