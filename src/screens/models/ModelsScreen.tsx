import { ArrowDown, Check, Laptop, RotateCw, Sparkles, Trash2, TriangleAlert, X, Zap } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Badge, Button, Card, ConfirmDialog, IconButton, IconTile, Page, PageHeader, ProgressBar, Spinner } from "@/components/ui";
import { api, type ModelStatus } from "@/lib/api";
import { cn } from "@/lib/cn";
import { accuracyTier, formatBytes, formatLanguages, formatModelSize, speedTier } from "@/lib/format";
import { useDeviceReport, useEngineStatus } from "@/lib/hooks";
import { useStore } from "@/lib/store";

/** Model, speed, accuracy, size, action. Speed and accuracy fold into the model column in narrow windows. */
const COLUMNS =
  "grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-6 @min-[760px]:grid-cols-[minmax(0,1fr)_112px_112px_64px_148px]";

export function ModelsScreen() {
  const { models } = useStore();
  const report = useDeviceReport();
  const recommended = models.find((m) => m.id === report?.recommendation.modelId);
  const installed = models.filter((m) => m.downloaded);
  const available = models
    .filter((m) => !m.downloaded)
    .sort((a, b) => Number(b.id === recommended?.id) - Number(a.id === recommended?.id));

  return (
    <Page>
      <PageHeader
        title="AI Models"
        description="Every model runs on this computer. Pick the one SpeakType uses to transcribe."
        actions={
          report && (
            <span className="flex items-center gap-2 type-small text-ink-muted">
              <Laptop size={14} />
              {report.device.summary}
            </span>
          )
        }
      />

      {recommended && report && !recommended.downloaded && (
        <Recommendation model={recommended} reason={report.recommendation.reason} chip={report.device.chip} />
      )}

      {installed.length > 0 && (
        <ModelList title="On this computer" description="Click a model to use it for dictation.">
          {installed.map((model) => (
            <ModelRow key={model.id} model={model} recommended={model.id === recommended?.id} />
          ))}
        </ModelList>
      )}

      {available.length > 0 && (
        <ModelList title="Available to download" description="Larger models are more accurate. Smaller ones are faster.">
          {available.map((model) => (
            <ModelRow key={model.id} model={model} recommended={model.id === recommended?.id} />
          ))}
        </ModelList>
      )}
    </Page>
  );
}

/** Shown until the model recommended for this computer is downloaded. */
function Recommendation({ model, reason, chip }: { model: ModelStatus; reason: string; chip: string }) {
  return (
    <Card padding="none" className="mb-8 flex items-center gap-4 px-5 py-4">
      <IconTile icon={Sparkles} tone="brand" />
      <div className="min-w-0 flex-1">
        <div className="type-label">
          {model.name} is recommended for your {chip}
        </div>
        <p className="mt-0.5 type-small text-ink-secondary">{reason}</p>
      </div>
      <div className="w-[148px] shrink-0">
        <DownloadAction model={model} accent />
      </div>
    </Card>
  );
}

