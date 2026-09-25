// pxl-kernel/layer/decor/scatter.rs

//! What a decoration kind is allowed to stand on, and how a placed one picks
//! its size and heading — the general half of the "scatter some prop across
//! eligible terrain cells" pipeline this module family exists to establish
//! (trees, boats, cars, and trains today; more kinds later declare their own
//! DecorKindSeed + eligibility rules, never their own bespoke RNG).
//!
//! The scatter itself is the kernel's (crates/pxl/web/kernel/src/layer/decor/build.rs):
//! it walks a chunk's cells into the decor block's lanes, drawing from a
//! stream seeded by the page, the kind and the cell. Which cells it may use
//! and what it does with the draws is declared here, so a kind is one
//! description rather than two rules kept in step. Nothing here reaches for a tile store — plain ground-channel
//! arrays and numbers, so it can be read head-on.

use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::cell::cube_types;
use pxl_spec::space::pyramid::PAGE_CELLS;

/// What a channel's type must be for a kind to stand on the cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Any,
    /// Nothing painted.
    Void,
    /// One of the types whose bits are set.
    OneOf([u64; 4]),
}

impl Rule {
    fn allows(&self, t: u8) -> bool {
        match self {
            Rule::Any => true,
            Rule::Void => t == cube_types::common::void,
            Rule::OneOf(bits) => bits[(t >> 6) as usize] & (1 << (t & 63)) != 0,
        }
    }
}

/// A kind's rule per ground channel, by the channel's number: what a kind's
/// file writes and what the scatter tests, one and the same. A channel no
/// rule names takes anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorRules([Rule; 4]);

impl DecorRules {
    pub const ANY: DecorRules = DecorRules([Rule::Any; 4]);

    /// `channel` has nothing painted.
    pub const fn void(mut self, channel: GroundChannelKey) -> DecorRules {
        self.0[channel as usize] = Rule::Void;
        self
    }

    /// `channel` holds one of `types`.
    pub const fn one_of(mut self, channel: GroundChannelKey, types: &[u8]) -> DecorRules {
        let mut bits = [0u64; 4];
        let mut i = 0;
        while i < types.len() {
            bits[(types[i] >> 6) as usize] |= 1 << (types[i] & 63);
            i += 1;
        }
        self.0[channel as usize] = Rule::OneOf(bits);
        self
    }

    /// Whether a cell's channels are ground the kind may stand on — the
    /// terrain paint's precedence: land is the base, and water, structure and
    /// transport each stand over it where they are painted.
    pub fn allows(&self, cell: GroundCell) -> bool {
        GroundChannelKey::ALL.iter().all(|&key| self.0[key as usize].allows(channel_of(cell, key)))
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
    let span = PAGE_CELLS as i64;
    let (x, y) = (cell_x as i64, cell_y as i64);
    let idx = y * span + x;
    let void = cube_types::common::void;
    let at = |i: i64| channel_of(cells[i as usize], GroundChannelKey::TRANSPORT) != void;
    let (mut horizontal_votes, mut vertical_votes) = (0, 0);
    for d in 1..=ALIGN_NEIGHBOR_RADIUS {
        if x - d >= 0 && at(idx - d) {
            horizontal_votes += 1;
        }
        if x + d < span && at(idx + d) {
            horizontal_votes += 1;
        }
        if y - d >= 0 && at(idx - d * span) {
            vertical_votes += 1;
        }
        if y + d < span && at(idx + d * span) {
            vertical_votes += 1;
        }
    }
    let axis_rotation = if horizontal_votes > vertical_votes { core::f64::consts::PI / 2.0 } else { 0.0 };
    if flip {
        axis_rotation + core::f64::consts::PI
    } else {
        axis_rotation
    }
}
