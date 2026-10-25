// pxl-kernel/domain/layer/terrain/param.rs

//! The ways' pool: what it takes on each side, the GPU's buffer and the
//! CPU's mirror, and the scratch its squares are built in.

/// The most the ways' pool takes on each side, the GPU's buffer and the
/// CPU's mirror (domain/layer/terrain/mirror.rs), 64 MiB: the bake's bound,
/// `ways::tree::POOL_BLOCKS` blocks of 64 B, which every set fits wherever
/// the eye is (pxl-spec bake/tile/terrain.rs `pool_needs`), so at this no
/// square of a baked set is refused. Less refuses a dense view's last
/// squares their ways' trees, each left absent to the coarser window
/// (domain/layer/terrain/window.rs `REFUSED`); the mirror at it is a heap flow
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
///
/// A known gap: the GPU's pool at it binds within WebGPU's floor, asserted
/// (pxl-render gpu/renderer.rs `pool_bytes`), so no device's limits refuse
/// it, but a device out of memory may fail to make the doubled buffer, and
/// nothing hears it: the host keeps no error scope, so the renderer reads
/// on with a buffer the device holds as an error. The last doubling holds
/// the old pool beside the new, 32 MiB with 64, until the copy is submitted.
pub const WAYS_POOL_BYTES: u32 = 64 << 20;
const _: () = assert!(
    WAYS_POOL_BYTES <= pxl_spec_core::ways::tree::POOL_BLOCKS * 64 && WAYS_POOL_BYTES >= 2 * 64,
    "the pool's cap is no more than the bake's bound, nor less than a block past block 0"
);

/// The ways' pool on the GPU at first, 1 MiB, 16 Ki blocks: a square with
/// no ways holds its root block alone, so the 960 squares' roots and some
/// ways. It doubles each time a build runs out (pxl-render
/// gpu/renderer.rs `GroundBuffers::grow`), the square that ran out filled
/// again a turn later, so a dense view reaches 64 MiB in six. Not yet
/// measured against a dense view.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = assert!(WAYS_POOL_FIRST_BYTES <= WAYS_POOL_BYTES && WAYS_POOL_FIRST_BYTES >= 2 * 64);

/// A square's ways' records' room in each of its reads at first, 16 KiB,
/// 240 blocks: a square with no ways reads 72 B. Every read's room doubles
/// past a square's records that outgrew it, that square filled again
/// (pxl-render gpu/renderer.rs `GroundBuffers::room`), up to a square's
/// most, `ways::tree::RECORDS_BYTES`: the GPU's read buffers and their
/// rooms on the heap together, `window::READS` of each.
pub const WAYS_RECORDS_FIRST_BYTES: u32 = 16 << 10;

/// The most the handed squares' scratch takes on the GPU, 15.54 MiB:
/// `window::HANDED` squares' each for a tree of `ways::tree::SQUARE_BLOCKS`,
/// the bake's bound on a square's, so at this no square of a baked set is
/// refused for it (pxl-render gpu/renderer.rs `GroundBuffers::grow_scratch`).
/// Less refuses a dense square its ways' tree, left absent to the coarser
/// window (domain/layer/terrain/window.rs `REFUSED`), as the pool's cap does. The
/// GPU's alone: its records are copied out in the frame they are built,
/// into each read's room (`WAYS_RECORDS_FIRST_BYTES`).
///
/// The pool's known gap holds for it too (`WAYS_POOL_BYTES`): a device
/// out of memory may fail to make the larger scratch, and nothing hears
/// it, the host keeping no error scope, so the renderer builds on in a
/// buffer the device holds as an error. Its last growth holds the old
/// scratch beside the new, 11.33 MiB with 15.54, until the old is let go.
pub const WAYS_SCRATCH_BYTES: u32 = pxl_spec_core::window::HANDED * pxl_spec_core::ways::tree::SCRATCH_WORDS * 4;

/// The handed squares' scratch at first, 1 MiB, 539 blocks a square: its
/// runs, 699,012 B for the nine, are a square's most whatever it holds,
/// and the rest a tree of 539 blocks, more than a read's first room holds
/// records of (240). Each square's tree room doubles each time a build
/// outgrows it, its runs fixed, the square filled again a turn later, so
/// the scratch goes 1.00, 1.33, 2.00, 3.33, 6.00, 11.33 and 15.54 MiB, a
/// square at the bake's bound reaching the cap in six. Not yet measured
/// against a dense view; the pool's known gap holds for each growth
/// (`WAYS_SCRATCH_BYTES`).
pub const WAYS_SCRATCH_FIRST_BYTES: u32 = 1 << 20;
const _: () = {
    use pxl_spec_core::{ways::tree, window::HANDED};
    let least = HANDED * tree::scratch_words(1) * 4;
    assert!(
        WAYS_SCRATCH_BYTES <= HANDED * tree::SCRATCH_WORDS * 4 && WAYS_SCRATCH_FIRST_BYTES <= WAYS_SCRATCH_BYTES && WAYS_SCRATCH_FIRST_BYTES >= least,
        "the scratch's cap is no more than a square's most, its first no more than its cap, nor less than a block a square"
    );
};
