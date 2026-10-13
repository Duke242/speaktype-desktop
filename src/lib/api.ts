// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";

export type Theme = "system" | "light" | "dark";
export type RecordingMode = "hold" | "toggle";

export type PillPosition =
  | "topLeft"
  | "topCenter"
  | "topRight"
  | "centerLeft"
  | "center"
  | "centerRight"
  | "bottomLeft"
  | "bottomCenter"
  | "bottomRight";

export interface DictionaryEntry {
  id: string;
  trigger: string;
  replacement: string;
  isEnabled: boolean;
  matchWholeWord: boolean;
  /** Added automatically from a correction, rather than by the user. */
  learned?: boolean;
}

export type CleanupBackend = "builtin" | "server";

/** Optional tidying of each transcript by a local language model. */
export interface CleanupSettings {
  enabled: boolean;
  backend: CleanupBackend;
  /** Id of a downloaded cleanup model, or "" when none is chosen. */
  builtinModel: string;
  /** Base URL of an OpenAI-compatible API, e.g. "http://localhost:11434/v1". */
  serverUrl: string;
  /** Model name as the server knows it. */
  serverModel: string;
  /** Minutes Ollama keeps the model loaded after a dictation; negative keeps it until Ollama quits. */
  serverKeepAliveMinutes: number;
}

/** How dictation into an app should read. "other" changes nothing. */
export type AppCategory = "chat" | "email" | "terminal" | "code" | "other";

/** The user's category for one app, matched by name, bundle id, executable or window class. */
export interface AppRule {
  app: string;
  category: AppCategory;
}

export interface ToneSettings {
  /** Match the tone to the app: instructions for AI cleanup, light rules without it. */
  enabled: boolean;
  /** Write spoken file names, identifiers and flags as code in editors and terminals. */
  formatCode: boolean;
  /** Checked before the built-in list. */
  apps: AppRule[];
}

/** Meeting notes: what to record, and whether to write notes afterwards. */
export interface MeetingSettings {
  /** Record what the computer plays along with the microphone. */
  captureSystemAudio: boolean;
  /** Id from `getMeetingSetup().systemAudioSources`; "" for the default output. */
  systemAudioSource: string;
  /** Write notes with the cleanup model when a meeting ends. */
  summarizeAutomatically: boolean;
}

export interface Settings {
  theme: Theme;
  hotkey: string;
  recordingMode: RecordingMode;
  liveTyping: boolean;
  selectedModel: string;
  language: string;
  recentLanguages: string[];
  inputDevice: string;
  /** Whisper mode: raise quiet speech to a normal level before transcribing. */
  boostQuietSpeech: boolean;
  /** Pause music and videos while recording, and resume them afterwards. */
  pauseMedia: boolean;
  launchAtLogin: boolean;
  autoEdit: boolean;
  smartTrailingPunctuation: boolean;
  cleanup: CleanupSettings;
  tone: ToneSettings;
  /** Shortcut that edits selected text by voice, or "" when off. */
  commandHotkey: string;
  restoreClipboard: boolean;
  /** Keep each dictation's audio so it can be replayed from History. */
  saveRecordings: boolean;
  /** Add misheard words to the dictionary when a transcript is corrected. */
  learnCorrections: boolean;
  alwaysShowPill: boolean;
  pillPosition: PillPosition;
  showTrayIcon: boolean;
  autoUpdate: boolean;
  dictionary: DictionaryEntry[];
  /** "Scratch that", "new line" and "new paragraph" act as commands. */
  voiceCommands: boolean;
  /** "Meet at 3, actually 4" is written "meet at 4". */
  selfCorrections: boolean;
  hasCompletedOnboarding: boolean;
  hasShownModelPrompt: boolean;
  /** Set once SpeakType 1's data has been brought over. */
  hasImportedV1: boolean;
  meetings: MeetingSettings;
}

export type OS = "macos" | "windows" | "linux";

export interface Status {
  os: OS;
  version: string;
  setupNotes: string[];
  hotkeyError: string | null;
  /** The hotkey is a single modifier key (like Fn), but macOS won't let us listen for it. */
  hotkeyBlocked: boolean;
  /** Single-modifier hotkeys this OS supports, e.g. "Fn". */
  modifierHotkeys: string[];
}

export interface InputDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export type EngineKind = "whisper" | "parakeet" | "llama";

