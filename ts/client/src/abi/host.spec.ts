// @pxl/client/abi/host.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// The kernel's own hostcalls, numbered beside xpute's (xpute/spec/ipc/sys.json,
// pxl-kernel sys/io/host.rs, ts/client/src/driver/text.ts) and made as hostcalls, answered
// before they return; and the range of the stream xpute's calls are recorded in, which
// a hostcall's record lies past while it runs.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as stream from "./host/stream.spec.ts";
import * as op from "./host/op.spec.ts";
import * as LAYOUT from "./host/layout.spec.ts";
import * as LAYOUT_ANSWER from "./host/layout_answer.spec.ts";
import * as LINE from "./host/line.spec.ts";
import * as RASTER from "./host/raster.spec.ts";
import * as root from "./host/root.spec.ts";
import * as object from "./host/object.spec.ts";
import * as write from "./host/write.spec.ts";
import * as top from "./host/top.spec.ts";
import * as storage from "./host/storage.spec.ts";
import * as format from "./host/format.spec.ts";

/** The stream's range, laid out by the kernel's map. */
export { stream };

/** The kernel's calls, each with xpute's PROGRAM bit and made as a hostcall; a record's call, then its arguments. */
export { op };

/** TEXT_LAYOUT's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight, px and max width as f32 bits, where its answer goes and
 * how many lines it may hold. */
export { LAYOUT };

/** TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece. */
export { LAYOUT_ANSWER };

/** A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits. */
export { LINE };

/** TEXT_RASTER's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight and px as f32 bits, the color, where its pixels go, their
 * width and height, and x as f32 bits. */
export { RASTER };

/** The roots the kernel opens files below (xpute's OPENAT). Below STORAGE a root is a
 * bake, numbered by its kind (pxl-spec bake/published.spec.rs). Below every root a path is
 * an object's number in decimal, its one name; what that object is called and where, a
 * tile's version and URL, a storage key, is what the host (ts/client/src/driver/namespace/)
 * binds the root to, `@xpute/host` carrying the open there. */
export { root };

/** Below the earth's root, the Earth's picture whole (pxl-spec route.json's `TILE_SPHERE`),
 * under the earth's own version. Any other number below a bake's root is a tile's subject
 * (pxl-spec space/layer/tile.rs), which is never 0: a tile's key ends in a 1 within its
 * high half. */
export const SPHERE: u32 = 0 as number;

/** Below the asset root, the catalog the host plugs in (pxl-spec asset/catalog.rs): the
 * file each of the asset specs' references names. Any other number below it is a file,
 * its row in the asset ingest's table counted from 1 (spec/spec/asset/ingested.json). */
export const CATALOG: u32 = 0 as number;

/** Below OBJECT, the files that are no shard's; any other number below it is a shard's
 * subject, which is neither: a tile's key ends in a 1 within its high half, at bit 32 or
 * above for a tile the pyramid holds. */
export { object };

/** The zoom of the tile a shard below OBJECT is: pxl-spec-core's `SHARD_ZOOM`, which the
 * kernel holds this to (domain/layer/object/repository.rs). A key's digits to it name the
 * shard, and past it each zoom adds a digit on each of three axes (pxl-spec
 * space/hierarchy/morton.rs). */
export const SHARD_ZOOM: u32 = 8 as number;

/** The children of the tree packet a shard's file and an edit batch hold after their
 * format byte, by index: each row a write `<key, id, ext>`, a leaf as the write that made
 * it in a shard's file, in the order made in a batch, where VOID erases (pxl-kernel
 * domain/layer/object/store.rs `Write`). A record a class's (pxl-spec object/ext.rs),
 * its row and its fields as columns. */
export { write };

/** The children of the tree packet `object.TOP` holds after its format byte, by index:
 * each row a node at `SHARD_ZOOM` holding objects, which only shards' leaves are under;
 * a shard has two, below sea level and above. */
export { top };

/** The files the kernel keeps below STORAGE, by slot. What a slot holds is its
 * owner's, written whole, its slot's `format` first (pxl-kernel sys/io/file.rs). */
export { storage };

/** Each kept file's format, the first byte of what it holds: a storage slot's by its
 * name, a shard's file below OBJECT as OBJECT, the top and an edit batch as theirs. The
 * kernel reads a file of its format and refuses any other, reading none of it and never
 * writing over it, as a driver refuses a volume whose superblock it does not know; what
 * to do with the file is the host's. Bump one whenever a change makes an older file mean
 * something other than it meant when it was written. */
export { format };
