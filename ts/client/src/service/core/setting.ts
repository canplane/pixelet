// @pxl/client/service/core/setting.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Command } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

export function set(key: u32, val: u32): void {
  core.send(Command.SETTING_SET, key, val);
}
