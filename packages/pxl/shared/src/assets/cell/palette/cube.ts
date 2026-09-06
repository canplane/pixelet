// @pxl/shared/assets/cell/palette/cube.ts
//
// Raw hex swatches for spec/cell/cube.ts's own channels — a plain TS module,
// not a real .json file imported via `with { type: "json" }` (2026-09-06):
// @deno/vite-plugin@1.0.6's own resolver (resolver.js's resolveDeno) only
// handles `deno info`'s "esm"/"npm"/"external" module kinds and throws
// `Unsupported: { kind: "asserted", ... }` for any JSON-import-assertion
// module living outside apps/web's own vite root — real, reproduced in this
// project's dev server, not a theoretical concern. A plain .ts export is the
// same shape download_texture_pack.ts's own manifest.ts/wire.ts already use
// for static data in this same assets/ tree, and sidesteps the whole
// import-assertion module kind, since Deno never needs to special-case a
// plain JS object literal the way it does a JSON module.
export default {
  _meta: { source: "land/water/structure/transport: OpenStreetMap Carto; block: hand-picked" },
  land: {
    BARE: { hex: "#EEE5DC" },
    SEA: { hex: "#AAD3DF" },
    GREEN_DEEP: { hex: "#ADD19E" },
    GREEN_LIGHT: { hex: "#C8FACC" },
    SAND: { hex: "#F5E9C6" },
    RESIDENTIAL: { hex: "#E0DFDF" },
    COMMERCIAL: { hex: "#F2DAD9" },
    INDUSTRIAL: { hex: "#EBDBE8" },
    SPORTS: { hex: "#88E0BE" },
    EDUCATION: { hex: "#FFFFE5" },
    MEDICAL: { hex: "#FFFFE5" },
    PUBLIC: { hex: "#FFFFE5" },
    APRON: { hex: "#DADAE0" },
    HELIPAD: { hex: "#BBBBCC" },
  },
  water: {
    RIVER: { hex: "#AAD3DF" },
    LAKE: { hex: "#AAD3DF" },
    WETLAND: { hex: "#AAD3DF" },
  },
  structure: {
    BUILDING: { hex: "#D9D0C9" },
    BUILDING_STATION: { hex: "#C4B6AB" },
    BUILDING_MEDICAL: { hex: "#D9D0C9" },
    BUILDING_EDUCATION: { hex: "#D9D0C9" },
    BUILDING_PUBLIC: { hex: "#D9D0C9" },
  },
  transport: {
    ROAD_DEFAULT: { hex: "#FFFFFF" },
    ROAD_MOTORWAY: { hex: "#E892A2" },
    ROAD_PRIMARY: { hex: "#FCD6A4" },
    ROAD_SECONDARY: { hex: "#F7FABF" },
    PEDESTRIAN: { hex: "#DDDDE8" },
    PARKING: { hex: "#EEEEEE" },
    RAIL_MAIN: { hex: "#707070" },
    RAIL_SUBWAY: { hex: "#999999" },
    RUNWAY: { hex: "#BBBBCC" },
    TAXIWAY: { hex: "#BBBBCC" },
  },
  block: {
    white: { hex: "#f7f7f7" },
    grey: { hex: "#8d8d8d" },
    dark_grey: { hex: "#3f3f3f" },
    black: { hex: "#191919" },
    red: { hex: "#b01313" },
    pink: { hex: "#ff8787" },
    brown: { hex: "#572b00" },
    orange: { hex: "#d44e14" },
    yellow: { hex: "#ffe611" },
    green: { hex: "#60dc1e" },
    dark_green: { hex: "#206700" },
    cyan: { hex: "#008c94" },
    blue: { hex: "#004791" },
    violet: { hex: "#57039f" },
    magenta: { hex: "#c9036d" },
  },
};
