// @pxl/client/abi/host/write.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json

/** u64[]: each key's high half (pxl-spec space/hierarchy/morton.rs `key_of`, 3D). */
export const KEY_HI = 0 as number;

/** u64[]: each key's low half. */
export const KEY_LO = 1 as number;

/** u32[]: each catalog value. */
export const ID = 2 as number;

/** u32[]: the rows a marker's text is for, a marker's each. */
export const TEXT_ROW = 3 as number;

/** strs: those markers' texts. */
export const TEXT = 4 as number;
