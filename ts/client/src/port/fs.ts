// @pxl/client/port/fs.ts

/** FS on the shell's port: the tile source's part the kernel is handed, and a
 * refused file it may read again. What they say is sys/fs's to decide
 * (source.ts, refusal.ts). */

import type { RefusedFile } from "@/abi/cmd.spec.ts";
import {
  Command,
  encode_fs_mount,
  encode_fs_truncated,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** The top zoom, the formats and the shards (FS_MOUNT); false where they are
 * more than a command carries or the kernel refuses them. */
export function fs_mount(
  top_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  const pkt = encode_fs_mount(HOST.doorbell, top_zoom, formats, shards);
  if (!HOST.doorbell.fits(pkt)) return false;
  return HOST.call(port.SHELL, Command.FS_MOUNT, pkt, (result) => result >= 0);
}

/** A refused file set aside, so the kernel may read it again (FS_TRUNCATED),
 * sent after the completion ring is read: the doorbell refuses a write while
 * it reads. */
export function fs_truncated(f: RefusedFile): void {
  queueMicrotask(() =>
    HOST.send(
      port.SHELL,
      Command.FS_TRUNCATED,
      encode_fs_truncated(HOST.doorbell, f.root, f.path),
    ),
  );
}
