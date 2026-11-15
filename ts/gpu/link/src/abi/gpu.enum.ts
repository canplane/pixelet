// @pxl/gpu/link/src/abi/gpu.enum.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

export type ErrorCode = typeof ERROR_CODE_NO_DEVICE;
/** The host has no device to give. */
export const ERROR_CODE_NO_DEVICE = 0;

/** The member `val` is, or null for a number no member has. */
export function of_ErrorCode(val: number): ErrorCode | null {
  return val === ERROR_CODE_NO_DEVICE ? val : null;
}
