// @pxl/client/service/core/setting.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ComponentError } from "@xpute/link";
import * as setting from "@/abi/setting.ts";
import { ui } from "@/service/core/core.ts";

/** Tells the core whether this is a development build, once its interfaces
 * are open: a development build's settings are offered only there. */
export function tell_dev(): Promise<void> {
  return setting.dev(ui, __DEV__ || __PERF_DRIVE__);
}

/** Sets the setting `key`; one set before the core has read its settings is
 * refused, and the read's value stands. */
export function set(key: u32, val: u32): void {
  void setting.set(ui, key, val).catch((e: unknown) => {
    if (!(e instanceof ComponentError)) throw e;
    console.warn(`[setting] ${key} not set: ${e.payload}`);
  });
}
