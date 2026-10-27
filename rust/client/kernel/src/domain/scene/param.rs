// pxl-kernel/domain/scene/param.rs

//! What is drawn and how it is lit, and what the stage hands
//! the GPU of the ways: the pool's first size, the records' first room and
//! the scratch. The pool's cap is the terrain's, which the mirror reads.

// ---- lighting ----

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all, on the shader's linear falloff: how much a rim brightens a face at
/// its edge, as a share of its own light — the day's rim of 0.7 of the sky's
/// light, the sky a fifth of a day's light on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// Edge shading along a cube's creases, convex and concave: its darkening,
/// and its width in units, one line alike on blocks of every size and
/// thinned by perspective alone: about a CSS pixel at the default orbit's
/// focus.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 1.0 / 64.0;

/// Corner occlusion's darkening at its deepest, written where the setting
/// asks for it; the shader cannot see a setting.
pub const AO_STRENGTH: f64 = 0.2;

// ---- the ways, as the stage hands them to the GPU ----

/// The ways' pool on the GPU at first, 1 MiB, 16 Ki families: a square with
/// no ways holds none, and one with ways a family from its own nodes along
/// z down (pxl-spec-core `way::tree`), so the ways of a sparse view. It
/// doubles each time a build runs out (pxl-render gpu/ray/ground.rs
/// `GroundBuffers::grow`), the square that ran out filled again a turn later, so a dense view reaches 64 MiB in six. Not yet
/// measured against a dense view.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = assert!(
    WAYS_POOL_FIRST_BYTES <= crate::domain::layer::terrain::param::WAYS_POOL_BYTES
        && WAYS_POOL_FIRST_BYTES >= 2 * 64
);

/// A square's ways' records' room in each of its reads at first, 16 KiB,
/// 240 families': a square with no ways reads its count alone, 4 B. Every
/// read's room doubles past a square's records that outgrew it, that square
/// filled again (pxl-render gpu/ray/ground.rs `GroundBuffers::room`), up to
/// a square's most, `way::tree::RECORDS_BYTES`: the GPU's read buffers and
/// their rooms on the heap together, each pxl-render gpu/renderer.rs
/// `READS` of them.
pub const WAYS_RECORDS_FIRST_BYTES: u32 = 16 << 10;

/// The most the handed squares' scratch takes on the GPU, 18.01 MiB:
/// `window::HANDED` squares' each for a tree of `way::tree::SQUARE_FAMILIES`,
/// the bake's bound on a square's, so at this no square of a baked set is
/// refused for it (pxl-render gpu/ray/ground.rs `GroundBuffers::grow_scratch`).
/// Less refuses a dense square its ways' tree, left absent to the coarser
/// window (domain/layer/terrain/window.rs `REFUSED`), as the pool's cap does. The
/// GPU's alone: its records are copied out in the frame they are built,
/// into each read's room (`WAYS_RECORDS_FIRST_BYTES`).
///
/// The pool's known gap holds for it too (domain/layer/terrain/param.rs
/// `WAYS_POOL_BYTES`): a device
/// out of memory may fail to make the larger scratch, and nothing hears
/// it, the host keeping no error scope, so the renderer builds on in a
/// buffer the device holds as an error. Its last growth holds the old
/// scratch beside the new, 16.88 MiB with 18.01, until the old is let go.
pub const WAYS_SCRATCH_BYTES: u32 =
    pxl_spec_core::window::HANDED * pxl_spec_core::way::tree::SCRATCH_WORDS * 4;

/// The handed squares' scratch at first, 4 MiB, 1390 families a square:
/// its columns' spans, 3.14 MiB for the nine, are a square's most whatever
/// it holds, and the rest a tree of 1390 families, more than a read's first
/// room holds records of (240). Each square's tree room doubles each time a
/// build outgrows it, its spans fixed, the square filled again a turn
/// later, so the scratch goes 4.00, 4.86, 6.58, 10.01, 16.88 and 18.01 MiB,
/// a square at the bake's bound reaching the cap in five. Not yet measured
/// against a dense view; the pool's known gap holds for each growth
/// (`WAYS_SCRATCH_BYTES`).
pub const WAYS_SCRATCH_FIRST_BYTES: u32 = 4 << 20;
const _: () = {
    use pxl_spec_core::{way::tree, window::HANDED};
    let least = HANDED * tree::scratch_words(1) * 4;
    assert!(
        WAYS_SCRATCH_BYTES <= HANDED * tree::SCRATCH_WORDS * 4 && WAYS_SCRATCH_FIRST_BYTES <= WAYS_SCRATCH_BYTES && WAYS_SCRATCH_FIRST_BYTES >= least,
        "the scratch's cap is no more than a square's most, its first no more than its cap, nor less than a family a square"
    );
};
