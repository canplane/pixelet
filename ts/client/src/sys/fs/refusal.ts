// @pxl/client/sys/fs/refusal.ts

/**
 * The files the core refused (the snapshot's `refused_files`): each set aside
 * once by the backend its root is bound to, a storage file's slot emptied
 * (storage.ts) or a placed objects' file taken from its source
 * (object.ts), and the core told it may read it again (FS_CHANGED).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { root } from "@/abi/file.spec.ts";
import { OBJECTS } from "@/sys/fs/object.ts";
import { STORAGE, set_aside, type Storage } from "@/sys/fs/storage.ts";

/** A file set aside by the backend `root` is bound to, by its path; false,
 * nothing changed, where it did not. */
export type SetAside = (
  root: u32,
  path: string,
  now_ms: number,
) => boolean | Promise<boolean>;

/** Each root's backend's own set-aside, STORAGE's over `s`. */
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

/** The files set aside and not yet seen read again, as `<root>/<path>`, so
 * a snapshot that still names one before the core hears it sets nothing
 * aside twice. */
const emptying = new Set<string>();

/** The snapshot's `refused_files`: each file set aside once, and the core told
 * (`emptied`). */
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
