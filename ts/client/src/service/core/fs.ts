// @pxl/client/service/core/fs.ts

/** The core reads a refused file again once it hears here that the file was
 * set aside. Which file, and when, is sys/fs's to decide (refusal.ts). */

import { Branch } from "@xpute/kit/wire/xtp/mod.ts";
import { fs_exports } from "@xpute/runtime/abi/fs.spec.ts";
import type { RefusedFile } from "@/abi/ui.ts";
import { ui } from "@/service/core/core.ts";

export function fs_changed(f: RefusedFile): void {
  fs_exports(ui).fs_changed(new Branch(f.path));
}
