// @pxl/client/service/core/fs.ts

/** The core reads a refused file again once it hears here that the file was
 * set aside. Which file, and when, is sys/fs's to decide (refusal.ts). */

import { fs_exports } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { ui } from "@/service/core/core.ts";

export function fs_changed(f: RefusedFile): void {
  fs_exports(ui).fs_changed(f.root, f.path);
}
