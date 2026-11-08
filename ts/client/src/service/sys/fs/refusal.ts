// @pxl/client/service/sys/fs/refusal.ts

/**
 * Follows the files the core refused, which the snapshot lists as
 * `refused_files`. Each is set aside once by the backend its root is bound to:
 * a storage file's slot is emptied (storage.ts), or a file of placed objects
 * is taken out of its source (object.ts). Then the core is told that it may
 * read the file again (FS_CHANGED).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { root } from "@/abi/file.spec.ts";
import { OBJECTS } from "@/service/sys/fs/object.ts";
import { STORAGE, set_aside, type Storage } from "@/service/sys/fs/storage.ts";

/** Sets aside the file at `path` through the backend that `root` is bound
 * to. It answers false, with nothing changed, when the backend could not. */
export type SetAside = (
  root: u32,
  path: string,
  now_ms: number,
) => boolean | Promise<boolean>;

/** Sends each root to its backend's set-aside. STORAGE's acts on `s`. */
export function set_aside_by_root(s: Storage): SetAside {
  return (r, path, now_ms) => {
    switch (r) {
      case root.STORAGE:
        return set_aside(s, path, now_ms);
      case root.OBJECT:
        return OBJECTS.set_aside(path, now_ms);
      default:
        return false;
    }
  };
}

/** The files set aside that the snapshots may still name, as `<root>/<path>`.
 * A snapshot keeps naming a file until the core hears that it changed, and
 * this set keeps such a file from being set aside twice. */
const emptying = new Set<string>();

/** Takes a snapshot's `refused_files`, sets each file aside once, and tells
 * the core through `emptied`. */
export function follow_refused_files(
  files: readonly RefusedFile[],
  emptied: (f: RefusedFile) => void,
  aside: SetAside = set_aside_by_root(STORAGE),
): void {
  const named = new Set(files.map((f) => `${f.root}/${f.path}`));
  for (const f of emptying) {
    if (!named.has(f)) emptying.delete(f);
  }
  for (const f of files) {
    const at = `${f.root}/${f.path}`;
    if (emptying.has(at)) continue;
    emptying.add(at);
    void Promise.resolve(aside(f.root, f.path, Date.now())).then((set) => {
      if (set) emptied(f);
      else emptying.delete(at);
    });
  }
}
