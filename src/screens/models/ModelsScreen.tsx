import { ArrowDown, ArrowRight, Check, CircleCheck, HardDrive, Laptop, RotateCw, Sparkles, Trash2, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  ConfirmDialog,
  IconButton,
  IconTile,
  Page,
  PageHeader,
  ProgressBar,
  Spinner,
} from "@/components/ui";
import { api, type ModelStatus } from "@/lib/api";
import { cn } from "@/lib/cn";
import { accuracyTier, formatBytes, formatModelSize, speedTier } from "@/lib/format";
import { useDeviceReport, useEngineStatus } from "@/lib/hooks";
import { useStore } from "@/lib/store";

export function ModelsScreen() {
  const { models, settings } = useStore();
  const report = useDeviceReport();
  const engine = useEngineStatus();
  const recommended = models.find((m) => m.id === report?.recommendation.modelId);
  const others = models.filter((m) => m.id !== recommended?.id);
  const selected = models.find((m) => m.id === settings.selectedModel && m.downloaded);

  return (
    <Page>
      <PageHeader
        title="AI Models"
        description="Every model runs on this computer. Pick the one SpeakType uses to transcribe."
        actions={
          selected && (
            <Badge tone={engine.loaded === selected.id ? "success" : "neutral"} icon={engine.loaded === selected.id ? Check : undefined}>
              {engine.loading === selected.id ? "Loading…" : `Using ${selected.name}`}
            </Badge>
          )
        }
      />

      {recommended && report && (
        <RecommendedCard model={recommended} reason={report.recommendation.reason} deviceSummary={report.device.summary} />
      )}

      <section className="mt-10">
        <div className="mb-3 px-1">
          <h2 className="type-section">All models</h2>
          <p className="mt-0.5 type-small text-ink-secondary">Larger models are more accurate. Smaller ones are faster.</p>
        </div>
        <div className="flex flex-col gap-2.5">
          {others.map((model) => (
            <ModelRow key={model.id} model={model} />
          ))}
        </div>
      </section>
    </Page>
  );
}

function RecommendedCard({
  model,
  reason,
  deviceSummary,
}: {
  model: ModelStatus;
  reason: string;
  deviceSummary: string;
}) {
  return (
    <Card padding="none" className="relative overflow-hidden">
      <div className="relative grid grid-cols-[minmax(0,1fr)_280px] gap-8 p-7">
        <div className="flex flex-col">
          <div className="flex items-center gap-3">
            <IconTile icon={Sparkles} tone="brand" size="md" />
            <div>
              <h2 className="type-title">{model.name}</h2>
              <div className="mt-1 flex items-center gap-2">
                <Badge tone="brand">Recommended for you</Badge>
                <LanguageBadge model={model} />
              </div>
            </div>
          </div>
          <p className="mt-5 max-w-[460px] type-body-lg text-ink-secondary">{reason}</p>
          <p className="mt-3 flex items-center gap-2 type-small text-ink-muted">
            <Laptop size={14} /> {deviceSummary}
          </p>
          <div className="mt-auto pt-6">
            <ModelAction model={model} large />
          </div>
        </div>

        <div className="flex flex-col gap-4 self-start rounded-card bg-surface-sunken p-5">
          <MetricBar label="Speed" value={model.speed} tier={speedTier(model.speed)} />
          <MetricBar label="Accuracy" value={model.accuracy} tier={accuracyTier(model.accuracy)} />
          <p className="flex items-center gap-2 border-t border-line-subtle pt-4 type-small text-ink-secondary">
            <HardDrive size={14} /> {formatModelSize(model.sizeMb)} download
          </p>
        </div>
      </div>
    </Card>
  );
}

function ModelRow({ model }: { model: ModelStatus }) {
  const { settings } = useStore();
  const active = model.downloaded && model.id === settings.selectedModel;

  return (
    <Card padding="none" interactive className={cn(active && "border-ink ring-1 ring-ink")}>
      <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-8 px-6 py-5">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="type-section">{model.name}</h3>
            {active && (
              <Badge tone="brand" icon={Check}>
                In use
              </Badge>
            )}
          </div>
          <p className="mt-1 type-body text-ink-secondary">{model.description}</p>
          <div className="mt-4 flex flex-wrap items-center gap-x-6 gap-y-2 type-small text-ink-secondary">
            <MetricBar label="Speed" value={model.speed} tier={speedTier(model.speed)} className="w-44" />
            <MetricBar label="Accuracy" value={model.accuracy} tier={accuracyTier(model.accuracy)} className="w-44" />
            <span className="h-4 w-px bg-line" />
            <span>{formatModelSize(model.sizeMb)}</span>
            <span>{model.englishOnly ? "English only" : "Multilingual"}</span>
            {model.downloaded && !active && <span className="font-medium text-success">Installed</span>}
          </div>
        </div>
        <ModelAction model={model} />
      </div>
      {model.downloading && <DownloadProgressBar model={model} />}
      <DownloadError model={model} />
    </Card>
  );
}

