// @pxl/client/port/fs.ts

/** xpute's file device, on the core's port: a refused file set aside, which
 * the core reads again. What it says is sys/fs's to decide (refusal.ts). */

import { fs_exports } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { ui } from "@/port/core.ts";

/** A refused file set aside, so the core reads it again (FS_CHANGED). */
export function fs_changed(f: RefusedFile): void {
  fs_exports(ui).fs_changed(f.root, f.path);
}