export interface ModelStatus {
  id: string;
  name: string;
  engine: EngineKind;
  sizeMb: number;
  /** Size of a fresh download here, including Neural Engine files included by default. */
  downloadMb: number;
  /** Optional Neural Engine files for Whisper on macOS. */
  acceleratorMb: number;
  accelerator: "none" | "missing" | "installed";
  englishOnly: boolean;
  /** Languages it transcribes, or null for all Whisper languages. */
  languages: string[] | null;
  description: string;
  speed: number;
  accuracy: number;
  minRamGb: number;
  downloaded: boolean;
  downloading: boolean;
}

export interface CleanupStatus {
  /** Whether this build includes the helper that runs built-in models. */
  builtinAvailable: boolean;
  models: ModelStatus[];
}

export interface ServerModels {
  models: string[];
  /** False when the address isn't this computer, so transcripts would leave it. */
  isLocal: boolean;
}

export interface AppTone {
  /** Apps Wordfall already knows. */
  builtIn: { name: string; category: AppCategory }[];
  /** The app dictated into last, so users can see what to call it. */
  lastApp: { id: string; name: string; title: string; category: AppCategory } | null;
}

export interface DownloadProgress {
  id: string;
  downloaded: number;
  total: number;
}

export interface EngineStatus {
  loaded: string | null;
  loading: string | null;
}

export interface DeviceReport {
  device: {
    chip: string;
    ramGb: number;
    cores: number;
    gpu: boolean;
    summary: string;
    performanceTier: number;
  };
  recommendation: { modelId: string; reason: string };
}

export interface HistoryItem {
  id: string;
  createdAt: number;
  transcript: string;
  durationSecs: number;
  model: string;
  wordCount: number;
  audioPath: string | null;
}

export interface StatsEntry {
  createdAt: number;
  wordCount: number;
  durationSecs: number;
}

export type DictationState =
  | { phase: "idle" }
  | { phase: "recording"; startedAtMs: number }
  | { phase: "transcribing" };

export type PillState =
  | { phase: "idle" }
  /** `locked`: double-tapped in Hold mode, so it records without the hotkey held. */
  | { phase: "recording"; startedAtMs: number; locked: boolean }
  | { phase: "warming" }
  | { phase: "processing"; message: string };

export type PermissionKind = "microphone" | "accessibility";

export interface Permission {
  kind: PermissionKind;
  granted: boolean;
}

/** What was brought over from SpeakType 1. */
export interface ImportSummary {
  transcripts: number;
  dictionary: number;
  settings: boolean;
}

export interface LegacyStatus {
  /** SpeakType 1's data on this computer, if there is any. */
  available: { transcripts: number; dictionary: number } | null;
  /** What was brought over at first launch. Only reported once. */
  imported: ImportSummary | null;
}

export interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  latestVersion: string;
  notes: string;
  url: string;
}

/** The microphone is "You", what the computer played is "Others". */
export type MeetingSource = "microphone" | "system";

export interface MeetingSegment {
  /** Seconds from the start of the meeting. */
  startSecs: number;
  endSecs: number;
  source: MeetingSource;
  text: string;
}

export interface MeetingNotes {
  summary: string;
  actionItems: string[];
  openQuestions: string[];
}

export type MeetingStatus = "recording" | "processing" | "done";

export interface Meeting {
  id: string;
  title: string;
  createdAt: number;
  durationSecs: number;
  status: MeetingStatus;
  /** The tracks recorded. With both, segments are labelled "You" and "Others". */
  sources: MeetingSource[];
  /** In time order. */
  segments: MeetingSegment[];
  audioPath: string | null;
  model: string;
  notes: MeetingNotes | null;
  /** Why the last attempt at notes failed. */
  notesError: string | null;
  /** Notes are being written right now. */
  summarizing: boolean;
}

export interface MeetingListItem {
  id: string;
  title: string;
  createdAt: number;
  durationSecs: number;
  status: MeetingStatus;
  wordCount: number;
  /** The start of the transcript. */
  preview: string;
  hasNotes: boolean;
}

export type MeetingState =
  | { phase: "idle" }
  | {
      phase: "recording";
      id: string;
      title: string;
      startedAtMs: number;
      /** Whether what the computer plays is recorded too. */
      systemAudio: boolean;
      /** Why system audio couldn't be recorded, when it was asked for. */
      systemError: string | null;
    }
  /** Transcribing the last of the audio and saving the recording. */
  | { phase: "finishing"; id: string };

