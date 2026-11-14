// @pxl/client/ui/feature/search/search.spec.ts
//
// GENERATED from spec/spec/search.json5 (xpute-gen data.rs)
//
// What the search endpoint (route.json5 `SEARCH_URL`) answers, by number: a
// result's kind, which says what its `category` numbers. A place's is its
// category as a place tile carries it (bake/place_category.json5, counted from
// 1, 0 for none); a division's is its subtype (division.json5), as each part
// of a result's `context` gives its own.

export const SEARCH_KIND = {
  PLACE: 0 as number,
  DIVISION: 1 as number,
} as const;
