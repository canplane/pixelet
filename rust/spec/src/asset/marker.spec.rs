// pxl-spec/asset/marker.spec.rs
//
// GENERATED from spec/spec/asset/marker.json
//
// How the marker kind is shown: its one shape, the pin, the marker someone
// puts up at a place (id.json). A marker's cell is the camera grid's at its
// scale, as a place's icon is, its own apart from the places' since a
// marker is drawn as its own design (pxl-spec asset/marker.rs
// `marker_cell`). Per id where the kind comes to hold more than one shape.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The most a marker's words may be, in code points: a product choice, a short line put up at a place. Bytes follow, four to a code point at most.
pub const MARKER_TEXT_CHARS: u32 = 40;

/// The coarsest camera grid cell a marker shows at: z19, 32 of them across
/// a z14 tile, the zoom OpenMapTiles' `poi` layer holds every point at
/// (layers/poi/poi.yaml). What someone puts up, by the cells of the brick
/// it is read from (pxl-kernel domain/scene/marker/pin.rs); a chat line's
/// bubble, which no brick holds, by the cell where its speaker stands,
/// which its speaker judges (pxl-kernel
/// domain/scene/character/companion.rs `line_hangs`).
pub const MARKER_SHOWN_FROM: u8 = 19;

/// A marker's cell on the camera grid, in zooms: −1 spans two of the grid's
/// cells, as a place's icon does. A cell shows one marker: where more than
/// one stands in it, they are one, their count on a marker's pill in its
/// colors turned over, at the cell's middle over what is drawn there
/// (pxl-kernel domain/scene/marker/pin.rs).
pub const MARKER_SCALE_STEPS: i8 = -1;
