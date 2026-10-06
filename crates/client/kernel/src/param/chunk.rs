// pxl-kernel/param/chunk.rs

//! How chunks are refined, held and built, and what each frame may start.

use pxl_spec::space::layer::lattice::TILE_CHUNKS;

// ---- temporal smoothness of the quality ----

/// One quality step: half a zoom of every split distance, so a step reads
/// as ground drawn a little farther out rather than a zoom jumping.
pub const TAU_STEP: f64 = core::f64::consts::SQRT_2;

/// Twelve half-zooms; coarser than that a view is its top zoom and saves nothing.
pub const TAU_STEPS: usize = 12;

/// How long a finer step must have had room before it is taken, so a pan
/// does not build and discard a zoom each time. Coarser is immediate.
pub const QUALITY_WAIT_MS: f64 = 1_000.0;

/// Share of each pool kept free before a finer step, so the next eye move
/// does not take it straight back.
pub const QUALITY_MARGIN: f64 = 0.05;

// ---- concurrency ----

/// Refinements under way at once; each holds the piece it replaces, so each
/// is a slot past the frontier the quality policy keeps free.
pub const REFINEMENTS_IN_FLIGHT: u32 = 32;

// ---- the frontier's time ----

/// How long the goal must ask for a coarser piece before it replaces the
/// finer ones, as for a finer quality step, so a turn that comes back finds
/// them still drawn.
pub const COARSEN_HOLD_MS: f64 = QUALITY_WAIT_MS;

// ---- device budget: memory ----

/// Bytes of meshes drawn at a leaf's cells or finer: about the flat-ground
/// disc within where the leaves end. A view needing more is drawn coarser.
pub const FINE_POOL_BYTES: u32 = 31 << 20;

/// Bytes of the coarser meshes. The goal over rolling ground is some 350 to
/// 760 pieces (measured 2026-09-28), plus refinements under way.
pub const COARSE_POOL_BYTES: u32 = 115 << 20;

/// Groups the coarser meshes are bound in, each one storage binding.
pub const ARENA_COARSE_GROUPS: u32 = 5;

/// Boxes a coarse chunk draws a cell, as a slot is sized for: the ground's,
/// the floor and skirt under it, what objects add. A larger mesh fills more
/// slots, never a coarser cube.
pub const COARSE_BOXES_PER_CELL: f64 = 4.44;

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

// ---- places' zooms ----

/// Every place where a logical pixel spans a z14 pixel or less, a zoom of
/// 256-pixel tiles: OpenMapTiles' `poi` layer holds every point POI from z14
/// (`etl_layer_points`), and Google and Naver hide businesses below about it.
pub const POI_ALL_ZOOM: i32 = 14;

/// From z12, only ranks up to `POI_MAJOR_RANK`: OpenMapTiles shows stations
/// and ferry terminals at z12–13 and nothing under z12.
pub const POI_MAJOR_ZOOM: i32 = 12;

/// OpenMapTiles' `poi_class_rank` for a railway station.
pub const POI_MAJOR_RANK: u16 = 40;
