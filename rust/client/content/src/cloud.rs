// pxl-content/cloud.rs

//! wgsl-port: the cloud field that the cumulus and the sheets share: the lattice it lies
//! on, the wind that carries it and the hash the sheets' noise sums over on the GPU
//! (pxl-render frame/shader/noise.wgsl), whose `lattice_hash` pxl-render build.rs writes
//! from `HASH_IN`, `HASH_ROUNDS`, `HASH_LAST` and `HASH_SHARE_BITS`. The
//! cumulus is handed down and drawn by `node_seed`, as the decor and the caves are
//! (`holds_cloud`, `cloud_draw`).

use pxl_engine::seed::node_seed;
use pxl_spec::space::coord::zoom::{Zoom, UNIT_ZOOM};
use pxl_spec::space::hierarchy::tree::child_idx;
use xpute_kit::math::rng::seed_mix32;

use crate::halve;
use crate::param::cloud::LATTICE_SHAMT;
use crate::param::cloud::WIND;

/// The lattice units around the world: X's 2^24 units divided by a lattice unit's.
pub const LATTICE_CNT: u32 = 1 << (UNIT_ZOOM - LATTICE_SHAMT);

/// How far the field has moved under the map `time` seconds in, in cells.
pub fn drift(time: f64) -> [f64; 2] {
    [WIND[0] * time, WIND[1] * time]
}

/// The hash's multipliers of a cell's x, its y and the salt, whose products
/// are xored.
pub const HASH_IN: [u32; 3] = [0x8da6_b343, 0xd816_3841, 0xcb1a_b31f];
/// The hash's rounds: each xors in the word shifted down by the first number, then multiplies
/// by the second.
pub const HASH_ROUNDS: [[u32; 2]; 2] = [[16, 0x7feb_352d], [15, 0x846c_a68b]];
/// The shift of the last xor, after the rounds.
pub const HASH_LAST: u32 = 16;
/// The top bits kept as the share: an f32's whole significand, so the share
/// is exact.
pub const HASH_SHARE_BITS: u32 = 24;

/// The lattice's zoom: a cell of the field is a node of this zoom across.
const CELL_ZOOM: Zoom = UNIT_ZOOM - LATTICE_SHAMT;
/// A patch of cells, 16 a side, at which a cumulus count starts.
const PATCH_ZOOM: Zoom = CELL_ZOOM - 4;
/// The tags of a patch's count and its split (`node_seed`).
const CUMULUS_TAG: u32 = 0xc10d_0001;
const CUMULUS_SPLIT_TAG: u32 = 0xc10d_0002;

/// Cell `cell`'s draw under `salt`, in [0, 1), from which a cumulus there takes its place,
/// size, turn and variant, by the one seed the decor draws theirs by (`node_seed`).
pub fn cloud_draw(cell: [i32; 2], salt: u32) -> f64 {
    f64::from(seed_mix32(node_seed(CUMULUS_TAG, (CELL_ZOOM, cell)), salt)) / 4_294_967_296.0
}

/// Whether the field's cell `cell` holds a cumulus. Its patch holds `mean` of its 256 cells,
/// give or take a hash's draw, and the count is halved across and along, a step at a time, by
/// weights a hash draws, pushed apart so that clouds gather in clumps (`halve::quarter`, the
/// one rule the decor and the caves hand theirs down by), down to one or none a cell.
pub fn holds_cloud(cell: [i32; 2], mean: u32) -> bool {
    let at = |z: Zoom| [cell[0] >> (CELL_ZOOM - z), cell[1] >> (CELL_ZOOM - z)];
    let draw = node_seed(CUMULUS_TAG, (PATCH_ZOOM, at(PATCH_ZOOM)));
    let n = mean as i32 + (draw & 0x3f) as i32 - 32;
    let mut n = n.clamp(0, 256) as u32;
    let mut z = PATCH_ZOOM;
    while z < CELL_ZOOM && n > 0 {
        let h = node_seed(CUMULUS_SPLIT_TAG, (z, at(z)));
        // A weight of 2^16 by hash, pushed out from the middle so the halves
        // come uneven and the clouds gather.
        let w = |b: u32| {
            let x = ((h >> b) & 0xff) as i32 - 128;
            let far = 128 - (128 - x.abs()) * (128 - x.abs()) / 128;
            (32768 + x.signum() * far * 255) as u32
        };
        let cap = 1u32 << (2 * (CELL_ZOOM - z - 1));
        let c = at(z + 1);
        let quarter = child_idx(c);
        n = halve::quarter(
            n,
            cap,
            [halve::weight(w(0), h >> 16), halve::weight(w(8), h)],
            quarter as u32,
        );
        z += 1;
    }
    n > 0
}

#[cfg(test)]
#[path = "cloud.test.rs"]
mod test;
