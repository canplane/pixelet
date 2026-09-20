// @pxl/web/feature/setting/schema.ts

/**
 * Every setting the app has, declared once: its default, its kind, and
 * where the settings sheet shows it. The kernel's store
 * (crates/pxl/web/kernel/src/shell/setting/store.rs) reads the same table
 * from schema.json's Rust side and the sheet (section.tsx) renders rows
 * from here, so adding a setting is one entry in schema.json plus its two
 * strings in the locale catalogue under `settings.<section>.<key>`.
 *
 * The values are the fallback layer of a two-layer model, the way an
 * editor keeps a default settings file under the user's own: what the app
 * reads is the defaults with the user's overrides on top, and only the
 * overrides are stored. See the kernel's store.rs for why that matters.
 *
 * A `dev` setting is in the kernel's table in every build — offered only
 * under a development one — but is absent from the shell's production
 * bundle: the table below is SETTINGS_SHIPPED there, which has no such
 * row, and its strings live in the dev locale catalogue, so the artifact
 * carries neither the row nor its name (apps/web/script/check-bundle.ts
 * reads the artifact to be sure).
 */

import { HANDHELD } from "@/shared/lib/browser/handheld.ts";

import { CAMERA_TERRAIN_MODES as MODES, DEV_SECTIONS, SETTINGS as EVERY, SETTINGS_SHIPPED as SHIPPED } from "./schema.spec.ts";

type Spec = typeof EVERY;

/** The rows this build offers. Two literals and a `define`d choice rather
 * than a filter: the bundler folds `__IS_DEV__` and drops the literal a
 * production build never reads, so a dev row is absent from the artifact
 * and not merely hidden — a filter would leave every name in it. The type
 * stays the whole table's: a dev key is a `SettingKey` in every build, and
 * what a production build lacks is its row. */
const SPEC = (__IS_DEV__ ? EVERY : SHIPPED) as Spec;

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
export type ChoiceSetting<T extends string = string> = Common & { kind: "choice"; default: T; options: readonly T[]; costs?: readonly (SettingCost | "")[] };
export type TextSetting = Common & { kind: "text"; default: string };
export type Setting = ToggleSetting | ChoiceSetting | TextSetting;

/** What terrain does to the camera (camera/terrain_gate.ts); schema.json says each. */
export const CAMERA_TERRAIN_MODES = MODES;
export type CameraTerrainMode = (typeof CAMERA_TERRAIN_MODES)[number];

type SettingOf<S> = S extends { kind: "toggle" } ? ToggleSetting : S extends { kind: "choice"; options: readonly (infer T extends string)[] } ? ChoiceSetting<T> : TextSetting;

/** Every setting this build offers, as schema.json declares it, with a
 * handheld device's own default applied where one is given. */
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

/** Whether a section is a development build's as a whole (schema.json's
 * DEV_SECTIONS): shown under `__IS_DEV__` only, badged on its heading rather
 * than on each row. */
export function isDevSection(section: SettingSection): boolean {
  return (DEV_SECTIONS as readonly string[]).includes(section);
}

/** The settings shown under a section, in declaration order. */
export function settingsIn(section: SettingSection): SettingKey[] {
  return SETTING_KEYS.filter((key) => SETTINGS[key].section === section);
}

/** Whether `value` is something `key` may hold — what a stored override is
 * checked against before it is trusted. A key this build has no row for
 * holds nothing. */
export function isValidSetting(key: SettingKey, value: unknown): value is SettingValues[SettingKey] {
  const setting: Setting | undefined = SETTINGS[key];
  if (setting === undefined) return false;
  switch (setting.kind) {
    case "toggle":
      return typeof value === "boolean";
    case "choice":
      return typeof value === "string" && setting.options.includes(value);
    case "text":
      return typeof value === "string";
  }
}
