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

use pxl_spec::bake::constant::PAGE_CELL_SPAN;
use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::cell::cube_types;

/// What a channel's type must be for a kind to stand on the cell: anything,
/// nothing painted, or one of a list of types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecorChannelRule {
    Any,
    Void,
    OneOf(Vec<u32>),
}

/// A kind's rule per ground channel, declared so the scheduler can be taught
/// the same rule (crates/pxl/web/kernel/src/layer/decor/build.rs) and `eligibility_of`
/// derives the predicate from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecorRules {
    pub land: DecorChannelRule,
    pub water: DecorChannelRule,
    pub structure: DecorChannelRule,
    pub transport: DecorChannelRule,
}

impl DecorRules {
    /// A channel's rule, in GROUND_CH_KEYS' order.
    pub fn channel(&self, index: usize) -> &DecorChannelRule {
        match GROUND_CH_KEYS[index] {
            GroundChannelKey::LAND => &self.land,
            GroundChannelKey::WATER => &self.water,
            GroundChannelKey::STRUCTURE => &self.structure,
            GroundChannelKey::TRANSPORT => &self.transport,
        }
    }
}

fn rule_allows(rule: &DecorChannelRule, t: u32) -> bool {
    match rule {
        DecorChannelRule::Any => true,
        DecorChannelRule::Void => t == cube_types::common::void as u32,
        DecorChannelRule::OneOf(types) => types.contains(&t),
    }
}

/// A kind's rules as kind.json writes them, in the numbers a cell's channels
/// hold: each channel's types where it lists them, nothing painted for the
/// channels its `void` names, and anything else.
pub fn rules_of(void: &[GroundChannelKey], land: &[u32], water: &[u32], structure: &[u32], transport: &[u32]) -> DecorRules {
    let types = [land, water, structure, transport];
    let rule = |channel: GroundChannelKey| {
        if void.contains(&channel) {
            DecorChannelRule::Void
        } else if types[channel as usize].is_empty() {
            DecorChannelRule::Any
        } else {
            DecorChannelRule::OneOf(types[channel as usize].to_vec())
        }
    };
    DecorRules {
        land: rule(GroundChannelKey::LAND),
        water: rule(GroundChannelKey::WATER),
        structure: rule(GroundChannelKey::STRUCTURE),
        transport: rule(GroundChannelKey::TRANSPORT),
    }
}

/// True if this cell's raw ground-channel values are safe ground for this
/// decoration kind to stand on — mirrors the terrain paint's ground_material_at
/// precedence (land is the base, water/structure/transport each override it
/// only when not VOID).
pub fn eligibility_of(rules: &DecorRules, land: u32, water: u32, structure: u32, transport: u32) -> bool {
    rule_allows(&rules.land, land) && rule_allows(&rules.water, water) && rule_allows(&rules.structure, structure) && rule_allows(&rules.transport, transport)
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
    /// Uniformly picks a variant index in [0, variant_count).
    pub variant_count: u32,
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
    let span = PAGE_CELL_SPAN as i64;
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

#[cfg(test)]
#[path = "scatter.test.rs"]
mod test;
