/**
 * Lets the UI run in a plain browser (`npm run dev`, then open localhost:1420)
 * with sample data, for design work without the native app. Never used inside Tauri.
 */
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

export function installBrowserPreview() {
  if ("__TAURI_INTERNALS__" in window) return;

  const now = Date.now();
  const day = 86_400_000;
  const params = new URLSearchParams(location.search);
  // Add ?os=windows or ?os=linux to preview another system.
  const os = params.get("os") ?? "macos";

  let settings = {
    theme: (params.get("theme") ?? "light") as "light",
    hotkey: os === "macos" ? "Fn" : "Ctrl+Shift+Space",
    recordingMode: "hold",
    liveTyping: true,
    selectedModel: "small-en",
    language: "auto",
    recentLanguages: ["en", "hi"],
    inputDevice: "",
    autoEdit: false,
    smartTrailingPunctuation: true,
    cleanup: {
      enabled: params.get("cleanup") !== null,
      backend: (params.get("cleanup") === "server" ? "server" : "builtin") as "builtin" | "server",
      builtinModel: "qwen2.5-1.5b",
      serverUrl: "http://localhost:11434/v1",
      serverModel: "llama3.2",
    },
    restoreClipboard: true,
    saveRecordings: true,
    learnCorrections: true,
    alwaysShowPill: false,
    pillPosition: "bottomCenter",
    showTrayIcon: true,
    autoUpdate: true,
    dictionary: [
      { id: "1", trigger: "my email", replacement: "karan@example.com", isEnabled: true, matchWholeWord: true },
      { id: "2", trigger: "word fall", replacement: "Wordfall", isEnabled: true, matchWholeWord: true },
      { id: "3", trigger: "um", replacement: "", isEnabled: false, matchWholeWord: false },
      { id: "4", trigger: "cooper netties", replacement: "Kubernetes", isEnabled: true, matchWholeWord: true, learned: true },
    ],
    hasCompletedOnboarding: params.get("onboarding") !== "1",
    hasShownModelPrompt: true,
    hasImportedV1: false,
  };

  const transcripts = [
    "Let's move the design review to Thursday afternoon so the whole team can make it.",
    "Remind me to send the invoice to the client before the end of the week.",
    "The new onboarding flow feels much faster. I think we should ship it behind a flag first and watch the numbers.",
    "karan@example.com",
    "Can you pull the latest numbers for the quarterly report and share them in the channel?",
    "Great talk today. Let's follow up next week on the pricing changes.",
  ];
  const history = transcripts.map((transcript, i) => ({
    id: String(i),
    createdAt: now - i * day * 0.6 - 3_600_000,
    transcript,
    durationSecs: 3 + i * 2.5,
    model: "Whisper Large v3 Turbo (compressed)",
    wordCount: transcript.split(/\s+/).length,
    audioPath: i % 2 === 0 ? `/tmp/recording-${i}.wav` : null,
  }));
  const stats = Array.from({ length: 140 }, (_, i) => ({
    createdAt: now - Math.floor(i * i * 0.004 * day) - i * 1_800_000,
    wordCount: 8 + ((i * 37) % 60),
    durationSecs: 4 + ((i * 13) % 20),
  }));

  const models = [
    ["parakeet-tdt-v3", "Parakeet v3", "parakeet", 640, 0, false, 25, 9.7, 9.2, false],
    ["parakeet-tdt-v2", "Parakeet v2 (English)", "parakeet", 631, 0, true, 1, 9.8, 9.1, false],
    ["large-v3-turbo-q5", "Whisper Large v3 Turbo (compressed)", "whisper", 547, 1119, false, null, 7.5, 9.4, true],
    ["large-v3-turbo", "Whisper Large v3 Turbo", "whisper", 1624, 1119, false, null, 7.0, 9.5, false],
    ["small-en", "Whisper Small (English)", "whisper", 466, 155, true, 1, 8.0, 8.5, true],
    ["base-en", "Whisper Base (English)", "whisper", 142, 36, true, 1, 9.0, 7.5, false],
    ["base", "Whisper Base", "whisper", 142, 36, false, null, 9.0, 7.3, false],
    ["tiny", "Whisper Tiny", "whisper", 75, 14, false, null, 9.5, 6.0, false],
  ].map(([id, name, engine, sizeMb, acceleratorMb, englishOnly, languageCount, speed, accuracy, downloaded]) => ({
    id,
    name,
    engine,
    sizeMb,
    acceleratorMb,
    downloadMb: (sizeMb as number) + ((acceleratorMb as number) <= 200 ? (acceleratorMb as number) : 0),
    accelerator: engine === "parakeet" ? "none" : id === "small-en" ? "installed" : "missing",
    englishOnly,
    languages: languageCount === null ? null : Array.from({ length: languageCount as number }, (_, i) => `l${i}`),
    description: "A sample description for the browser preview.",
    speed,
    accuracy,
    minRamGb: 4,
    // Add ?nomodel=1 to preview a fresh install with nothing downloaded.
    downloaded: downloaded && !params.get("nomodel"),
    downloading: false,
  }));

  const cleanupModels = [
    ["qwen2.5-1.5b", "Qwen 2.5 1.5B", 1117, "Quick on any computer, and writes 29 languages. The one to start with.", true],
    ["llama3.2-3b", "Llama 3.2 3B", 2020, "More careful edits, about twice as slow. Best with a GPU or Apple Silicon.", false],
  ].map(([id, name, sizeMb, description, downloaded]) => ({
    id,
    name,
    engine: "llama",
    sizeMb,
    acceleratorMb: 0,
    downloadMb: sizeMb,
    accelerator: "none",
    englishOnly: false,
    languages: null,
    description,
    speed: 9,
    accuracy: 7.5,
    minRamGb: 4,
    downloaded,
    downloading: false,
  }));

  const cleanupModels = [
    ["qwen2.5-1.5b", "Qwen 2.5 1.5B", 1117, "Quick on any computer, and writes 29 languages. The one to start with.", true],
    ["llama3.2-3b", "Llama 3.2 3B", 2020, "More careful edits, about twice as slow. Best with a GPU or Apple Silicon.", false],
  ].map(([id, name, sizeMb, description, downloaded]) => ({
    id,
    name,
    engine: "llama",
    sizeMb,
    acceleratorMb: 0,
    downloadMb: sizeMb,
    accelerator: "none",
    englishOnly: false,
    languages: null,
    description,
    speed: 9,
    accuracy: 7.5,
    minRamGb: 4,
    downloaded,
    downloading: false,
  }));

  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      switch (cmd) {
        case "get_status":
          return {
            os,
            version: "2.0.0",
            setupNotes: [],
            // Add ?hotkeyError=1 to preview a hotkey the system wouldn't register.
            hotkeyError: params.get("hotkeyError") ? "Ctrl+Shift+Space is already used by another app" : null,
            modifierHotkeys: os === "macos" ? ["Fn", "RightCommand", "LeftCommand", "RightOption", "LeftOption", "RightControl", "LeftControl"] : [],
          };
        case "get_settings":
          return settings;
        case "save_settings":
          settings = a.settings as typeof settings;
          return settings;
        case "list_models":
          return models;
        case "get_cleanup_status":
          return { builtinAvailable: true, models: cleanupModels };
        case "list_server_models":
          return { models: ["gemma3:4b", "llama3.2", "qwen2.5:1.5b"], isLocal: String(a.url).includes("localhost") };
        case "get_engine_status":
          // Add ?warming=1 to preview the model loading.
          return params.get("warming") ? { loaded: null, loading: "small-en" } : { loaded: "small-en", loading: null };
        case "read_history_audio":
          return sampleRecording(history.find((h) => h.id === a.id)?.durationSecs ?? 3);
        case "get_device_info":
          return {
            device: { chip: "Apple M3 Pro", ramGb: 18, cores: 11, gpu: true, summary: "Apple M3 Pro · 18 GB · Metal", performanceTier: 0.95 },
            recommendation: {
              modelId: "parakeet-tdt-v3",
              reason: "Fast and accurate enough for live dictation, and loads quickly on your Apple M3 Pro.",
            },
          };
        case "get_history":
          return params.get("empty") ? [] : history;
        case "get_stats":
          return params.get("empty") ? [] : stats;
        case "edit_history_item": {
          const item = history.find((h) => h.id === a.id);
          if (!item) return [];
          const transcript = String(a.transcript);
          item.transcript = transcript;
          item.wordCount = transcript.split(/\s+/).length;
          // Pretend any new capitalized word was a misheard name.
          const learned = transcript
            .split(/\s+/)
            .map((w) => w.replace(/[^\p{L}\p{N}]/gu, ""))
            .filter((w) => /^\p{Lu}/u.test(w) && !transcripts.some((t) => t.includes(w)))
            .slice(0, 1)
            .map((word) => {
              const entry = { id: `learned-${word}`, trigger: word.toLowerCase(), replacement: word, isEnabled: true, matchWholeWord: true, learned: true };
              settings.dictionary.push(entry);
              return entry;
            });
          return learned;
        }
        case "forget_words":
          settings.dictionary = settings.dictionary.filter((e) => !(a.ids as string[]).includes(e.id));
          return null;
        case "list_input_devices":
          return [
            { id: "coreaudio:1", name: "MacBook Pro Microphone", isDefault: true },
            { id: "coreaudio:2", name: "AirPods Pro", isDefault: false },
          ];
        case "get_dictation_state":
          return { phase: "idle" };
        case "get_permissions":
          return [
            { kind: "microphone", granted: true },
            { kind: "accessibility", granted: false },
          ];
        case "get_legacy_status":
          return {
            available: { transcripts: 21, dictionary: 3 },
            // Add ?imported=1 to preview onboarding after an upgrade from SpeakType 1.
            imported: params.get("imported") ? { transcripts: 21, dictionary: 3, settings: true } : null,
          };
        case "import_legacy":
          return { transcripts: 21, dictionary: 3, settings: false };
        case "check_for_update":
          return { available: false, currentVersion: "2.0.0", latestVersion: "2.0.0", notes: "", url: "" };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}

/** A WAV of a soft tone that swells and pauses like speech, so playback can be previewed. */
function sampleRecording(seconds: number): ArrayBuffer {
  const rate = 8000;
  const samples = Math.round(seconds * rate);
  const buffer = new ArrayBuffer(44 + samples * 2);
  const view = new DataView(buffer);
  const text = (offset: number, value: string) =>
    [...value].forEach((c, i) => view.setUint8(offset + i, c.charCodeAt(0)));
  text(0, "RIFF");
  view.setUint32(4, 36 + samples * 2, true);
  text(8, "WAVEfmt ");
  view.setUint32(16, 16, true); // format chunk size
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, rate, true);
  view.setUint32(28, rate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  text(36, "data");
  view.setUint32(40, samples * 2, true);
  for (let i = 0; i < samples; i++) {
    const t = i / rate;
    const envelope = Math.max(0, Math.sin(t * 2.3)) * (0.5 + 0.5 * Math.sin(t * 7.1) ** 2);
    view.setInt16(44 + i * 2, Math.round(envelope * 0.3 * Math.sin(2 * Math.PI * 220 * t) * 32767), true);
  }
  return buffer;
}
