// pxl-kernel/layer/decor/scatter.rs

//! What a decor kind may stand on and how it picks its heading, declared
//! once per kind; the scatter itself is build.rs's.

use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::bake::ground::shown_ground;
use pxl_spec::bake::pack::{channel_of, GroundCell};
use pxl_spec::cell::cube_types;
use pxl_spec::space::pyramid::{GROUND, PAGE_CELLS};

/// A kind stands where the cell *shows* one of `types` on `channel`: a cell
/// shows its last painted channel (`shown_ground`), so a tree stands only
/// where no water, way or structure is over the land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorRules {
    channel: GroundChannelKey,
    types: [u64; 4],
}

impl DecorRules {
    pub const fn on(channel: GroundChannelKey, types: &[u8]) -> DecorRules {
        let mut bits = [0u64; 4];
        let mut i = 0;
        while i < types.len() {
            bits[(types[i] >> 6) as usize] |= 1 << (types[i] & 63);
            i += 1;
        }
        DecorRules { channel, types: bits }
    }

    pub fn allows(&self, cell: GroundCell) -> bool {
        let shown = shown_ground(|key| u32::from(channel_of(cell, key)));
        let t = shown.type_id as u8;
        shown.channel as u8 == self.channel as u8 && self.types[(t >> 6) as usize] & (1 << (t & 63)) != 0
    }
}

/// AlignTransport faces along the road or rail the cell sits on, for things
/// that look crashed at a random spin (cars, trains).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorOrientationMode {
    Random,
    AlignTransport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorKindSeed {
    /// Changing it reshuffles every placement of the kind.
    pub tag: u64,
    /// Mean candidates per cell at its zoom: the intensity of the Poisson process
    /// that scatters it (`pxl-spec procedural.rs`).
    pub density: f64,
    pub orientation: DecorOrientationMode,
}

// Variety between instances without any reading as the wrong size.
pub const SCALE_JITTER: f64 = 0.18;

// A fixed radius (O(1) reads, no flood fill), enough to outvote a one-cell
// stairstep jog in a straight run; it does not resolve curves.
const ALIGN_NEIGHBOR_RADIUS: i64 = 3;

/// A plurality vote of transport cells along each axis: PI/2 if horizontal
/// wins, else 0 (a tie or isolated cell has no real answer). `flip` adds PI,
/// as two-way traffic faces both ways.
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
