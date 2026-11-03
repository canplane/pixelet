// @pxl/client/driver/namespace/refusal.ts

/**
 * The files the kernel refused (the snapshot's `refused_files`): each set aside
 * once by the backend its root is bound to, a storage file's slot emptied
 * (storage.ts) or a placed objects' file taken from its source
 * (object.ts), and the kernel told it may read it again (FS_TRUNCATED).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

import {
  Command,
  encode_fs_truncated,
  type RefusedFile,
} from "@/abi/cmd.spec.ts";
import { root } from "@/abi/host.spec.ts";
import * as port from "@/abi/port.spec.ts";
import { HOST } from "@/host/mount.ts";
import { OBJECTS } from "@/driver/namespace/object.ts";
import {
  STORAGE,
  set_aside,
  type Storage,
} from "@/driver/namespace/storage.ts";

/** A file set aside by the backend `root` is bound to, by its number; false,
 * nothing changed, where it did not. */
export type SetAside = (
  root: u32,
  number: bigint,
  now_ms: number,
) => boolean | Promise<boolean>;

/** Each root's backend's own set-aside, STORAGE's over `s`. */
export function set_aside_by_root(s: Storage): SetAside {
  return (r, number, now_ms) => {
    switch (r) {
      case root.STORAGE:
        return number <= 0xffff_ffffn && set_aside(s, Number(number), now_ms);
      case root.OBJECT:
        return OBJECTS.set_aside(number, now_ms);
      default:
        return false;
    }
  };
}

/** The files set aside and not yet seen read again, as `<root>/<path>`, so
 * a snapshot that still names one before the kernel hears it sets nothing
 * aside twice. */
const emptying = new Set<string>();

/** The kernel told a file is empty (FS_TRUNCATED), after the completion
 * ring is read: the doorbell refuses a write while it reads. */
function file_emptied(f: RefusedFile): void {
  queueMicrotask(() =>
    HOST.send(
      port.SHELL,
      Command.FS_TRUNCATED,
      encode_fs_truncated(HOST.doorbell, f.root, f.path),
    ),
  );
}

/** The snapshot's `refused_files`: each file set aside once, and the kernel told
 * (`emptied`). */
export function follow_refused_files(
  files: readonly RefusedFile[],
  emptied: (f: RefusedFile) => void = file_emptied,
  aside: SetAside = set_aside_by_root(STORAGE),
): void {
  const named = new Set(files.map((f) => `${f.root}/${f.path}`));
  for (const f of emptying) {
    if (!named.has(f)) emptying.delete(f);
  }
  for (const f of files) {
    const at = `${f.root}/${f.path}`;
    if (emptying.has(at) || !/^\d{1,20}$/.test(f.path)) continue;
    emptying.add(at);
    void Promise.resolve(aside(f.root, BigInt(f.path), Date.now())).then(
      (set) => {
        if (set) emptied(f);
        else emptying.delete(at);
      },
    );
  }
}
