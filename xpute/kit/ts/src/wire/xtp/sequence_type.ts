// @xpute/kit/wire/xtp/sequence_type.ts

// These are the type bytes of the sequence leaves, each of which holds many
// values after a len slot. spec.ts re-exports this module as `SequenceType`,
// read as a namespace (`SequenceType.U8_ARRAY`). Each constant is a literal, so
// it is its own type, and `SequenceType` as a type is their union.

export const U8_ARRAY = 0xa0; // BIT_SEQ | I(0, 0)
export const I8_ARRAY = 0xa8; // BIT_SEQ | I(0, 1)
export const U16_ARRAY = 0xa1; // BIT_SEQ | I(1, 0)
export const I16_ARRAY = 0xa9; // BIT_SEQ | I(1, 1)
export const U32_ARRAY = 0xa2; // BIT_SEQ | I(2, 0)
export const I32_ARRAY = 0xaa; // BIT_SEQ | I(2, 1)
export const U64_ARRAY = 0xa3; // BIT_SEQ | I(3, 0)
export const I64_ARRAY = 0xab; // BIT_SEQ | I(3, 1)
export const F32_ARRAY = 0xb2; // BIT_SEQ | F(2)
export const F64_ARRAY = 0xb3; // BIT_SEQ | F(3)

export const BITSET = 0xb8; // BIT_SEQ | R(0)
/** UTF-8 bytes followed by a NUL, which `len` does not count. */
export const STR = 0xb9; // BIT_SEQ | R(1)
/** Many strings in one leaf, so that a string costs no node of its own:
 * `len` + 1 u32 offsets, each no less than the one before, then the UTF-8
 * they index. String `i` lies between offsets `i` and `i + 1`. */
export const STRS = 0xba; // BIT_SEQ | R(2)
