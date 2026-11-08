// @xpute/kit/wire/xtp/special_type.ts

// These are the type bytes with the leaf bit clear. spec.ts re-exports this
// module as `SpecialType`, read as a namespace (`SpecialType.NIL`). Each
// constant is a literal, so it is its own type, and `SpecialType` as a type is
// their union.

/** The untyped null. An absent node is not NIL: it keeps its own type. */
export const NIL = 0x00;

export const BRANCH = 0x20;

/** Never on the wire: a graft is written as its packet's root, under that
 * root's type. */
export const GRAFT = 0x40;
