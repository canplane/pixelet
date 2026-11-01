// @xpute/host/conformance/hostcall.spec.ts
//
// GENERATED from spec/conformance/guest.json

/** A number and an offset: the host writes the number plus one at the offset and answers 0,
 * before the hostcall returns; a number of 0 it refuses with EINVAL, writing nothing. */
export const ANSWER = 2147483649 as number;