function ModelList({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  return (
    <section className="mb-8">
      <div className="mb-3 px-1">
        <h2 className="type-section">{title}</h2>
        <p className="mt-0.5 type-small text-ink-secondary">{description}</p>
      </div>
      <Card padding="none">
        <div className={cn(COLUMNS, "hidden border-b border-line-subtle px-5 py-2.5 type-caption text-ink-muted @min-[760px]:grid")}>
          <span>Model</span>
          <span>Speed</span>
          <span>Accuracy</span>
          <span className="text-right">Size</span>
          <span />
        </div>
        <div className="divide-y divide-line-subtle">{children}</div>
      </Card>
    </section>
  );
}

function ModelRow({ model, recommended }: { model: ModelStatus; recommended: boolean }) {
  const { settings, updateSettings, downloadErrors } = useStore();
  const engine = useEngineStatus();
  const active = model.downloaded && model.id === settings.selectedModel;
  const selectable = model.downloaded && !active;
  const error = !model.downloaded && !model.downloading ? downloadErrors[model.id] : undefined;
  const speed = speedTier(model.speed);
  const accuracy = accuracyTier(model.accuracy);

  return (
    <div
      role={model.downloaded ? "radio" : undefined}
      aria-checked={model.downloaded ? active : undefined}
      tabIndex={selectable ? 0 : undefined}
      onClick={selectable ? () => updateSettings({ selectedModel: model.id }) : undefined}
      onKeyDown={(e) => {
        if (selectable && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          updateSettings({ selectedModel: model.id });
        }
      }}
      className={cn(
        "group/row px-5 py-4 transition-colors first:rounded-t-card last:rounded-b-card",
        selectable && "cursor-pointer hover:bg-hover/50",
        active && "bg-accent-soft/40",
      )}
    >
      <div className={COLUMNS}>
        <div className="flex min-w-0 items-start gap-3.5">
          {model.downloaded && <SelectionMark active={active} />}
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
              <h3 className="type-label">{model.name}</h3>
              {recommended && <Badge tone="accent">Recommended</Badge>}
            </div>
            <p className="mt-0.5 line-clamp-1 type-small text-ink-secondary">{model.description}</p>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 type-caption text-ink-muted">
              <span>{formatLanguages(model)}</span>
              <span className="@min-[760px]:hidden">
                {speed} · {accuracy} · {formatModelSize(model.downloaded ? model.sizeMb : model.downloadMb)}
              </span>
              {model.accelerator === "installed" && (
                <span className="inline-flex items-center gap-1 text-ink-secondary">
                  <Zap size={11} strokeWidth={2.25} /> Neural Engine
                </span>
              )}
              {model.downloaded && model.accelerator === "missing" && !model.downloading && <SpeedUpLink model={model} />}
            </div>
          </div>
        </div>

        <Meter score={model.speed} tier={speed} className="hidden @min-[760px]:block" />
        <Meter score={model.accuracy} tier={accuracy} className="hidden @min-[760px]:block" />
        <span className="hidden text-right type-small text-ink-secondary tabular-nums @min-[760px]:block">
          {formatModelSize(model.downloaded ? model.sizeMb : model.downloadMb)}
        </span>

        <div className="flex items-center justify-end gap-1.5">
          {model.downloaded && !model.downloading ? (
            <>
              {active ? (
                engine.loading === model.id ? (
                  <span className="flex items-center gap-1.5 type-small text-ink-secondary">
                    <Spinner size={12} /> Loading
                  </span>
                ) : (
                  <Badge tone="accent" icon={Check}>
                    In use
                  </Badge>
                )
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
        <p className="mt-3 flex items-start gap-2 type-small text-danger">
          <TriangleAlert size={14} className="mt-0.5 shrink-0" />
          {error}
        </p>
      )}
    </div>
  );
}

function SelectionMark({ active }: { active: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "mt-0.5 grid size-[18px] shrink-0 place-items-center rounded-full transition-colors",
        active ? "bg-primary text-accent" : "border-[1.5px] border-line-strong group-hover/row:border-ink-muted",
      )}
    >
      {active && <Check size={11} strokeWidth={3} />}
    </span>
  );
}

/** Five segments, filled in proportion to a 0–10 score, with the tier name above. */
function Meter({ score, tier, className }: { score: number; tier: string; className?: string }) {
  return (
    <div className={className}>
      <div className="type-small text-ink">{tier}</div>
      <div className="mt-1.5 flex gap-1">
        {Array.from({ length: 5 }, (_, i) => {
          const fill = Math.min(1, Math.max(0, score / 2 - i));
          return (
            <span key={i} className="h-1 flex-1 overflow-hidden rounded-full bg-hover">
              <span className="block h-full rounded-full bg-ink" style={{ width: `${fill * 100}%` }} />
            </span>
          );
        })}
      </div>
    </div>
  );
}

/** Download, retry, or progress with cancel. */
function DownloadAction({ model, accent }: { model: ModelStatus; accent?: boolean }) {
  const { progress, downloadModel, downloadErrors } = useStore();

  if (model.downloading) {
    const p = progress[model.id];
    const fraction = p && p.total ? p.downloaded / p.total : 0;
    return (
      <div className="flex w-full items-center gap-2" onClick={(e) => e.stopPropagation()}>
        <div className="min-w-0 flex-1" title={p ? `${formatBytes(p.downloaded)} of ${formatBytes(p.total)}` : undefined}>
          <div className="flex items-center justify-between type-caption">
            <span className="text-ink-muted">Downloading</span>
            <span className="font-medium text-ink tabular-nums">{Math.round(fraction * 100)}%</span>
          </div>
          <ProgressBar value={fraction} className="mt-1.5 h-1" />
        </div>
        <IconButton icon={X} label="Cancel download" size="sm" onClick={() => api.cancelDownload(model.id)} />
      </div>
    );
  }

  const failed = Boolean(downloadErrors[model.id]);
  return (
    <Button
      variant={accent ? "accent" : "secondary"}
      size="sm"
      icon={failed ? RotateCw : ArrowDown}
      className={cn(accent && "w-full")}
      onClick={(e) => {
        e.stopPropagation();
        downloadModel(model.id);
      }}
    >
      {failed ? "Try again" : "Download"}
    </Button>
  );
}

/** Neural Engine files that didn't come with the download (large models) can be added later. */
function SpeedUpLink({ model }: { model: ModelStatus }) {
  const { downloadModel } = useStore();
  return (
    <button
      type="button"
      title="Runs part of the model on the Neural Engine, about 15–30% faster"
      onClick={(e) => {
        e.stopPropagation();
        downloadModel(model.id, true);
      }}
      className="inline-flex items-center gap-1 font-medium text-accent-ink hover:underline"
    >
      <Zap size={11} strokeWidth={2.25} />
      Speed up with the Neural Engine · {formatModelSize(model.acceleratorMb)}
    </button>
  );
}

function DeleteButton({ model }: { model: ModelStatus }) {
  const [confirming, setConfirming] = useState(false);
  return (
    <div onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
      <IconButton icon={Trash2} label="Delete model" tone="danger" size="sm" onClick={() => setConfirming(true)} />
      <ConfirmDialog
        open={confirming}
        onClose={() => setConfirming(false)}
        onConfirm={() => api.deleteModel(model.id)}
        title={`Delete ${model.name}?`}
        description={`This frees ${formatModelSize(model.sizeMb)}. You can download it again at any time.`}
        confirmLabel="Delete"
      />
    </div>
  );
}
