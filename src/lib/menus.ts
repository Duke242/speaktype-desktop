import { Menu } from "@tauri-apps/api/menu";
import { api, type Settings } from "@/lib/api";
import { LANGUAGES, languageName } from "@/lib/languages";

const QUICK_LANGUAGES = ["en", "es", "fr", "de", "hi", "pt", "ja", "zh"];

async function save(patch: Partial<Settings>) {
  const settings = await api.getSettings();
  await api.saveSettings({ ...settings, ...patch });
}

/** Native menus open outside small windows like the pill and menu bar panel, so they never get clipped. */
export async function showMicrophoneMenu(settings: Settings) {
  const devices = await api.listInputDevices();
  const menu = await Menu.new({
    items: [
      { text: "System default", checked: settings.inputDevice === "", action: () => save({ inputDevice: "" }) },
      ...(devices.length ? [{ item: "Separator" as const }] : []),
      ...devices.map((device) => ({
        text: device.name,
        checked: device.id === settings.inputDevice,
        action: () => save({ inputDevice: device.id }),
      })),
    ],
  });
  await menu.popup();
}

export async function showModeMenu(settings: Settings) {
  const menu = await Menu.new({
    items: [
      { text: "Hold to talk", checked: settings.recordingMode === "hold", action: () => save({ recordingMode: "hold" }) },
      { text: "Toggle on and off", checked: settings.recordingMode === "toggle", action: () => save({ recordingMode: "toggle" }) },
    ],
  });
  await menu.popup();
}

export async function showLanguageMenu(settings: Settings) {
  const choose = (language: string) => {
    const recentLanguages =
      language === "auto"
        ? settings.recentLanguages
        : [language, ...settings.recentLanguages.filter((c) => c !== language)].slice(0, 5);
    return save({ language, recentLanguages });
  };
  const quick = [...new Set([settings.language, ...settings.recentLanguages, ...QUICK_LANGUAGES])]
    .filter((code) => code !== "auto")
    .slice(0, 6);

  const menu = await Menu.new({
    items: [
      { text: "Detect automatically", checked: settings.language === "auto", action: () => choose("auto") },
      { item: "Separator" },
      ...quick.map((code) => ({
        text: languageName(code),
        checked: settings.language === code,
        action: () => choose(code),
      })),
      { item: "Separator" },
      {
        text: "More languages",
        items: LANGUAGES.map(([code, name]) => ({
          text: name,
          checked: settings.language === code,
          action: () => choose(code),
        })),
      },
    ],
  });
  await menu.popup();
}

export async function showModelMenu(settings: Settings) {
  const models = (await api.listModels()).filter((m) => m.downloaded);
  const menu = await Menu.new({
    items: [
      ...models.map((model) => ({
        text: model.name,
        checked: model.id === settings.selectedModel,
        action: () => save({ selectedModel: model.id }),
      })),
      ...(models.length ? [{ item: "Separator" as const }] : []),
      { text: "Manage models…", action: () => api.openMainWindow("models") },
    ],
  });
  await menu.popup();
}
