// pxl-spec-core/cloud.rs

//! gpu-portable: the cloud field the cumulus and the sheets share. The
//! lattice it lies on, the wind that carries it and the hash its cells draw
//! from: the cumulus draws its cells from the hash on the CPU (pxl-kernel
//! domain/scene/environment/cloud.rs), and the sheets sum the hash's noise on the
//! GPU (pxl-render frame/shader/noise.wgsl), whose `lattice_hash` pxl-render
//! build.rs writes from `HASH_IN`, `HASH_ROUNDS`, `HASH_LAST` and `HASH_KEPT`.

use crate::UNIT_ZOOM;

/// A lattice unit is 2^12 cells, about 8 km at mid latitudes; a power of two
/// so the clouds wrap with the map.
pub const LATTICE_SHAMT: u32 = 12;
/// The lattice's units around the world, X's 2^24 units over a lattice unit's.
pub const LATTICE_COUNT: u32 = 1 << (UNIT_ZOOM - LATTICE_SHAMT);

/// In cells a second, about 50 m/s at mid latitudes, a jet stream's: at a
/// breeze's 8 a cloud a kilometer wide takes minutes to cross its width and
/// reads as still. The field moves against it, so a point on the ground sees
/// the field at itself plus the drift.
pub const WIND: [f64; 2] = [24.0, 10.0];

/// A fair-weather cumulus base.
pub const CUMULUS_KM: f64 = 1.5;

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
pub const HASH_KEPT: u32 = 24;

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
    (h >> (32 - HASH_KEPT)) as f32 / (1u32 << HASH_KEPT) as f32
}
