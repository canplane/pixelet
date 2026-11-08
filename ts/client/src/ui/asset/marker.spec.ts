// @pxl/client/ui/asset/marker.spec.ts
//
// GENERATED from spec/spec/asset/marker.json
//
// How the marker kind is shown: its one shape, the pin, the marker someone
// puts up at a place (id.json). A pin's cell is the camera grid's at its
// scale, as a place's icon is, its own apart from the places' since a
// pin is drawn as its own design (pxl-spec asset/marker.rs
// `marker_cell_zoom`).

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The pin's name, the one every locale shows: an item has one name. */
export const PIN_LABEL = "Pin";

/** The most a marker's text may be, in code points: a product choice, a short line put up at a place. Bytes follow, four to a code point at most. */
export const MARKER_TEXT_CHARS: u32 = 40 as number;