/** A place system audio can come from. */
export interface AudioSource {
  /** "" for the default output. */
  id: string;
  name: string;
  /** An output's sound, or an input device such as BlackHole. */
  kind: "output" | "input";
}

export interface MeetingSetup {
  /** What to know about recording system audio on this OS. */
  systemAudioNote: string | null;
  systemAudioSources: AudioSource[];
  /** Whether a cleanup model is set up to write notes. */
  notesAvailable: boolean;
}

/** Sent as `meeting-segment` while a meeting is transcribed. */
export interface MeetingSegmentEvent {
  meetingId: string;
  segment: MeetingSegment;
}

export const api = {
  getStatus: () => invoke<Status>("get_status"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  listInputDevices: () => invoke<InputDevice[]>("list_input_devices"),

  listModels: () => invoke<ModelStatus[]>("list_models"),
  getEngineStatus: () => invoke<EngineStatus>("get_engine_status"),
  getDeviceInfo: () => invoke<DeviceReport>("get_device_info"),
  downloadModel: (id: string, accelerator?: boolean) => invoke<void>("download_model", { id, accelerator }),
  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),

  getCleanupStatus: () => invoke<CleanupStatus>("get_cleanup_status"),
  listServerModels: (url: string) => invoke<ServerModels>("list_server_models", { url }),
  getAppTone: () => invoke<AppTone>("get_app_tone"),

  getHistory: () => invoke<HistoryItem[]>("get_history"),
  getStats: () => invoke<StatsEntry[]>("get_stats"),
  deleteHistoryItem: (id: string) => invoke<void>("delete_history_item", { id }),
  /** Saves a corrected transcript. Returns the words learned from the correction. */
  editHistoryItem: (id: string, transcript: string) =>
    invoke<DictionaryEntry[]>("edit_history_item", { id, transcript }),
  forgetWords: (ids: string[]) => invoke<void>("forget_words", { ids }),
  clearHistory: () => invoke<void>("clear_history"),
  readHistoryAudio: (id: string) => invoke<ArrayBuffer>("read_history_audio", { id }),
  revealHistoryAudio: (id: string) => invoke<void>("reveal_history_audio", { id }),

  toggleDictation: () => invoke<void>("toggle_dictation"),
  getDictationState: () => invoke<DictationState>("get_dictation_state"),

  getPermissions: () => invoke<Permission[]>("get_permissions"),
  requestPermission: (kind: PermissionKind) => invoke<void>("request_permission", { kind }),
  openPermissionSettings: (kind: PermissionKind) => invoke<void>("open_permission_settings", { kind }),

  checkForUpdate: () => invoke<UpdateInfo>("check_for_update"),
  getLegacyStatus: () => invoke<LegacyStatus>("get_legacy_status"),
  importLegacy: () => invoke<ImportSummary>("import_legacy"),

  getMeetingSetup: () => invoke<MeetingSetup>("get_meeting_setup"),
  getMeetingState: () => invoke<MeetingState>("get_meeting_state"),
  startMeeting: (title: string) => invoke<MeetingState>("start_meeting", { title }),
  stopMeeting: () => invoke<void>("stop_meeting"),
  /** Meetings whose title, transcript or notes contain `query`, newest first. */
  listMeetings: (query = "") => invoke<MeetingListItem[]>("list_meetings", { query }),
  getMeeting: (id: string) => invoke<Meeting>("get_meeting", { id }),
  renameMeeting: (id: string, title: string) => invoke<void>("rename_meeting", { id, title }),
  deleteMeeting: (id: string) => invoke<void>("delete_meeting", { id }),
  /** Writes notes with the cleanup model. Resolves once they're saved, which can take a few minutes. */
  summarizeMeeting: (id: string) => invoke<Meeting>("summarize_meeting", { id }),
  revealMeetingAudio: (id: string) => invoke<void>("reveal_meeting_audio", { id }),
  /** Saves Markdown to the Downloads folder and shows it. Returns the file's path. */
  exportMeetingMarkdown: (id: string, markdown: string) =>
    invoke<string>("export_meeting_markdown", { id, markdown }),

  openMainWindow: (route?: string) => invoke<void>("open_main_window", { route }),
  hideTrayPanel: () => invoke<void>("hide_tray_panel"),
  quitApp: () => invoke<void>("quit_app"),
};

/** Turns a rejected invoke into a readable message. */
export function errorMessage(error: unknown) {
  return typeof error === "string" ? error : error instanceof Error ? error.message : "Something went wrong";
}
