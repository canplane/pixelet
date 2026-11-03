// @pxl/client/driver/namespace/namespace.ts

/**
 * The kernel's namespace: each root it opens a file under (file.json `root`)
 * bound to what fetches or keeps its bytes, a path below it the file's one
 * name. The boot hands it to `@xpute/host`'s system calls (abi/boot.ts).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { type Binding, Namespace } from "@xpute/host/ipc/ns.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root } from "@/abi/file.spec.ts";
import { BASE_LAYERS } from "./publication.spec.ts";
import { read_asset } from "@/driver/namespace/asset.ts";
import { OBJECTS } from "@/driver/namespace/object.ts";
import { source_declaration } from "@/driver/namespace/source.ts";
import { commit_storage, open_storage } from "@/driver/namespace/storage.ts";
import { read_object } from "@/driver/namespace/tile.ts";

/** A load's or a slot's id in decimal, a path below ASSET or STORAGE; ENOENT
 * for any other. */
function id_of(path: string): bigint {
  if (!/^\d{1,20}$/.test(path)) throw new FileError(Errno.ENOENT);
  return BigInt(path);
}

/** Each bake to its tiles at the edge, ASSET to the shipped assets, STORAGE
 * to the platform's storage, OBJECT to the placed objects' source. */
export function namespace(): Namespace {
  const bound: [u32, Binding][] = [
    ...BASE_LAYERS.map((_, bake): [u32, Binding] => [
      bake,
      {
        open: (name, signal) =>
          read_object(source_declaration()!, bake, name, signal),
      },
    ]),
    [root.ASSET, { open: (path, signal) => read_asset(id_of(path), signal) }],
    [
      root.STORAGE,
      {
        open: (path) => open_storage(id_of(path)),
        commit: (path, bytes) => commit_storage(id_of(path), bytes),
      },
    ],
    [
      root.OBJECT,
      {
        open: (path) => OBJECTS.open(path),
        commit: (path, bytes) => OBJECTS.commit(path, bytes),
      },
    ],
  ];
  return new Namespace(bound);
}
