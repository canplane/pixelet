// @pxl/client/service/core/raw.ts

import type { Arg } from "@xpute/kit/wire/xtp/mod.ts";
import type { Handle, Method } from "@xpute/runtime";
import { ui } from "@/service/core/core.ts";

/** The console calls this to send any method of any handle, past the
 * interfaces, with its arguments as the fields of its packet. */
export const raw = {
  cmd: (h: Handle, m: Method, ...args: Arg[]): void => void ui.send(h, m, args),
};
