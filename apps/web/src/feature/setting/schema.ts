// @pxl/web/feature/setting/schema.ts

/**
 * Every setting the app has, declared once: its default, its kind, and
 * where the settings sheet shows it. The store (store.ts) reads
 * the defaults from here and the sheet (section.tsx) renders
 * rows from here, so adding a setting is one entry in SETTINGS plus its
 * two strings in the locale catalogue under `settings.<section>.<key>`.
 *
 * The values are the fallback layer of a two-layer model, the way an
 * editor keeps a default settings file under the user's own: what the app
 * reads is the defaults with the user's overrides on top, and only the
 * overrides are stored. See store.ts for why that matters.
 *
 * A `dev` setting exists in every build — the store is the same — but is
 * only offered in the sheet under `__IS_DEV__`, and its strings live in
 * the dev locale catalogue, so a production bundle carries neither the row
 * nor its name.
 */

import { HANDHELD } from "@/shared/lib/browser/handheld.ts";

import { CAMERA_TERRAIN_MODES as MODES, SETTINGS as SPEC } from "./schema.spec.ts";

/** Rough per-frame cost, shown as a badge on the row. */
export type SettingCost = "low" | "med" | "high";

/** The sheet's sections, in the order they are shown (schema.json). */
export type SettingSection = "display" | "camera" | "editing" | "lighting" | "quality" | "debug" | "character";

interface Common {
  section: SettingSection;
  cost?: SettingCost;
  dev?: boolean;
}
export type ToggleSetting = Common & { kind: "toggle"; default: boolean };
export type ChoiceSetting<T extends string = string> = Common & { kind: "choice"; default: T; options: readonly T[] };
export type TextSetting = Common & { kind: "text"; default: string };
export type Setting = ToggleSetting | ChoiceSetting | TextSetting;

/** What terrain does to the camera (camera/terrain_gate.ts); schema.json says each. */
export const CAMERA_TERRAIN_MODES = MODES;
export type CameraTerrainMode = (typeof CAMERA_TERRAIN_MODES)[number];

type Spec = typeof SPEC;
type SettingOf<S> = S extends { kind: "toggle" } ? ToggleSetting : S extends { kind: "choice"; options: readonly (infer T extends string)[] } ? ChoiceSetting<T> : TextSetting;

/** Every setting, as schema.json declares it, with a handheld device's own
 * default applied where one is given. */
export const SETTINGS = Object.fromEntries(
  Object.entries(SPEC).map(([key, spec]) => {
    const { handheld_default, ...setting } = spec as typeof spec & { handheld_default?: boolean };
    return [key, HANDHELD && handheld_default !== undefined ? { ...setting, default: handheld_default } : setting];
  }),
) as unknown as { [K in keyof Spec]: SettingOf<Spec[K]> };

export type SettingKey = keyof typeof SETTINGS;
export type SettingValues = { [K in SettingKey]: (typeof SETTINGS)[K]["default"] };
export type ToggleKey = { [K in SettingKey]: (typeof SETTINGS)[K] extends ToggleSetting ? K : never }[SettingKey];

export const SETTING_KEYS = Object.keys(SETTINGS) as SettingKey[];

/** The fallback layer: every setting at its declared default. */
export const DEFAULT_SETTINGS: Readonly<SettingValues> = Object.fromEntries(
  SETTING_KEYS.map((key) => [key, SETTINGS[key].default]),
) as SettingValues;

/** The settings shown under a section, in declaration order. */
export function settingsIn(section: SettingSection): SettingKey[] {
  return SETTING_KEYS.filter((key) => SETTINGS[key].section === section);
}

/** Whether `value` is something `key` may hold — what a stored override is
 * checked against before it is trusted. */
export function isValidSetting(key: SettingKey, value: unknown): value is SettingValues[SettingKey] {
  const setting: Setting = SETTINGS[key];
  switch (setting.kind) {
    case "toggle":
      return typeof value === "boolean";
    case "choice":
      return typeof value === "string" && setting.options.includes(value);
    case "text":
      return typeof value === "string";
  }
}
