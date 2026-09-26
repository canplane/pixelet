// @pxl/web/feature/setting/schema.ts

/**
 * Every setting the app has, as the sheet draws it: its kind, where it is
 * shown, what it costs, and the number the kernel knows it by (`key`). The
 * kernel's store (crates/pxl/web/kernel/src/shell/setting/store.rs) holds the
 * values and sends them in its snapshot, a setting's value at its `key`; the
 * sheet (section.tsx) renders rows from here, so adding a setting is one
 * entry in schema.json plus its two strings in the locale catalog under
 * `settings.<section>.<name>`.
 *
 * A `dev` setting is offered only under a development build, and is absent
 * from the shell's production bundle: the table below is SETTINGS_SHIPPED
 * there, which has no such row, and its strings live in the dev locale
 * catalog, so the artifact carries neither the row nor its name
 * (apps/web/script/check-bundle.ts reads the artifact to be sure).
 */

import { SETTINGS as EVERY, SETTINGS_SHIPPED as SHIPPED } from "./schema.spec.ts";

type Spec = typeof EVERY;

/** The rows this build offers. Two literals and a `define`d choice rather
 * than a filter: the bundler folds `__IS_DEV__` and drops the literal a
 * production build never reads, so a dev row is absent from the artifact
 * and not merely hidden — a filter would leave every name in it. The type
 * stays the whole table's: a dev setting is a `SettingName` in every build,
 * and what a production build lacks is its row. */
export const SETTINGS = (__IS_DEV__ ? EVERY : SHIPPED) as Spec;

/** Rough per-frame cost, shown as a badge on the row. */
export type SettingCost = "low" | "med" | "high";

/** The sheet's sections, in the order they are shown (schema.json). */
export type SettingSection = "display" | "camera" | "editing" | "lighting" | "quality" | "debug" | "character";

export type SettingName = keyof Spec;

/** A row as the sheet reads one, whichever setting it is. */
export type Setting =
  & { key: number; section: SettingSection; cost?: SettingCost; dev?: boolean }
  & ({ kind: "toggle" } | { kind: "choice"; options: readonly string[]; costs?: readonly (SettingCost | "")[] } | { kind: "number" });

const SETTING_NAMES = Object.keys(SETTINGS) as SettingName[];

/** The settings shown under a section, in declaration order. */
export function settingsIn(section: SettingSection): SettingName[] {
  return SETTING_NAMES.filter((name) => SETTINGS[name].section === section);
}
