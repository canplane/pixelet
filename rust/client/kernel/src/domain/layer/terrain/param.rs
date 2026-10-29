// pxl-kernel/domain/layer/terrain/param.rs

//! The ways' pool's cap, which the mirror reads.

use pxl_spec_core::way::tree::{FAMILY_WORDS, POOL_FAMILIES};

/// The most the ways' pool takes on each side, the GPU's buffer and the
/// CPU's mirror (domain/layer/terrain/mirror.rs), 64 MiB: the bake's bound,
/// `way::tree::POOL_FAMILIES` families of 64 B, which every set fits wherever
/// the eye is (pxl-spec bake/tile/terrain.rs `pool_needs`), so at this no
/// square of a baked set is refused. A cap below it would refuse a dense
/// view's last squares their ways' trees, each left absent to the coarser window
/// (domain/layer/terrain/window.rs `REFUSED`); the mirror at it is a heap flow
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
///
/// A known gap: the GPU's pool at it binds within WebGPU's floor, asserted
/// (pxl-render gpu/ray/ground.rs `pool_bytes`), so no device's limits refuse
/// it, but a device out of memory may fail to make the doubled buffer, and
/// nothing hears it: the host keeps no error scope, so the renderer reads
/// on with a buffer the device holds as an error. The last doubling holds
/// the old pool beside the new, 32 MiB with 64, until the copy is submitted.
pub const WAYS_POOL_BYTES: u32 = POOL_FAMILIES * FAMILY_WORDS * 4;
const _: () = assert!(
    WAYS_POOL_BYTES >= 2 * FAMILY_WORDS * 4,
    "the pool's cap holds a family past family 0"
);
