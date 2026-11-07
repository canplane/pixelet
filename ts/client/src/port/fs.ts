// @pxl/client/port/fs.ts

/** xpute's file device, on the kernel's port: a refused file set aside, which
 * the kernel reads again. What it says is sys/fs's to decide (refusal.ts). */

import { fs_exports } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { ui } from "@/runtime/kernel.ts";

/** A refused file set aside, so the kernel reads it again (FS_CHANGED). */
export function fs_changed(f: RefusedFile): void {
  fs_exports(ui).fs_changed(f.root, f.path);
}