/** Download, use, cancel or delete, depending on the model's state. Shared by the recommendation and rows. */
function ModelAction({ model, large }: { model: ModelStatus; large?: boolean }) {
  const { settings, updateSettings, downloadModel, downloadErrors } = useStore();
  const engine = useEngineStatus();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const size = large ? "lg" : "md";
  const active = model.downloaded && model.id === settings.selectedModel;

  if (model.downloading) {
    if (large) return <DownloadProgressBar model={model} inline />;
    return (
      <Button size={size} icon={X} onClick={() => api.cancelDownload(model.id)}>
        Cancel
      </Button>
    );
  }

  if (!model.downloaded) {
    const failed = Boolean(downloadErrors[model.id]);
    return (
      <div className="flex flex-col items-start gap-3">
        {large && <DownloadError model={model} inline />}
        <Button variant={large ? "accent" : "secondary"} size={size} icon={failed ? RotateCw : ArrowDown} onClick={() => downloadModel(model.id)}>
          {failed ? "Try again" : "Download"}
        </Button>
      </div>
    );
  }

  const deleteButton = (
    <>
      <IconButton icon={Trash2} label="Delete model" tone="danger" onClick={() => setConfirmDelete(true)} />
      <ConfirmDialog
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={() => api.deleteModel(model.id)}
        title={`Delete ${model.name}?`}
        description={`This frees ${formatModelSize(model.sizeMb)}. You can download it again at any time.`}
        confirmLabel="Delete"
      />
    </>
  );

  if (active) {
    const loading = engine.loading === model.id;
    if (!large) {
      return (
        <div className="flex items-center gap-2">
          {loading && <Spinner size={14} className="mr-1 text-ink-muted" />}
          {deleteButton}
        </div>
      );
    }
    return loading ? (
      <span className="flex items-center gap-2 type-label text-ink-secondary">
        <Spinner size={14} className="text-accent-ink" /> Getting it ready…
      </span>
    ) : (
      <span className="flex items-center gap-2 type-label text-success">
        <CircleCheck size={16} /> This is your default model
      </span>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <Button variant={large ? "accent" : "secondary"} size={size} onClick={() => updateSettings({ selectedModel: model.id })}>
        Use this model
        <ArrowRight size={16} />
      </Button>
      {deleteButton}
    </div>
  );
}

function DownloadProgressBar({ model, inline }: { model: ModelStatus; inline?: boolean }) {
  const { progress } = useStore();
  const p = progress[model.id];
  const fraction = p && p.total ? p.downloaded / p.total : 0;

  return (
    <div className={cn(inline ? "w-[360px]" : "border-t border-line-subtle px-5 pt-3 pb-4")}>
      <div className="mb-2 flex items-center gap-2 type-small">
        <Spinner size={12} className="text-accent-ink" />
        <span className="text-ink-secondary">
          Downloading{p ? ` · ${formatBytes(p.downloaded)} of ${formatBytes(p.total)}` : "…"}
        </span>
        <span className="flex-1" />
        <span className="font-medium text-ink tabular-nums">{Math.round(fraction * 100)}%</span>
        {inline && (
          <IconButton icon={X} label="Cancel download" size="sm" onClick={() => api.cancelDownload(model.id)} />
        )}
      </div>
      <ProgressBar value={fraction} />
    </div>
  );
}

function DownloadError({ model, inline }: { model: ModelStatus; inline?: boolean }) {
  const { downloadErrors } = useStore();
  const error = downloadErrors[model.id];
  if (!error || model.downloading || model.downloaded) return null;
  return (
    <p className={cn("flex items-start gap-2 type-small text-danger", !inline && "border-t border-line-subtle px-5 py-3")}>
      <TriangleAlert size={14} className="mt-0.5 shrink-0" />
      {error}
    </p>
  );
}

function LanguageBadge({ model }: { model: ModelStatus }) {
  return model.englishOnly ? <Badge tone="neutral">English</Badge> : <Badge tone="neutral">Multilingual</Badge>;
}

function MetricBar({ label, value, tier, className }: { label: string; value: number; tier: string; className?: string }) {
  return (
    <div className={cn("flex items-center gap-2.5", className)}>
      <span className="shrink-0 type-small text-ink-secondary">{label}</span>
      <div className="h-1 flex-1 overflow-hidden rounded-full bg-hover">
        <div className="h-full rounded-full bg-ink" style={{ width: `${value * 10}%` }} />
      </div>
      <span className="shrink-0 type-small font-medium text-ink">{tier}</span>
    </div>
  );
}
