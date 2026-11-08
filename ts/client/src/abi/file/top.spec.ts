// @pxl/client/abi/file/top.spec.ts
//
// GENERATED from spec/client/core/sys/io/file.json

/** A `strs` column holding each row's shard by its quadkey. */
export const SHARD = 0 as number;

/** A `u32[]` column holding what each row's leaves fold to, its word in the objects' tree
 * (pxl-engine object.rs `fold`, `fold_word`): the kind most of its fill is and its share filled. */
export const FOLD = 1 as number;
