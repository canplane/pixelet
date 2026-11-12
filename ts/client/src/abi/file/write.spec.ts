// @pxl/client/abi/file/write.spec.ts
//
// GENERATED from spec/client/core/sys/io/file.json

/** A `strs` column holding each key as an octkey (pxl-spec space/hierarchy/morton.rs `name`, 3D): the shard's quadkey, then a digit 0 to 7 a zoom. */
export const KEY = 0 as number;

/** A `u32[]` column holding each row's catalog value. */
export const ASSET_ID = 1 as number;

/** A `u32[]` column holding, for each marker with a text, the row that text is for. */
export const TEXT_ROW = 2 as number;

/** A `strs` column holding those markers' texts. */
export const TEXT = 3 as number;
