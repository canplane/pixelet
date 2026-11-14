// @pxl/client/ui/shared/asset/marker.spec.ts
//
// GENERATED from spec/spec/asset/marker.json (xpute-gen data.rs)
//
// How the marker kind is shown: its one shape, the pin, the marker someone
// puts up at a place (id.json). A pin's cell is the camera grid's at its
// scale, as a place's icon is, its own apart from the places' since a
// pin is drawn as its own design (pxl-spec asset/marker.rs
// `marker_cell_zoom`).

/** The pin's name, the one every locale shows: an item has one name. */
export const PIN_LABEL = "Pin";

/** The most a marker's text may be, in code points: a product choice, a short line put up at a place. Bytes follow, four to a code point at most. */
export const MARKER_TEXT_CHARS = 40 as number;

/** The coarsest camera grid cell a scene marker of these shows at: z19, 32 of them across
 * a z14 tile, the zoom OpenMapTiles' `poi` layer holds every point at
 * (layers/poi/poi.yaml). What someone puts up, by the cells of the brick
 * it is read from (pxl-core domain/scene/marker/pin.rs); a chat line's
 * bubble, which no brick holds, by the cell where its speaker stands,
 * which its speaker judges (pxl-core
 * domain/scene/character/companion.rs `line_hangs`). */
export const MARKER_SHOWN_FROM = 19 as number;

/** A pin's cell on the camera grid, in zooms: −1 spans two of the grid's
 * cells, as a place's icon does. A cell shows one pin: where more than
 * one stands in it, they are one, their count on a pin's pill in its
 * colors turned over, at the cell's middle over what is drawn there
 * (pxl-core domain/scene/marker/pin.rs). */
export const MARKER_SCALE_STEP = -1 as number;
