// @pxl/client/service/sys/fs/refusal.ts

/**
 * Follows the files the core refused, which the snapshot lists as
 * `refused_files`. Each is set aside once by the backend its path names
 * (binding.ts): a storage file's slot is emptied (storage.ts), or
 * a file of placed objects is taken out of its source (object.ts). Then the
 * core is told that it may read the file again (FS_CHANGED).
 */

import type { Path } from "@xpute/runtime";

import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { path } from "@/abi/file.spec.ts";
import { object_key, walk } from "@/service/sys/fs/binding.ts";
import { OBJECTS } from "@/service/sys/fs/object.ts";
import { STORAGE, set_aside, type Storage } from "@/service/sys/fs/storage.ts";

/** A set-aside goes through the backend that `path` names, and answers
 * false, with nothing changed, when the backend could not make it. */
export type SetAside = (
  path: Path,
  now_ms: number,
) => boolean | Promise<boolean>;

export function set_aside_by_path(s: Storage): SetAside {
  return (p, now_ms) => {
    const [node, rest] = walk(p) ?? [];
    if (node === path.STORAGE.SETTINGS && rest!.length === 0) {
      return set_aside(s, path.STORAGE.SETTINGS.ID, now_ms);
    }
    const key = object_key(p);
    return key !== undefined && OBJECTS.set_aside(key, now_ms);
  };
}

/** The files set aside that the snapshots may still name, by their paths.
 * A snapshot keeps naming a file until the core hears that it changed, and
 * this set keeps such a file from being set aside twice. */
const emptying = new Set<string>();

export function follow_refused_files(
  files: readonly RefusedFile[],
  emptied: (f: RefusedFile) => void,
  aside: SetAside = set_aside_by_path(STORAGE),
): void {
  const named = new Set(files.map((f) => JSON.stringify(f.path)));
  for (const f of emptying) {
    if (!named.has(f)) emptying.delete(f);
  }
  for (const f of files) {
    const at = JSON.stringify(f.path);
    if (emptying.has(at)) continue;
    emptying.add(at);
    void Promise.resolve(aside(f.path, Date.now())).then((set) => {
      if (set) emptied(f);
      else emptying.delete(at);
    });
  }
}
