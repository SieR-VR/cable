import { LazyStore } from "@tauri-apps/plugin-store";

/**
 * Single source of truth for the persisted-settings file.
 *
 * All readers/writers (state.ts, components/Menu.tsx, ...) share this one
 * `LazyStore` instance so that every change goes through the same in-memory
 * cache and reaches the backend's tauri-plugin-store handle. Adding a new
 * persisted setting is a three-step recipe:
 *   1. add a key here in `SETTING_KEYS`,
 *   2. wire it into the Zustand store in `state.ts`,
 *   3. (optional) mirror the key as a `const` in `crates/tauri/src/lib.rs`
 *      if the backend needs to read it.
 *
 * Keep the JSON keys in sync between this file and `lib.rs`.
 */
export const SETTINGS_FILE = "settings.json";

export const settingsStore = new LazyStore(SETTINGS_FILE);

export const SETTING_KEYS = {
  audioHost: "audioHost",
  bufferSize: "bufferSize",
  minimizeToTray: "minimizeToTrayEnabled",
  bluetoothBatteryEnabled: "bluetoothBatteryEnabled",
  virtualDevices: "virtualDevices",
} as const;

export type SettingKey = (typeof SETTING_KEYS)[keyof typeof SETTING_KEYS];

/**
 * Set + flush a single setting. Errors are surfaced to the caller; callers
 * typically log-and-ignore so that a transient persistence failure doesn't
 * block in-memory state updates.
 */
export async function persistSetting(key: SettingKey, value: unknown): Promise<void> {
  await settingsStore.set(key, value);
  await settingsStore.save();
}

/** Read a single setting. Returns undefined if unset or on error. */
export async function readSetting<T>(key: SettingKey): Promise<T | undefined> {
  try {
    const v = await settingsStore.get<T>(key);
    return v ?? undefined;
  } catch (e) {
    console.warn(`Failed to read setting ${key}:`, e);
    return undefined;
  }
}
