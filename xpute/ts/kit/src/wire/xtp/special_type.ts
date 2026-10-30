// @xpute/kit/wire/xtp/special_type.ts

// The types with the leaf bit clear. One constant a member, read as a
// namespace (`SpecialType.NIL`, re-exported by spec.ts): no enum, so a file
// transpiled alone or a stripped one reads it, and a bundle inlines it. Literals, so each
// is its own type and the group a union of them (spec.ts).

export const NIL = 0x00; // untyped null

export const BRANCH = 0x20;

export const GRAFT = 0x40;
