// @pxl/client/sys/fs/namespace.ts

/**
 * The kernel's namespace: each root it opens a file under (file.json `root`)
 * bound to what fetches or keeps its bytes, a path below it the file's one
 * name. The boot hands it to `@xpute/host`'s system calls (abi/boot.ts).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { type Binding, Namespace } from "@xpute/host";

import { root } from "@/abi/file.spec.ts";
import { BASE_LAYERS } from "@/origin/publication.spec.ts";
import { read_asset } from "@/sys/fs/asset.ts";
import { OBJECTS } from "@/sys/fs/object.ts";
import { source_declaration } from "@/sys/fs/source.ts";
import { commit_storage, open_storage } from "@/sys/fs/storage.ts";
import { read_object } from "@/sys/fs/tile.ts";

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
    [root.ASSET, { open: read_asset }],
    [
      root.STORAGE,
      {
        open: (path) => open_storage(path),
        commit: (path, bytes) => commit_storage(path, bytes),
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
