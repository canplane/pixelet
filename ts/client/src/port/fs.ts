// @pxl/client/port/fs.ts

/** FS on the shell's port: the tile source's part the kernel is handed, and a
 * refused file it may read again. What they say is sys/fs's to decide
 * (source.ts, refusal.ts). */

import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { kernel } from "@/host/kernel.ts";

/** The top zoom, the formats and the shards (FS_MOUNT); false where they are
 * more than a command carries or the kernel refuses them. */
export function fs_mount(
  top_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  return kernel.fs_mount(top_zoom, formats, shards) >= 0;
}

/** A refused file set aside, so the kernel may read it again (FS_TRUNCATED). */
export function fs_truncated(f: RefusedFile): void {
  kernel.fs_truncated(f.root, f.path);
}
