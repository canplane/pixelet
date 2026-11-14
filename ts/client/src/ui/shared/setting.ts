// @pxl/client/ui/shared/setting.ts

/** A new setting takes an entry in schema.json and its strings under
 * `settings.<section>.<name>`. */

import { SETTING as EVERY, SETTING_PROD as PROD } from "@/abi/schema.spec.ts";

type Spec = typeof EVERY;

/** The table is chosen by a constant that the build folds, rather than
 * filtered, so that the dev rows are absent from a production bundle instead
 * of hidden. */
export const SETTING = (__DEV__ ? EVERY : PROD) as Spec;

export type SettingName = keyof Spec;

export type Setting = Spec[SettingName];

export type SettingSection = Setting["section"];

/** The costs a setting can carry, whole or by option: any setting's `cost`,
 * and any entry of an option's `costs` except "", which marks a free option. */
export type SettingCost =
  | Extract<Setting, { cost: unknown }>["cost"]
  | Exclude<Extract<Setting, { costs: unknown }>["costs"][number], "">;

const SETTING_NAMES = Object.keys(SETTING) as SettingName[];

export function settings_in(section: SettingSection): SettingName[] {
  return SETTING_NAMES.filter((name) => SETTING[name].section === section);
}
