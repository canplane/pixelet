// @xpute/kit/wire/xtp/scalar_type.ts

// These are the type bytes of the scalar leaves, each of which holds one value.
// spec.ts re-exports this module as `ScalarType`, read as a namespace
// (`ScalarType.U8`). Each constant is a literal, so it is its own type, and
// `ScalarType` as a type is their union.

export const U8 = 0x80; // I(0, 0)
export const I8 = 0x88; // I(0, 1)
export const U16 = 0x81; // I(1, 0)
export const I16 = 0x89; // I(1, 1)
export const U32 = 0x82; // I(2, 0)
export const I32 = 0x8a; // I(2, 1)
export const U64 = 0x83; // I(3, 0)
export const I64 = 0x8b; // I(3, 1)
export const F32 = 0x92; // F(2)
export const F64 = 0x93; // F(3)

export const BOOL = 0x98; // R(0)
