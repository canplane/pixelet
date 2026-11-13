// pxl-spec/asset/marker.spec.rs
//
// GENERATED from spec/spec/asset/marker.json (xpute-gen data.rs)
//
// How the marker kind is shown: its one shape, the pin, the marker someone
// puts up at a place (id.json). A pin's cell is the camera grid's at its
// scale, as a place's icon is, its own apart from the places' since a
// pin is drawn as its own design (pxl-spec asset/marker.rs
// `marker_cell_zoom`).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    reason = "generated from the spec"
)]

/// The pin's name, the one every locale shows: an item has one name.
pub const PIN_LABEL: &str = "Pin";

/// The most a marker's text may be, in code points: a product choice, a short line put up at a place. Bytes follow, four to a code point at most.
pub const MARKER_TEXT_CHARS: u32 = 40;

/// The coarsest camera grid cell a scene marker of these shows at: z19, 32 of them across
/// a z14 tile, the zoom OpenMapTiles' `poi` layer holds every point at
/// (layers/poi/poi.yaml). What someone puts up, by the cells of the brick
/// it is read from (pxl-core domain/scene/marker/pin.rs); a chat line's
/// bubble, which no brick holds, by the cell where its speaker stands,
/// which its speaker judges (pxl-core
/// domain/scene/character/companion.rs `line_hangs`).
pub const MARKER_SHOWN_FROM: u8 = 19;

/// A pin's cell on the camera grid, in zooms: −1 spans two of the grid's
/// cells, as a place's icon does. A cell shows one pin: where more than
/// one stands in it, they are one, their count on a pin's pill in its
/// colors turned over, at the cell's middle over what is drawn there
/// (pxl-core domain/scene/marker/pin.rs).
pub const MARKER_SCALE_STEPS: i8 = -1;
