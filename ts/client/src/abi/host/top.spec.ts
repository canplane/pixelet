// @pxl/client/abi/host/top.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json

/** u64[]: the shard it is, by subject. */
export const SHARD = 0 as number;

/** i32[]: its index on Z, -1 below sea level and 0 above. */
export const Z = 1 as number;

/** u32[]: what its leaves fold to, its word in the objects' tree (pxl-engine
 * overwrite.rs `fold`, `fold_word`): the kind most of its fill is and its share filled. */
export const FOLD = 2 as number;
