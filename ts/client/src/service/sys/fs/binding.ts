// @pxl/client/service/sys/fs/binding.ts

/** The binding behind every path the core opens or writes. A path is a tuple
 * of tokens walked down file.json's `path` tree, as a URI's path is: each node
 * takes one token, its `ID`, and hands the rest of the tuple to the child it
 * names. The walk goes down the generated tree itself (`path`), so its shape
 * and ids are the spec's alone. A node with no child named takes the tokens
 * left as its data, such as a tile's quadkey or a load's id, and its handler
 * checks their types and count. A path that leads to no node, to a node with
 * no handler, or whose data its handler does not take, is no file. Only the
 * settings are written; the objects' edits go up as pxl:core object `apply`
 * (object.ts). The settings and the objects' files can be set aside. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { ComponentError } from "@xpute/link";
import {
  ERROR_CODE_NO_ENTRY,
  ERROR_CODE_READ_ONLY,
} from "@xpute/link/abi/fs/types.enum.ts";

import { LEAF_ZOOM, path, SHARD_ZOOM } from "@/abi/file.spec.ts";
import {
  BASE_LAYERS,
  TILE_SPHERE,
} from "@/service/origin/publication.spec.ts";
import { read_asset } from "@/service/sys/fs/asset.ts";
import { OBJECTS, TOP_KEY } from "@/service/sys/fs/object.ts";
import { source_decl } from "@/service/sys/fs/source.ts";
import {
  commit_storage,
  open_storage,
  set_aside,
  STORAGE,
} from "@/service/sys/fs/storage.ts";
import { read_object } from "@/service/sys/fs/tile.ts";

/** A file's path: a tuple of tokens, as the URI the core opens it by. */
export type Path = readonly (number | string)[];

/** What the paths are bound to: a file's bytes to read, the bytes a written
 * file commits, and a file set aside at `now_ms`, out of its path. */
export interface Binding {
  open(p: Path, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit(p: Path, bytes: U8Array): void | Promise<void>;
  set_aside(p: Path, now_ms: number): void | Promise<void>;
}

/** A node of the generated tree: its own number under `ID`, and each child a
 * node beside it. The tree's top, `path`, is a node with no `ID`. */
type Node = { readonly ID?: number; readonly [child: string]: unknown };

/** What a node does, given the tokens left past it: the file it opens, the
 * file it writes, and the file it sets aside. */
interface Handler {
  open?(rest: Path, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(rest: Path, bytes: U8Array): void | Promise<void>;
  set_aside?(rest: Path, now_ms: number): void | Promise<void>;
}

/** A tile's quadkey, at most a leaf's zoom long. */
const QUADKEY = new RegExp(`^[0-3]{1,${LEAF_ZOOM}}$`);
/** A shard's quadkey, at `SHARD_ZOOM`. */
const SHARD = new RegExp(`^[0-3]{${SHARD_ZOOM}}$`);

const none = (): never => {
  throw new ComponentError(ERROR_CODE_NO_ENTRY);
};

const read_only = (): never => {
  throw new ComponentError(ERROR_CODE_READ_ONLY);
};

/** The one string left, where `form` matches it. */
function string_of(rest: Path, form: RegExp): string | undefined {
  const [s] = rest;
  return rest.length === 1 && typeof s === "string" && form.test(s)
    ? s
    : undefined;
}

/** The base layers' tiles, each by its layer's number and its quadkey. */
const LAYER: Handler = {
  open([n, ...rest], signal) {
    const layer = typeof n === "number" ? BASE_LAYERS[n] : undefined;
    const key = string_of(rest, QUADKEY);
    if (layer === undefined || key === undefined) return none();
    return read_object(source_decl()!, layer, key, signal);
  },
};

/** Each node's handler, beside the generated node it belongs to. */
const HANDLERS: ReadonlyMap<Node, Handler> = new Map<Node, Handler>([
  [path.LAYER, LAYER],
  [
    path.SPHERE,
    {
      open: (rest, signal) =>
        rest.length === 0
          ? read_object(
              source_decl()!,
              BASE_LAYERS[path.SPHERE.LAYER],
              TILE_SPHERE,
              signal,
            )
          : none(),
    },
  ],
  [
    path.STORAGE.SETTINGS,
    {
      open: (rest) =>
        rest.length === 0 ? open_storage(path.STORAGE.SETTINGS.ID) : none(),
      commit: (rest, bytes) =>
        rest.length === 0
          ? commit_storage(path.STORAGE.SETTINGS.ID, bytes)
          : read_only(),
      set_aside: (rest, now_ms) =>
        rest.length === 0
          ? set_aside(STORAGE, path.STORAGE.SETTINGS.ID, now_ms)
          : none(),
    },
  ],
  [
    path.ASSET,
    {
      open(rest, signal) {
        const [id] = rest;
        return rest.length === 1 && typeof id === "number"
          ? read_asset(id, signal)
          : none();
      },
    },
  ],
  [
    path.OBJECT,
    {
      open: (rest) => OBJECTS.open(string_of(rest, SHARD) ?? none()),
      set_aside: (rest, now_ms) =>
        OBJECTS.set_aside(string_of(rest, SHARD) ?? none(), now_ms),
    },
  ],
  [
    path.OBJECT.TOP,
    {
      open: (rest) => (rest.length === 0 ? OBJECTS.open(TOP_KEY) : none()),
      set_aside: (rest, now_ms) =>
        rest.length === 0 ? OBJECTS.set_aside(TOP_KEY, now_ms) : none(),
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
function handler_of(p: Path): [Handler | undefined, Path] {
  const [node, rest] = walk(p) ?? [undefined, []];
  return [node && HANDLERS.get(node), rest];
}

export function binding(): Binding {
  return {
    open(p, signal) {
      const [h, rest] = handler_of(p);
      if (h?.open === undefined) return none();
      return h.open(rest, signal);
    },
    commit(p, bytes) {
      const [h, rest] = handler_of(p);
      if (h?.commit === undefined) return read_only();
      return h.commit(rest, bytes);
    },
    set_aside(p, now_ms) {
      const [h, rest] = handler_of(p);
      if (h?.set_aside === undefined) return read_only();
      return h.set_aside(rest, now_ms);
    },
  };
}
