// @pxl/client/abi/host/object.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json

/** The top of the tree, read: what each node at `SHARD_ZOOM` holding objects folds to
 * (`top`), so which shards hold any, its `format.OBJECT_TOP` first. Its source rebuilds
 * it from the shards as an edit lands; none is no shard holding any. */
export const TOP = 0 as number;

/** Edits, written, never read: a batch of writes in the order they were made (`write`),
 * its `format.OBJECT_EDITS` first, which the source applies to its shards. */
export const EDITS = 1 as number;
