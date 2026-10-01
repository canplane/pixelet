// pxl-kernel/layer/decor/scatter.rs

//! What a decoration kind is allowed to stand on, and how a placed one picks
//! its size and heading — the general half of the "scatter some prop across
//! eligible terrain cells" pipeline this module family exists to establish
//! (trees, boats, cars, and trains today; more kinds later declare their own
//! DecorKindSeed + eligibility rules, never their own bespoke RNG).
//!
//! The scatter itself is the kernel's (crates/pxl/web/kernel/src/layer/decor/build.rs):
//! it walks a decor cell's cells into its block's lanes, drawing from a
//! stream seeded by the page, the kind and the cell. Which cells it may use
//! and what it does with the draws is declared here, so a kind is one
//! description rather than two rules kept in step. Nothing here reaches for a tile store — plain ground-channel
//! arrays and numbers, so it can be read head-on.

use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::bake::ground::shown_ground;
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::cell::cube_types;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};

/// The ground a kind stands on: one channel and the types of it, as the
/// cell shows them. A cell shows the last of its channels that is painted,
/// in the paint's order (pxl-spec bake/ground.rs `shown_ground`), so a kind
/// on one channel stands only where every channel after it is empty: a tree
/// on land where no water, way or structure is over it, a boat on water
/// where no way or structure is, a car on a way where no structure is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorRules {
    channel: GroundChannelKey,
    types: [u64; 4],
}

impl DecorRules {
    /// On `channel`, where it shows one of `types`.
    pub const fn on(channel: GroundChannelKey, types: &[u8]) -> DecorRules {
        let mut bits = [0u64; 4];
        let mut i = 0;
        while i < types.len() {
            bits[(types[i] >> 6) as usize] |= 1 << (types[i] & 63);
            i += 1;
        }
        DecorRules { channel, types: bits }
    }

    /// Whether a cell shows ground the kind may stand on.
    pub fn allows(&self, cell: GroundCell) -> bool {
        let shown = shown_ground(|key| u32::from(channel_of(cell, key)), 0);
        let t = shown.type_id as u8;
        shown.channel as u8 == self.channel as u8 && self.types[(t >> 6) as usize] & (1 << (t & 63)) != 0
    }
}

/// How a placed instance picks its own heading:
/// - Random: uniform over a full turn — fine for anything that reads the same
///   from any angle (a tree, a boat drifting on open water).
/// - AlignTransport: faces along whichever axis (X or Y) this cell's own
///   transport channel actually runs, read off its immediate neighbors — for
///   anything that only looks right lined up with a road/rail (a car, a
///   train), where a uniformly random spin would read as "crashed", not
///   "parked/passing through".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorOrientationMode {
    Random,
    AlignTransport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorKindSeed {
    /// Distinguishes this kind's own random stream from another kind scattered
    /// over the very same cells — any value works as long as it stays fixed for
    /// this kind across sessions; changing it reshuffles every placement.
    pub tag: u64,
    /// Per-eligible-cell probability [0, 1) of actually placing an instance
    /// there — the only knob "how dense" tuning needs.
    pub density: f64,
    pub orientation: DecorOrientationMode,
}

// +/- this fraction around 1.0 — visible variety between instances of the
// same variant without any one of them reading as a visibly wrong size.
pub const SCALE_JITTER: f64 = 0.18;

// How far transport_aligned_rotation looks past the immediate neighbor on each
// side — a fixed, small radius (never a flood-fill: O(1) reads per cell,
// independent of how long the actual road/rail run is), just enough to outvote
// a single-cell diagonal "stairstep" jog in an otherwise straight run. A true
// curve still won't resolve to the right heading at any radius — this only
// smooths over rasterization noise on a fundamentally straight segment.
const ALIGN_NEIGHBOR_RADIUS: i64 = 3;

/// "align_transport" heading: counts populated transport cells up to
/// ALIGN_NEIGHBOR_RADIUS out along each axis and picks whichever axis has more
/// (a plurality vote) — PI/2 if the horizontal count wins, 0 otherwise
/// (vertical, a tie, or a fully isolated cell; a tie or isolated cell has no
/// real answer, so 0 is only an arbitrary-but-deterministic fallback).
/// Bounds-checked against the page edge itself, since `transport` here is
/// always a full page's own array.
///
/// `flip` adds a random +PI on top of that axis — still lined up with the
/// road/rail either way, but facing one direction or the opposite one, same as
/// real two-way traffic never all pointing the same way.
pub fn transport_aligned_rotation(cells: &[GroundCell], cell_x: u32, cell_y: u32, flip: bool) -> f64 {
    let span = 0..PAGE_CELLS as i64;
    let (x, y) = (cell_x as i64, cell_y as i64);
    let void = cube_types::common::void;
    let at = |x: i64, y: i64| span.contains(&x) && span.contains(&y) && channel_of(cells[GROUND.cell_index(x as u32, y as u32) as usize], GroundChannelKey::TRANSPORT) != void;
    let (mut horizontal_votes, mut vertical_votes) = (0, 0);
    for d in 1..=ALIGN_NEIGHBOR_RADIUS {
        horizontal_votes += at(x - d, y) as u32 + at(x + d, y) as u32;
        vertical_votes += at(x, y - d) as u32 + at(x, y + d) as u32;
    }
    let axis_rotation = if horizontal_votes > vertical_votes { core::f64::consts::PI / 2.0 } else { 0.0 };
    if flip {
        axis_rotation + core::f64::consts::PI
    } else {
        axis_rotation
    }
}
