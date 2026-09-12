// @pxl/web/app/settings/settings_store.ts

/**
 * The one settings store: what the sheet edits, what the drawer's character
 * picker edits, and what every consumer reads, user-facing and dev alike.
 *
 * Two layers, read as one. `DEFAULT_SETTINGS` (schema.ts) is the fallback;
 * `overrides` is what the user has set differently; the flat fields on the
 * state are the two merged, so a consumer reads `state.show_poi` and never
 * asks which layer it came from. Only the overrides are stored, and an
 * override equal to the default is dropped rather than kept: a default
 * that has been written to storage is no longer a default — it is a
 * decision nobody made, and changing the value in the source would then
 * reach only people with empty storage. A stored override is checked
 * against the schema on load, so a setting that no longer exists, or a
 * choice that no longer has that option, falls back rather than lingers.
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";

import { DEFAULT_SETTINGS, is_valid_setting, SETTING_KEYS, type SettingKey, type SettingValues, type ToggleKey } from "./schema.ts";

export type SettingsStore = SettingValues & {
  /** The user's layer: only what differs from the defaults. */
  overrides: Partial<SettingValues>;
  set_setting: <K extends SettingKey>(key: K, value: SettingValues[K]) => void;
  toggle_setting: (key: ToggleKey) => void;
};

function only_valid(candidate: unknown): Partial<SettingValues> {
  const overrides: Record<string, unknown> = {};
  if (typeof candidate !== "object" || candidate === null) return overrides;
  for (const key of SETTING_KEYS) {
    const value = (candidate as Record<string, unknown>)[key];
    if (value !== undefined && is_valid_setting(key, value) && !Object.is(value, DEFAULT_SETTINGS[key])) overrides[key] = value;
  }
  return overrides;
}

export const useSettingsStore = create<SettingsStore>()(persist((set, get) => ({
  ...DEFAULT_SETTINGS,
  overrides: {},

  set_setting: (key, value) => {
    if (Object.is(get()[key], value)) return;
    const overrides = { ...get().overrides };
    if (Object.is(value, DEFAULT_SETTINGS[key])) delete overrides[key];
    else overrides[key] = value;
    set({ [key]: value, overrides } as Partial<SettingsStore>);
  },
  toggle_setting: (key) => get().set_setting(key, !get()[key]),
}), {
  name: "pxl:settings",
  partialize: (state) => ({ overrides: state.overrides }),
  merge: (persisted, current) => {
    const overrides = only_valid((persisted as { overrides?: unknown } | undefined)?.overrides);
    return { ...current, ...overrides, overrides };
  },
}));
