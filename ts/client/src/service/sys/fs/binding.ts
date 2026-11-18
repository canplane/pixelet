// @pxl/client/service/sys/fs/binding.ts

/** The binding behind every path the core opens or writes. A path is a tuple
 * of tokens walked down file.json5's `path` tree, as a URI's path is: each node
 * takes one token, its `ID`, and hands the rest of the tuple to the child it
 * names. The walk goes down the generated tree itself (`path`), so its shape
 * and ids are the spec's alone. A node with no child named takes the tokens
 * left as its data, such as a tile's quadkey or a load's id, and its handler
 * checks their types and count. A path that leads to no node, to a node with
 * no handler, or whose data its handler does not take, is no file. Only the
 * settings are written; the object edits go up as pxl:core object `apply`
 * (object.ts). The settings and the object files can be set aside. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { ComponentError } from "@xpute/link";
import {
  ERROR_CODE_NO_ENTRY,
  ERROR_CODE_READ_ONLY,
} from "@xpute/host/abi/fs/types.enum.ts";

import { LEAF_ZOOM, path, SHARD_ZOOM } from "@/abi/file.spec.ts";
import { BASE_KINDS, TILE_SPHERE } from "@/abi/publication.spec.ts";
import { readAsset } from "@/service/sys/fs/asset.ts";
import { OBJECTS, TOP_KEY } from "@/service/sys/fs/object.ts";
import { whenSourceDeclared } from "@/service/sys/fs/source.ts";
import {
  commitStorage,
  openStorage,
  setAside,
} from "@/service/sys/fs/storage.ts";
import { readObject } from "@/service/sys/fs/tile.ts";

/** A file's path: a tuple of tokens, as the URI the core opens it by. */
export type Path = readonly (number | string)[];

/** What the paths are bound to: a file's bytes to read, the bytes a written
 * file commits, and a file set aside at `nowMs`, out of its path. */
interface Binding {
  open(p: Path, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit(p: Path, bytes: U8Array): void | Promise<void>;
  setAside(p: Path, nowMs: number): void | Promise<void>;
}

/** A node of the generated tree: its own number under `ID`, and each child a
 * node beside it. The tree's top, `path`, is a node with no `ID`. */
type Node = { readonly ID?: number; readonly [child: string]: unknown };

/** What a node does, given the tokens left past it: the file it opens, the
 * file it writes, and the file it sets aside. */
interface Handler {
  open?(rest: Path, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(rest: Path, bytes: U8Array): void | Promise<void>;
  setAside?(rest: Path, nowMs: number): void | Promise<void>;
}

/** A tile's quadkey, at most a leaf's zoom long. */
const QUADKEY = new RegExp(`^[0-3]{1,${LEAF_ZOOM}}$`);
/** A shard's quadkey, at `SHARD_ZOOM`. */
const SHARD = new RegExp(`^[0-3]{${SHARD_ZOOM}}$`);

const none = (): never => {
  throw new ComponentError(ERROR_CODE_NO_ENTRY);
};

const readOnly = (): never => {
  throw new ComponentError(ERROR_CODE_READ_ONLY);
};

/** The one string left, where `form` matches it. */
function stringOf(rest: Path, form: RegExp): string | undefined {
  const [s] = rest;
  return rest.length === 1 && typeof s === "string" && form.test(s)
    ? s
    : undefined;
}

/** A base layer's tiles, each by its quadkey. */
function base(layer: (typeof BASE_KINDS)[number]): Handler {
  return {
    open(rest, signal) {
      const key = stringOf(rest, QUADKEY);
      if (key === undefined) return none();
      return whenSourceDeclared(signal).then((d) =>
        readObject(d, layer, key, signal),
      );
    },
  };
}

/** Each node's handler, beside the generated node it belongs to: each base
 * layer's at the node numbered as the layer is (file.json5 `BASE`). */
const HANDLERS: ReadonlyMap<Node, Handler> = new Map<Node, Handler>([
  ...BASE_KINDS.map((layer, n): [Node, Handler] => [
    walk([n])![0],
    base(layer),
  ]),
  [
    path.EARTH.SPHERE,
    {
      open: (rest, signal) =>
        rest.length === 0
          ? whenSourceDeclared(signal).then((d) =>
              readObject(d, BASE_KINDS[path.EARTH.ID], TILE_SPHERE, signal),
            )
          : none(),
    },
  ],
  [
    path.STORAGE.SETTINGS,
    {
      open: (rest) =>
        rest.length === 0 ? openStorage(path.STORAGE.SETTINGS.ID) : none(),
      commit: (rest, bytes) =>
        rest.length === 0
          ? commitStorage(path.STORAGE.SETTINGS.ID, bytes)
          : readOnly(),
      setAside: (rest, nowMs) =>
        rest.length === 0 ? setAside(path.STORAGE.SETTINGS.ID, nowMs) : none(),
    },
  ],
  [
    path.ASSET,
    {
      open(rest, signal) {
        const [id] = rest;
        return rest.length === 1 && typeof id === "number"
          ? readAsset(id, signal)
          : none();
      },
    },
  ],
  [
    path.OBJECT,
    {
      open: (rest) => OBJECTS.open(stringOf(rest, SHARD) ?? none()),
      setAside: (rest, nowMs) =>
        OBJECTS.setAside(stringOf(rest, SHARD) ?? none(), nowMs),
    },
  ],
  [
    path.OBJECT.TOP,
    {
      open: (rest) => (rest.length === 0 ? OBJECTS.open(TOP_KEY) : none()),
      setAside: (rest, nowMs) =>
        rest.length === 0 ? OBJECTS.setAside(TOP_KEY, nowMs) : none(),
    },
  ],
]);

/** The node `p` leads to down from `at`, and the tokens left past it: each
 * node takes one token, and where the next names a child, the walk goes on in
 * it. */
export function walk(p: Path, at: Node = path): [Node, Path] | undefined {
  const [id, ...rest] = p;
  const node = Object.values(at).find(
    (n): n is Node =>
      typeof n === "object" && n !== null && (n as Node).ID === id,
  );
  if (node === undefined) return undefined;
  return (rest.length > 0 && walk(rest, node)) || [node, rest];
}

/** The handler of the node `p` leads to, and the tokens left past it. */
function handlerOf(p: Path): [Handler | undefined, Path] {
  const [node, rest] = walk(p) ?? [undefined, []];
  return [node && HANDLERS.get(node), rest];
}

/** The paths bound to the handlers mounted on them, each call going to the
 * handler of its path's node. */
export const BINDING: Binding = {
  open(p, signal) {
    const [h, rest] = handlerOf(p);
    if (h?.open === undefined) return none();
    return h.open(rest, signal);
  },
  commit(p, bytes) {
    const [h, rest] = handlerOf(p);
    if (h?.commit === undefined) return readOnly();
    return h.commit(rest, bytes);
  },
  setAside(p, nowMs) {
    const [h, rest] = handlerOf(p);
    if (h?.setAside === undefined) return readOnly();
    return h.setAside(rest, nowMs);
  },
};
