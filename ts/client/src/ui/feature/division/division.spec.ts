// @pxl/client/ui/feature/division/division.spec.ts
//
// GENERATED from spec/spec/division.json5 (xpute-gen data.rs)
//
// A division's subtype, Overture's, broadest first (pxl-spec division.rs
// `Subtype`): a name is a division and its ancestors, and the client picks
// how deep to read it. The numbers are stored, so a subtype is appended,
// never inserted.

export const DIVISION_NAME = {
  /** The most bytes of a division's name in UTF-8 that the overlay's snapshot carries. A tile holds its names whole, and the snapshot cuts a longer one at a character's boundary; it carries at most DIVISION_SUBTYPE's count of a chain's names, the narrowest (pxl-core ui.rs `division_parts`). */
  BYTES: 128 as number,
} as const;

export const DIVISION_SUBTYPE = {
  COUNTRY: 0 as number,
  DEPENDENCY: 1 as number,
  MACROREGION: 2 as number,
  REGION: 3 as number,
  MACROCOUNTY: 4 as number,
  COUNTY: 5 as number,
  LOCALADMIN: 6 as number,
  LOCALITY: 7 as number,
  BOROUGH: 8 as number,
  MACROHOOD: 9 as number,
  NEIGHBORHOOD: 10 as number,
  MICROHOOD: 11 as number,
} as const;
