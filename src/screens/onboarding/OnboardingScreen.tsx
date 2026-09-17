import { ArrowDown, ArrowLeft, ArrowRight, Check, Feather, Lock, Zap, type LucideIcon } from "lucide-react";
import { useState, type ReactNode } from "react";
import { LogoMark } from "@/components/brand/LogoMark";
import { PillPreview } from "@/components/brand/PillPreview";
import { HotkeyPicker } from "@/components/settings/HotkeyPicker";
import { PermissionList } from "@/components/PermissionList";
import { Button, Card, IconTile, ProgressBar } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatModelSize } from "@/lib/format";
import { useStore } from "@/lib/store";

type Step = "welcome" | "permissions" | "model" | "ready";

/** A small multilingual model, so setup doesn't start with a large download. */
const STARTER_MODEL = "base";

export function OnboardingScreen() {
  const { status, models, updateSettings } = useStore();
  const steps: Step[] = ["welcome", ...(status.os === "macos" ? (["permissions"] as Step[]) : []), "model", "ready"];
  const [index, setIndex] = useState(0);
  const step = steps[index];
  const back = () => setIndex((i) => Math.max(0, i - 1));
  const next = () => setIndex((i) => Math.min(steps.length - 1, i + 1));
  const finish = () => updateSettings({ hasCompletedOnboarding: true, hasShownModelPrompt: true });
  // Dictation can't work without a model, so the model step can't be skipped.
  const blocked = step === "model" && !models.some((m) => m.downloaded);

  return (
    <div className="relative flex h-full flex-col bg-app">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-[70%] bg-glow" />

      <header data-tauri-drag-region className="relative flex h-14 shrink-0 items-center justify-center">
        <div className="pointer-events-none flex items-center gap-2">
          <LogoMark className="size-6" />
          <span className="type-label">SpeakType</span>
        </div>
      </header>

      <div key={step} className="relative flex flex-1 animate-fade-in flex-col items-center overflow-y-auto px-10">
        <div className="my-auto w-full max-w-[600px] py-8">
          {step === "welcome" && <Welcome />}
          {step === "permissions" && <Permissions />}
          {step === "model" && <ModelStep />}
          {step === "ready" && <Ready />}
        </div>
      </div>

      <footer className="relative flex shrink-0 items-center px-8 pt-4 pb-7">
        <div className="flex-1">
          {index > 0 && (
            <Button variant="ghost" icon={ArrowLeft} onClick={back}>
              Back
            </Button>
          )}
        </div>
        <div className="flex gap-1.5">
          {steps.map((s, i) => (
            <span
              key={s}
              className={cn(
                "h-1.5 rounded-full transition-all duration-300 ease-out-soft",
                i === index ? "w-6 bg-primary" : "w-1.5 bg-line-strong",
              )}
            />
          ))}
        </div>
        <div className="flex flex-1 justify-end">
          {step === "ready" ? (
            <Button variant="accent" size="lg" onClick={finish}>
              Start dictating
              <ArrowRight size={16} />
            </Button>
          ) : (
            <Button
              variant="accent"
              size="lg"
              disabled={blocked}
              title={blocked ? "Download a model to continue" : undefined}
              onClick={next}
            >
              {step === "welcome" ? "Get started" : "Continue"}
              <ArrowRight size={16} />
            </Button>
          )}
        </div>
      </footer>
    </div>
  );
}

function StepHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="text-center">
      <h1 className="type-display">{title}</h1>
      {children && <p className="mx-auto mt-3 max-w-[460px] type-body-lg text-ink-secondary">{children}</p>}
    </div>
  );
}

function Welcome() {
  const features: { icon: LucideIcon; title: string; description: string }[] = [
    { icon: Lock, title: "Private", description: "Your voice never leaves this computer." },
    { icon: Zap, title: "Fast", description: "Speech becomes text in about a second." },
    { icon: Feather, title: "Lightweight", description: "Tiny footprint, easy on your battery." },
  ];
  return (
    <div className="flex flex-col items-center">
      <PillPreview className="mb-10" />
      <StepHeader title="Welcome to SpeakType!">
        Hold a key, speak naturally, and your words appear in whatever app you're using.
      </StepHeader>
      <div className="mt-12 grid w-full grid-cols-3 gap-3">
        {features.map(({ icon, title, description }) => (
          <Card key={title} interactive padding="lg" className="flex flex-col items-center text-center">
            <IconTile icon={icon} tone="brand" size="md" />
            <h3 className="mt-4 type-section">{title}</h3>
            <p className="mt-1.5 type-body text-ink-secondary">{description}</p>
          </Card>
        ))}
      </div>
    </div>
  );
}

function Permissions() {
  return (
    <>
      <StepHeader title="Allow access">
        SpeakType only listens while you dictate, and types the result where your cursor is.
      </StepHeader>
      <div className="mt-10">
        <PermissionList />
      </div>
    </>
  );
}

function ModelStep() {
  const { models, progress, downloadModel, downloadErrors } = useStore();
  const model = models.find((m) => m.id === STARTER_MODEL);
  const p = model && progress[model.id];
  const fraction = p && p.total ? p.downloaded / p.total : 0;

  return (
    <>
      <StepHeader title="Get a speech model">
        Transcription happens on your computer. Start with a small model, and switch to a more accurate one any time.
      </StepHeader>

      {model && (
        <Card className="mt-10 flex items-center gap-4">
          <IconTile icon={model.downloaded ? Check : ArrowDown} tone={model.downloaded ? "success" : "brand"} size="md" />
          <div className="min-w-0 flex-1">
            <h2 className="type-label">{model.name}</h2>
            <p className="mt-0.5 type-small text-ink-secondary">
              {formatModelSize(model.downloadMb)} · Any language
            </p>
            {model.downloading && <ProgressBar value={fraction} className="mt-3" />}
            {downloadErrors[model.id] && <p className="mt-2 type-small text-danger">{downloadErrors[model.id]}</p>}
          </div>
          {model.downloaded ? (
            <span className="type-label text-success">Ready</span>
          ) : model.downloading ? (
            <span className="type-small font-medium text-ink-secondary tabular-nums">{Math.round(fraction * 100)}%</span>
          ) : (
            <Button variant="accent" onClick={() => downloadModel(model.id)}>
              Download
            </Button>
          )}
        </Card>
      )}
      {!models.some((m) => m.downloaded) && (
        <p className="mt-4 text-center type-small text-ink-muted">
          {model?.downloading ? "You can continue once the download finishes." : "Download a model to continue."}
        </p>
      )}
    </>
  );
}

function Ready() {
  const { settings } = useStore();
  return (
    <>
      <StepHeader title="Pick your hotkey">
        Click into any text field, {settings.recordingMode === "hold" ? "hold" : "press"} it and speak.
      </StepHeader>
      <Card className="mt-10 flex flex-col items-center gap-4 py-8">
        <HotkeyPicker />
        <p className="type-small text-ink-muted">You can change this later in Settings.</p>
      </Card>
    </>
  );
}
