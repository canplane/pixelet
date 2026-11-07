// @pxl/client/port/setting.ts

/** The setting's command: one value set from the sheet or a picker. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { core } from "@/port/core.ts";

export function set(key: u32, value: u32): void {
  core.setting_set(key, value);
}
