//! One compute entry that asks the caves per node: the cube at `zoom`,
//! `[x, y, z]`, its value and whether it is whole.
//!
//! With the `spec` feature it calls pxl-spec as it is, which stops at `std`
//! (wall-std.txt). Without it, `probe` holds the caves' path copied verbatim
//! from pxl-spec procedural.rs and xpute-kit math/rng.rs, only so the
//! compiler can say what stops it past `std`. It is not a second
//! implementation; nothing outside this scratch crate reads it.

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(not(feature = "spec"))]
use probe::Cave;
#[cfg(feature = "spec")]
use pxl_spec::procedural::Cave;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

#[cfg(not(feature = "spec"))]
pub mod probe {
    // Probe edit: a zoom is a u8 in pxl-spec; Naga takes no 1-byte type
    // (wall-width1.txt).
    type Zoom = u32;
    type Node<const D: usize> = (Zoom, [i64; D]);

    // xpute-kit math/rng.rs, verbatim.
    const SEED_SALT: u64 = 0x9e3779b97f4a7c15;
    pub fn mix64(x: u64) -> u64 {
        let x = x.wrapping_add(SEED_SALT);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn derive_seed_u64(base: u64, tag: u64) -> u64 {
        mix64(base ^ tag)
    }
    pub fn u32_to_unit(x: u32) -> f64 {
        x as f64 / 4_294_967_296.0
    }

    // pxl-spec procedural.rs, verbatim; GROUND.cell() is 24.
    const LEAF_CELL: Zoom = 24;
    const MASTER_SEED: u64 = 0xd0c0_dec0_5eed_0001;
    pub fn node_seed<const D: usize>(tag: u64, (zoom, at): Node<D>) -> u64 {
        let seed = derive_seed_u64(MASTER_SEED, tag);
        // Probe edit: `at.iter()` stops rust-gpu (wall-array-iter-rule.txt).
        let mut h = derive_seed_u64(seed, u64::from(zoom));
        let mut k = 0;
        while k < D {
            h = derive_seed_u64(h, at[k] as u64);
            k += 1;
        }
        h
    }
    pub const CAVES: u64 = 0xca7e_0001;
    pub const CAVE_COARSEST: Zoom = LEAF_CELL - 5;
    const CAVE_TURN: f64 = 1.0 / 4.0;
    fn draws(z: Zoom, c: [i64; 3]) -> (Option<u64>, u32) {
        let h = node_seed(CAVES, (z, c));
        // Probe edit: Metal has no f64. x / 2^32 is exact in f64, so
        // `u32_to_unit(x) < 1/4` is exactly `x < 2^30`.
        let _ = (u32_to_unit, CAVE_TURN);
        (((h as u32) < 1 << 30).then_some((h >> 32) & 7), ((h >> 40) & 0xff) as u32)
    }
    fn slot(c: [i64; 3]) -> u64 {
        (c[0] & 1 | (c[1] & 1) << 1 | (c[2] & 1) << 2) as u64
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Cave {
        hollow: bool,
        live: bool,
    }

    impl Cave {
        pub const ROOT: Cave = Cave { hollow: false, live: true };

        pub fn children(self, zoom: Zoom, c: [i64; 3]) -> [Cave; 8] {
            if zoom < CAVE_COARSEST - 1 {
                return [Cave::ROOT; 8];
            }
            if !self.live || zoom >= LEAF_CELL {
                return [Cave { live: false, ..self }; 8];
            }
            let (turn, live) = draws(zoom, c);
            // Probe edit: `core::array::from_fn` stops rust-gpu ("memset on
            // structs not implemented yet", wall-from-fn.txt); spelled out.
            let kid = |k: u64| Cave {
                hollow: self.hollow ^ (turn == Some(k)),
                live: live >> k & 1 == 1,
            };
            [kid(0), kid(1), kid(2), kid(3), kid(4), kid(5), kid(6), kid(7)]
        }

        pub fn child(self, zoom: Zoom, c: [i64; 3]) -> Cave {
            // Probe edit: `[T; N]::map` stops rust-gpu (wall-array-map-rule.txt).
            self.children(zoom - 1, [c[0] >> 1, c[1] >> 1, c[2] >> 1])[slot(c) as usize]
        }

        pub fn of(zoom: Zoom, c: [i64; 3]) -> Cave {
            let mut cave = Cave::ROOT;
            for z in CAVE_COARSEST..=zoom.min(LEAF_CELL) {
                let s = zoom - z;
                cave = cave.child(z, [c[0] >> s, c[1] >> s, c[2] >> s]);
                if !cave.live {
                    break;
                }
            }
            cave
        }

        pub fn hollow(self) -> bool {
            self.hollow
        }

        // Probe edit: `Option<bool>` is a u8 to rust-gpu (wall-option-bool-rule.txt);
        // bit 0 known, bit 1 its value.
        pub fn whole(self, zoom: Zoom) -> u32 {
            let known = zoom >= CAVE_COARSEST && (!self.live || zoom >= LEAF_CELL);
            u32::from(known) | u32::from(known && self.hollow) << 1
        }
    }
}

/// The nodes asked: `zoom`, and the first node's index on each axis.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ask {
    pub zoom: u32,
    pub first: [i32; 3],
    pub across: [u32; 3],
}

/// Node `id` of `ask` answered: bit 0 hollow, bit 1 whole known, bit 2 whole's value.
pub fn answer(ask: &Ask, id: [u32; 3]) -> u32 {
    // `[T; N]::map` stops rust-gpu (wall-array-map.txt).
    let c = [i64::from(ask.first[0]) + i64::from(id[0]), i64::from(ask.first[1]) + i64::from(id[1]), i64::from(ask.first[2]) + i64::from(id[2])];
    let zoom = ask.zoom;
    let cave = Cave::of(zoom, c);
    u32::from(cave.hollow()) | cave.whole(zoom) << 1
}

#[spirv(compute(threads(4, 4, 4)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    // A storage buffer: a uniform's arrays need a 16-byte stride (wall-uniform-layout.txt).
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    let i = id.x + ask.across[0] * (id.y + ask.across[1] * id.z);
    if id.x < ask.across[0] && id.y < ask.across[1] && id.z < ask.across[2] {
        out[i as usize] = answer(ask, [id.x, id.y, id.z]);
    }
}

/// Node `id`'s caves hash, low and high words: to tell the u64 arithmetic
/// from the caves' logic.
#[cfg(not(feature = "spec"))]
pub fn seed(ask: &Ask, id: [u32; 3]) -> [u32; 2] {
    let c = [i64::from(ask.first[0]) + i64::from(id[0]), i64::from(ask.first[1]) + i64::from(id[1]), i64::from(ask.first[2]) + i64::from(id[2])];
    let h = probe::node_seed(probe::CAVES, (ask.zoom, c));
    [h as u32, (h >> 32) as u32]
}

#[cfg(not(feature = "spec"))]
#[spirv(compute(threads(4, 4, 4)))]
pub fn seed_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    let i = id.x + ask.across[0] * (id.y + ask.across[1] * id.z);
    if id.x < ask.across[0] && id.y < ask.across[1] && id.z < ask.across[2] {
        let [lo, hi] = seed(ask, [id.x, id.y, id.z]);
        out[2 * i as usize] = lo;
        out[2 * i as usize + 1] = hi;
    }
}
