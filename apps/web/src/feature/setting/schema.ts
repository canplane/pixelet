// @pxl/web/feature/setting/schema.ts

/**
 * Every setting as the sheet draws it, keyed by the kernel's number. Adding
 * one is a schema.json entry plus its strings under `settings.<section>.<name>`.
 */

import { SETTINGS as EVERY, SETTINGS_SHIPPED as SHIPPED } from "./schema.spec.ts";

type Spec = typeof EVERY;

/** A constant-folded choice, not a filter, so dev rows are absent from the
 * production artifact rather than hidden. */
export const SETTINGS = (__IS_DEV__ ? EVERY : SHIPPED) as Spec;

export type SettingCost = "low" | "med" | "high";

export type SettingSection = "display" | "navigation" | "camera" | "lighting" | "debug" | "character" | "map";

export type SettingName = keyof Spec;

export type Setting =
  & { key: number; section: SettingSection; cost?: SettingCost; dev?: boolean }
  & ({ kind: "toggle" } | { kind: "choice"; options: readonly string[]; costs?: readonly (SettingCost | "")[] } | { kind: "index" });

const SETTING_NAMES = Object.keys(SETTINGS) as SettingName[];

export function settingsIn(section: SettingSection): SettingName[] {
  return SETTING_NAMES.filter((name) => SETTINGS[name].section === section);
}
