// pxl-content/cloud.rs

//! gpu-portable: the cloud field the cumulus and the sheets share. The
//! lattice it lies on, the wind that carries it and the hash its cells draw
//! from: the cumulus draws its cells from the hash on the CPU (pxl-kernel
//! domain/scene/environment/cloud.rs), and the sheets sum the hash's noise on the
//! GPU (pxl-render frame/shader/noise.wgsl), whose `lattice_hash` pxl-render
//! build.rs writes from `HASH_IN`, `HASH_ROUNDS`, `HASH_LAST` and `HASH_SHARE_BITS`.

use pxl_engine::seed::node_seed;
use pxl_engine::{Zoom, UNIT_ZOOM};

use crate::param::cloud::LATTICE_SHAMT;
use crate::param::cloud::WIND;
use crate::share::halve;

/// The lattice's units around the world, X's 2^24 units over a lattice unit's.
pub const LATTICE_CNT: u32 = 1 << (UNIT_ZOOM - LATTICE_SHAMT);

/// How far the field has moved under the map `time` seconds in, in cells.
pub fn drift(time: f64) -> [f64; 2] {
    [WIND[0] * time, WIND[1] * time]
}

/// The hash's multipliers of a cell's x, its y and the salt, whose products
/// are xored.
pub const HASH_IN: [u32; 3] = [0x8da6_b343, 0xd816_3841, 0xcb1a_b31f];
/// Its rounds: each xors in the word shifted down by the first, then
/// multiplies by the second.
pub const HASH_ROUNDS: [[u32; 2]; 2] = [[16, 0x7feb_352d], [15, 0x846c_a68b]];
/// The shift of the last xor, after the rounds.
pub const HASH_LAST: u32 = 16;
/// The top bits kept as the share: an f32's whole significand, so the share
/// is exact on either side.
pub const HASH_SHARE_BITS: u32 = 24;

/// Cell `[x, y]`'s hash under `salt`, in [0, 1).
pub fn lattice_hash([x, y]: [u32; 2], salt: u32) -> f32 {
    let mut h =
        x.wrapping_mul(HASH_IN[0]) ^ y.wrapping_mul(HASH_IN[1]) ^ salt.wrapping_mul(HASH_IN[2]);
    let mut k = 0;
    while k < HASH_ROUNDS.len() {
        h ^= h >> HASH_ROUNDS[k][0];
        h = h.wrapping_mul(HASH_ROUNDS[k][1]);
        k += 1;
    }
    h ^= h >> HASH_LAST;
    (h >> (32 - HASH_SHARE_BITS)) as f32 / (1u32 << HASH_SHARE_BITS) as f32
}

/// The lattice's zoom: a cell of the field a node there, across.
const CELL_ZOOM: Zoom = UNIT_ZOOM - LATTICE_SHAMT;
/// A patch of cells a cumulus count starts at, 16 a side.
const PATCH_ZOOM: Zoom = CELL_ZOOM - 4;
/// The tags of a patch's count and its split (`node_seed`).
const CUMULUS: u32 = 0xc10d_0001;
const CUMULUS_SPLIT: u32 = 0xc10d_0002;

/// Cell `cell`'s draw under `salt`, in [0, 1): what a cumulus there takes
/// its place, size, turn and variant from, by the one seed the decor draws
/// theirs by (`node_seed`).
pub fn cloud_draw(cell: [i32; 2], salt: u32) -> f64 {
    f64::from(node_seed(CUMULUS ^ salt, (CELL_ZOOM, cell))) / 4_294_967_296.0
}

/// Whether the field's cell `cell` holds a cumulus: its patch holds `mean`
/// of its 256 about a hash's draw, halved across and along a step by weights
/// a hash draws, pushed apart so they gather in clumps (`share::halve`,
/// the one rule the decor and the caves hand theirs by), to one or none a
/// cell.
pub fn holds_cloud(cell: [i32; 2], mean: u32) -> bool {
    let at = |z: Zoom| [cell[0] >> (CELL_ZOOM - z), cell[1] >> (CELL_ZOOM - z)];
    let draw = node_seed(CUMULUS, (PATCH_ZOOM, at(PATCH_ZOOM)));
    let n = mean as i32 + (draw & 0x3f) as i32 - 32;
    let mut n = n.clamp(0, 256) as u32;
    let mut z = PATCH_ZOOM;
    while z < CELL_ZOOM && n > 0 {
        let h = node_seed(CUMULUS_SPLIT, (z, at(z)));
        // A weight of 2^16 by hash, pushed out from the middle so the halves
        // come uneven and the clouds gather.
        let w = |b: u32| {
            let x = ((h >> b) & 0xff) as i32 - 128;
            let far = 128 - (128 - x.abs()) * (128 - x.abs()) / 128;
            (32768 + x.signum() * far * 255) as u32
        };
        let room = 1u32 << (2 * (CELL_ZOOM - z - 1));
        let c = at(z + 1);
        let quarter = (c[0] & 1 | (c[1] & 1) << 1) as usize;
        let a = halve(n, [2 * room, 2 * room], w(0));
        let n2 = if quarter & 1 == 0 { a } else { n - a };
        let b = halve(n2, [room, room], w(16));
        n = if quarter & 2 == 0 { b } else { n2 - b };
        z += 1;
    }
    n > 0
}

#[cfg(test)]
#[path = "cloud.test.rs"]
mod test;
