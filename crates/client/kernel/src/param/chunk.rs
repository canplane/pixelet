// pxl-kernel/param/chunk.rs

//! How chunks are refined, held and built, and what each frame may start.

use pxl_spec::space::layer::lattice::TILE_CHUNKS;

// ---- device budget: memory ----

/// Bytes of meshes drawn at a leaf's cells or finer: about the flat-ground
/// disc within where the leaves end. A view needing more is drawn coarser.
pub const FINE_POOL_BYTES: u32 = 31 << 20;

/// Bytes of the coarser meshes. The goal over rolling ground is some 350 to
/// 760 pieces (measured 2026-09-28), plus refinements under way.
pub const COARSE_POOL_BYTES: u32 = 115 << 20;

/// Groups the coarser meshes are bound in, each one storage binding.
pub const ARENA_COARSE_GROUPS: u32 = 5;

// ---- decor's tiers ----

/// A decor kind is drawn as a model while it covers at least this many pixels
/// at the reference view: by eye, where Google's maps stop drawing buildings
/// in 3D.
pub const DECOR_MODEL_PX: f64 = 12.0;

/// Under this many pixels a decor kind's sprite is not drawn: set by eye,
/// below it an instance is pixel noise.
pub const DECOR_SPRITE_PX: f64 = 3.0;

// ---- per-frame work caps ----

/// A tile's chunks, so a tile of flat ground starts building in the frame it arrives.
pub const STARTS_PER_FRAME: u32 = TILE_CHUNKS * TILE_CHUNKS;

pub const COMMITS_PER_FRAME: u32 = 8;

// ---- a bake's slices ----

/// Boxes the mesher makes or emits a step, so one chunk never takes a
/// turn's budget whole.
pub const BAKE_BOXES_PER_STEP: u32 = 512;
