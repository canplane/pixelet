// @pxl/client/service/core/raw.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Arg } from "@xpute/kit/wire/xtp/mod.ts";
import { ui } from "@/service/core/core.ts";

/** The console calls this to send any command by its number, past
 * the command table, with its arguments as the fields of its packet. */
export const raw = {
  cmd: (cmd: u32, ...args: Arg[]): void => ui.send(cmd, ...args),
};
