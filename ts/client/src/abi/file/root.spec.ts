// @pxl/client/abi/file/root.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/file.json

/** The platform's storage; a path is a storage file's id in decimal (`storage`); where the platform holds a slot is the client's choice. */
export const STORAGE = 256 as number;

/** The shipped assets; a path is a load's id in decimal, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs). */
export const ASSET = 257 as number;

/** The placed objects, their source's: a file a shard, read, its path the shard's quadkey,
 * as a bake's tile is named, its `format.OBJECT` first; the top of their tree, read
 * (`object.TOP`); and their edits, written (`object.EDITS`)
 * (domain/layer/object/repository.rs). A shard with none has no file. The source keeps the
 * shards and the top and applies the edits; which source, a mock or a server, is the client's. */
export const OBJECT = 258 as number;
