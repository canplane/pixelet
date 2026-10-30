// @xpute/kit/wire/tlv_tag.ts

// A TLV value's tag byte, a constant a tag, read as a namespace by tlv.ts
// (`Tag.END`): no enum, so a file transpiled alone or a stripped one reads it.

export const END = 0x00;

export const BOOL = 0x01;

export const U8 = 0x02;
export const I8 = 0x03;
export const U16 = 0x04;
export const I16 = 0x05;
export const U32 = 0x06;
export const I32 = 0x07;
export const U64 = 0x08;
export const I64 = 0x09;
export const F32 = 0x0a;
export const F64 = 0x0b;

export const STR = 0x10; // [u32 len][utf8 bytes]

export const BYTES = 0x20;
