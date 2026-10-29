// pxl-kernel/domain/scene/param.rs

//! What is drawn and how it is lit, and what the stage hands
//! the GPU of the ways: the pool's first size, the records' first room and
//! the scratch, each growing to the bake's bound (pxl-spec-core
//! way/tree.rs `POOL_FAMILIES`, `BRICK_FAMILIES`).

// ---- lighting ----

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all, on the shader's linear falloff: how much a rim brightens a face at
/// its edge, as a share of its own light — the day's rim of 0.7 of the sky's
/// light, the sky a fifth of a day's light on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// Edge shading along a cube's creases, convex and concave: its darkening,
/// and its width in units, one line alike on cells of every size and
/// thinned by perspective alone: about a CSS pixel at the default orbit's
/// focus.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 1.0 / 64.0;

/// Corner occlusion's darkening at its deepest, written where the setting
/// asks for it; the shader cannot see a setting.
pub const AO_STRENGTH: f64 = 0.2;

// ---- the ways, as the stage hands them to the GPU ----

/// The ways' pool on the GPU at first, 1 MiB, 16 Ki families: a brick with
/// no ways holds none, and one with ways a family from its own nodes along
/// z down (pxl-spec-core `way::tree`), so the ways of a sparse view. It
/// doubles each time a build runs out (pxl-render gpu/ray/ground.rs
/// `GroundBuffers::grow`), the brick that ran out filled again a turn later, so a dense view reaches 64 MiB in six. Not yet
/// measured against a dense view.
pub const WAYS_POOL_FIRST_BYTES: u32 = 1 << 20;
const _: () = {
    use pxl_spec_core::way::tree::{FAMILY_WORDS, POOL_FAMILIES};
    assert!(
        WAYS_POOL_FIRST_BYTES <= POOL_FAMILIES * FAMILY_WORDS * 4
            && WAYS_POOL_FIRST_BYTES >= 2 * FAMILY_WORDS * 4,
        "the pool's first is no more than its most, nor less than a family past family 0"
    );
};

/// A brick's ways' records' room in each of its reads at first, 16 KiB,
/// 240 families': a brick with no ways reads its count alone, 4 B. Every
/// read's room doubles past a brick's records that outgrew it, that brick
/// filled again (pxl-render gpu/ray/ground.rs `GroundBuffers::room`), up to
/// a brick's most, `way::tree::RECORDS_BYTES`: the GPU's read buffers and
/// their rooms on the heap together, each pxl-render gpu/renderer.rs
/// `READS` of them.
pub const WAYS_RECORDS_FIRST_BYTES: u32 = 16 << 10;

/// The handed bricks' scratch at first, 4 MiB, 1390 families a brick:
/// its columns' spans, 3.14 MiB for the nine, are a brick's most whatever
/// it holds, and the rest a tree of 1390 families, more than a read's first
/// room holds records of (240). Each brick's tree room doubles each time a
/// build outgrows it, its spans fixed, the brick filled again a turn
/// later, so the scratch goes 4.00, 4.86, 6.58, 10.01, 16.88 and 18.01 MiB,
/// a brick at the bake's bound reaching its most in five. Not yet measured
/// against a dense view; the pool's known gap holds for each growth
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::grow`).
pub const WAYS_SCRATCH_FIRST_BYTES: u32 = 4 << 20;
const _: () = {
    use pxl_spec_core::{way::tree, window::HANDED};
    let least = HANDED * tree::scratch_words(1) * 4;
    assert!(
        WAYS_SCRATCH_FIRST_BYTES <= HANDED * tree::SCRATCH_WORDS * 4
            && WAYS_SCRATCH_FIRST_BYTES >= least,
        "the scratch's first is no more than a brick's most, nor less than a family a brick"
    );
};
