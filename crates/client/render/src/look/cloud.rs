// pxl-render/look/cloud.rs

//! What the cloud layers share: the lattice the field lies on, the wind that
//! carries it, and the hash the cells draw from, the shaders' own
//! (`frame/shader/noise.wgsl`). The sheets' field is the GPU's
//! (`frame/shader/cloud_bake.wgsl`); the cumulus (`pxl-kernel
//! scene/environment/cloud.rs`) draws its cells from the hash alone.

use pxl_spec::space::coord::world::EXTENT;

/// A lattice unit is 2^12 cells, about 8 km at mid latitudes; a power of two
/// so the clouds wrap with the map.
pub const LATTICE_SHAMT: u32 = 12;
pub const LATTICE_COUNT: u32 = 1 << (EXTENT[0] - LATTICE_SHAMT);
const _: () = assert!(EXTENT[0] == EXTENT[1], "one lattice count for both axes");
const _: () = assert!(LATTICE_COUNT.is_multiple_of(TILE_UNITS), "the world is whole tiles");

/// The sheets' tile, one fetch a pixel and a mip chain in place of eight
/// octaves of hashes: 32 lattice units is about 250 km.
pub const TILE_UNITS: u32 = 32;
pub const TILE_TEXELS: u32 = 1024;

/// In cells a second, about 50 m/s at mid latitudes, a jet stream's: at a
/// breeze's 8 a cloud a kilometer wide takes minutes to cross its width and
/// reads as still. The field moves against it, so a point on the ground sees
/// the field at itself plus the drift.
pub const WIND: [f64; 2] = [24.0, 10.0];

/// A fair-weather cumulus base.
pub const CUMULUS_KM: f64 = 1.5;

/// Where the clouds are gone, faded from half of it: a product number.
const GONE_KM: f64 = 64.0;

/// How much of the clouds shows from `altitude_km` up: all of them to half
/// of `GONE_KM`, none from it.
pub fn shown(altitude_km: f64) -> f64 {
    let t = ((altitude_km - GONE_KM / 2.0) / (GONE_KM / 2.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Cell `[x, y]`'s hash under `salt`, in [0, 1): noise.wgsl's `lattice_hash`,
/// held equal by look/cloud.test.rs.
pub fn lattice_hash([x, y]: [u32; 2], salt: u32) -> f64 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    f64::from(h >> 8) / 16_777_216.0
}

/// How far the field has moved under the map `time` seconds in, in cells.
pub fn drift(time: f64) -> [f64; 2] {
    WIND.map(|w| w * time)
}

#[cfg(test)]
#[path = "cloud.test.rs"]
mod test;
