// @pxl/client/service/core/setting.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as setting from "@/abi/setting.ts";
import { core } from "@/service/core/core.ts";

export function set(key: u32, val: u32): void {
  void setting.set(core.setting, key, val);
}
