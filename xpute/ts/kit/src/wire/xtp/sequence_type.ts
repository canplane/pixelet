// @xpute/kit/wire/xtp/sequence_type.ts

// A leaf of many values, by its type byte. One constant a member, read as a
// namespace (`SequenceType.U8_ARRAY`, re-exported by spec.ts): no enum, so a file
// transpiled alone or a stripped one reads it, and a bundle inlines it. Literals, so each
// is its own type and the group a union of them (spec.ts).

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
export const STR = 0xb9; // BIT_SEQ | R(1)
/** `len` + 1 u32 offsets into the UTF-8 that follows them. */
export const STRS = 0xba; // BIT_SEQ | R(2)
