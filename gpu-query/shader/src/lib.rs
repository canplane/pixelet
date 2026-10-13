//! One compute entry that asks the caves per node, and one that hands back a
//! node's seed, from pxl-spec's gpu-portable source taken as it is
//! (crates/spec/src/procedural/portable.rs) over xpute-kit's
//! (xpute/rust/kit/src/math/rng.gpu.rs). Only what the portable module asks
//! of its parent is given here.

#![cfg_attr(target_arch = "spirv", no_std)]

#[path = "../../../xpute/rust/kit/src/math/rng.gpu.rs"]
pub mod rng_gpu;

/// What `portable` takes from its parent in pxl-spec (`procedural.rs`).
use rng_gpu::derive_seed_u32;
pub type Zoom = u32;
/// `GROUND.cell()`; a value pxl-spec's own differs from shows in the diff.
const LEAF_CELL: Zoom = 24;

#[path = "../../../crates/spec/src/procedural/portable.rs"]
pub mod portable;

use portable::Cave;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

/// The nodes asked: `zoom`, and the first node's index on each axis.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ask {
    pub zoom: u32,
    pub first: [i32; 3],
    pub across: [u32; 3],
}

/// Node `id` of `ask`.
pub fn node(ask: &Ask, id: [u32; 3]) -> [i32; 3] {
    [ask.first[0] + id[0] as i32, ask.first[1] + id[1] as i32, ask.first[2] + id[2] as i32]
}

/// Node `id` of `ask` answered: bit 0 hollow, bit 1 whole known, bit 2 whole's value.
pub fn answer(ask: &Ask, id: [u32; 3]) -> u32 {
    let cave = Cave::of(ask.zoom, node(ask, id));
    let (known, value) = cave.whole(ask.zoom);
    u32::from(cave.hollow()) | u32::from(known) << 1 | u32::from(value) << 2
}

fn index(ask: &Ask, id: UVec3) -> Option<usize> {
    (id.x < ask.across[0] && id.y < ask.across[1] && id.z < ask.across[2]).then(|| (id.x + ask.across[0] * (id.y + ask.across[1] * id.z)) as usize)
}

#[spirv(compute(threads(4, 4, 4)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    if let Some(i) = index(ask, id) {
        out[i] = answer(ask, [id.x, id.y, id.z]);
    }
}

#[spirv(compute(threads(4, 4, 4)))]
pub fn seed_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] ask: &Ask,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
) {
    if let Some(i) = index(ask, id) {
        out[i] = portable::node_seed(0xca7e_0001, (ask.zoom, node(ask, [id.x, id.y, id.z])));
    }
}
