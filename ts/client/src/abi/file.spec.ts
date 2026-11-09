// @pxl/client/abi/file.spec.ts
//
// GENERATED from spec/client/core/sys/io/file.json
//
// The core's files: the roots it opens them below, the names below each and what a
// file holds (pxl-core sys/io/file.rs, ts/client/src/service/sys/fs/).

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as root from "./file/root.spec.ts";
import * as write from "./file/write.spec.ts";
import * as top from "./file/top.spec.ts";
import * as storage from "./file/storage.spec.ts";
import * as format from "./file/format.spec.ts";

/** The roots the core opens files below (xpute's OPENAT). Below STORAGE a root is a
 * bake, numbered by its kind (pxl-spec bake/publication.spec.rs). Below every root a path is
 * one name: a tile's quadkey (pxl-spec space/layer/quadrant.rs), or what the root says. What it
 * is called and where past that, a tile's version and URL, a storage key, is what the client
 * (ts/client/src/service/sys/fs/) binds the root to, `@xpute/runtime` carrying the open there. */
export { root };

/** Below the asset root, the catalog the client plugs in (pxl-spec asset/catalog.rs): the
 * file each of the asset specs' references names. Any other id below it is a file,
 * its row in the asset ingest's table counted from 1 (spec/spec/asset/ingest.json). */
export const CATALOG: u32 = 0 as number;

/** Below OBJECT, the files that are no shard's; no quadkey has a letter. */
export const object = {
  /** The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
   * (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
   * it from the shards as an edit lands; none is no shard holding any. */
  TOP: "top",
  /** Edits, written, never read: a batch of writes in the order they were made (`write`),
   * its `format.OBJECT_EDITS` first, which the source applies to its shards. */
  EDITS: "edits",
} as const;

/** The zoom of the tile a shard below OBJECT is: pxl-engine's `SHARD_ZOOM`, which the
 * core holds this to (domain/layer/object/repository.rs). A key's first digits to it name
 * the shard, and each past it is a zoom's 0 to 7, on three axes (pxl-spec
 * space/hierarchy/morton.rs). */
export const SHARD_ZOOM: u32 = 8 as number;

/** The children of the tree packet a shard's file and an edit batch hold after their
 * format byte, by index: each row a write `<key, asset_id, ext>`, a leaf as the write that made
 * it in a shard's file, in the order made in a batch, where VOID erases (pxl-core
 * domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object/ext.rs),
 * its row and its fields as columns. */
export { write };

/** The children of the tree packet `object.TOP` holds after its format byte, by index:
 * each row a shard holding objects, which only its leaves are under. */
export { top };

/** The files the core keeps below STORAGE, by slot. What a slot holds is its
 * owner's, written whole, its slot's `format` first (pxl-core sys/io/file.rs). */
export { storage };

/** Each kept file's format, the first byte of what it holds: a storage slot's by its
 * name, a shard's file below OBJECT as OBJECT, the top and an edit batch as theirs. The
 * core reads a file of its format and refuses any other, reading none of it and never
 * writing over it, as a driver refuses a volume whose superblock it does not know; what
 * to do with the file is the client's. Bump one whenever a change makes an older file mean
 * something other than it meant when it was written. */
export { format };
