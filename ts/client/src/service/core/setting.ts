// @pxl/client/service/core/setting.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as setting from "@/abi/setting.ts";
import { core } from "@/service/core/core.ts";

/** Tells the core whether this is a development build, once its interfaces
 * are open: a development build's settings are offered only there. */
export function tell_dev(): Promise<void> {
  return setting.dev(core.setting, __IS_DEV__ || __PERF_DRIVE__);
}

export function set(key: u32, val: u32): void {
  void setting.set(core.setting, key, val);
}
