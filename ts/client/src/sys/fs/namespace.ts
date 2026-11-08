// @pxl/client/sys/fs/namespace.ts

/**
 * The core's namespace. Each root the core opens files under (file.json
 * `root`) is bound to the backend that fetches or keeps their bytes, and a
 * file's path below its root is its one name. The boot hands the namespace to
 * the system calls of `@xpute/runtime` (runtime/start.ts).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { type Binding, Namespace } from "@xpute/runtime";

import { root } from "@/abi/file.spec.ts";
import { BASE_LAYERS } from "@/origin/publication.spec.ts";
import { read_asset } from "@/sys/fs/asset.ts";
import { OBJECTS } from "@/sys/fs/object.ts";
import { source_declaration } from "@/sys/fs/source.ts";
import { commit_storage, open_storage } from "@/sys/fs/storage.ts";
import { read_object } from "@/sys/fs/tile.ts";

export function namespace(): Namespace {
  const bindings: [u32, Binding][] = [
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
  return new Namespace(bindings);
}
