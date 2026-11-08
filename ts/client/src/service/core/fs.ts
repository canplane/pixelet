// @pxl/client/service/core/fs.ts

/** The core reads a refused file again once it hears here that the file was
 * set aside. Which file, and when, is sys/fs's to decide (refusal.ts). */

import { FsCommand, type FsProtocol } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { ui } from "@/service/core/core.ts";

export function fs_changed(f: RefusedFile): void {
  const fs: FsProtocol = ui;
  fs.send(FsCommand.FS_CHANGED, f.root, f.path);
}
