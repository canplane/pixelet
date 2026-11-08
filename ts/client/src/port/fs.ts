// @pxl/client/port/fs.ts

/** xpute's file device, on the core's port. It tells the core that a refused
 * file was set aside, so that the core reads it again. Which file, and when,
 * is sys/fs's to decide (refusal.ts). */

import { fs_exports } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { ui } from "@/port/core.ts";

export function fs_changed(f: RefusedFile): void {
  fs_exports(ui).fs_changed(f.root, f.path);
}
