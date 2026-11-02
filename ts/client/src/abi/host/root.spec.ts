// @pxl/client/abi/host/root.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json

/** The platform's storage; a path is a storage file's number in decimal (`storage`); where the platform holds a slot is the host's choice. */
export const STORAGE = 256 as number;

/** The shipped assets; a path is a load's number in decimal, the catalog's (`CATALOG`) or a file it names (sys/io/loader.rs). */
export const ASSET = 257 as number;

/** The placed objects, their source's: a file a shard, read, its path the shard's subject in
 * decimal, as a bake's tile is named (pxl-spec space/layer/tile.rs), its `format.OBJECT`
 * first; the top of their tree, read (`object.TOP`); and their edits, written
 * (`object.EDITS`) (domain/layer/object/repository.rs). A shard with none has no file. The
 * source keeps the shards and the top and applies the edits; which source, the platform's
 * storage or a server, is the host's. */
export const OBJECT = 258 as number;
