// pxl-kernel/param.rs

//! The kernel's knobs: every tuned number nothing derives, with its reason,
//! grouped by the policy it tunes. None is a correctness bound: set to its
//! least, the view still converges, only coarser or later.

use pxl_spec::space::pyramid::PAGE_CHUNKS;

// ---- product quality ----

/// Logical pixels one observation cell spans at the optical axis. Six is
/// the reference view `SPLIT` was stated at (1080 tall at 50°); a screen
/// asking finer is held at `SPLIT`.
pub const CENTER_CELL_LOGICAL_PX: f64 = 6.0;

// ---- the view's reach (layer/environment/fog.rs) ----

/// The most doublings of distance a view draws past the ground under the
/// eye, or z16's split distance where that is nearer: its far edge, where
/// the fog closes. A level a doubling, so a view draws at most this many
/// levels and one whatever its height — fewer where it looks across and the
/// first ground it sees lies past what is under it — and what it asks of the
/// arena is held by this rather than by how far the planet lets it see. On
/// the ground the horizon is usually the nearer end; five puts the edge at
/// about 12 km there and about 32 from a kilometre up. A product number, not
/// a likeness of the air: the fog only hides where drawing stops.
pub const VIEW_LEVELS: u32 = 5;

// ---- what refinement observes (world/observer.rs) ----

/// Most doublings of distance drawn past the ground under the eye: the fog's
/// far edge, about 12 km on the ground and 32 km from 1 km up. Bounds the
/// levels a view draws whatever the horizon allows.
pub const VIEW_LEVELS: u32 = 5;

// ---- what refinement observes ----

/// The estimator's tile side, in radians (about 10°, 77 rays on a 16:9
/// screen at 50°). An eighth cast 135 rays for goals within 1% of these
/// over rolling ground (measured 2026-09-28).
pub const OBSERVED_TILE: f64 = 1.0 / 6.0;

/// How far a tile's last answer may drift, as a share of a tile, before it
/// is cast again.
pub const REUSE_SHIFT: f64 = 0.25;

// ---- temporal smoothness of the quality ----

/// One quality step: half a level of every split distance, so a step reads
/// as ground drawn a little farther out rather than a level jumping.
pub const TAU_STEP: f64 = core::f64::consts::SQRT_2;

/// Twelve half-levels; coarser than that a view is its top level and saves nothing.
pub const TAU_STEPS: usize = 12;

/// How long a finer step must have had room before it is taken, so a pan
/// does not build and discard a level each time. Coarser is immediate.
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

/// Slots in one arena group, and so z16 chunks drawn at their own cells at
/// once: about the flat-ground disc within where z16 ends. A view needing
/// more is drawn coarser.
pub const ARENA_GROUP_SLOTS: u32 = 330;

/// Groups shared by the coarse tiers. The goal over rolling ground is some
/// 350 to 760 pieces (measured 2026-09-28), plus refinements under way.
pub const ARENA_COARSE_GROUPS: u32 = 5;

/// Boxes a coarse slot holds. A larger mesh fills more slots, never a
/// coarser cube; the coarsest fills one (asserted in mesh_slot.rs).
pub const COARSE_BOXES: u32 = 4551;



// ---- decor's tiers ----

/// How small, in the tier's cells on the screen, a decor kind may get and
/// still be drawn, as its variants' sprites (layer/decor/build.rs
/// `decor_tier`): past the model's floor of two cells, down to half of one.
/// A knob set by eye and derived from nothing: a sprite costs two triangles
/// an instance, so how far decor reaches is what this buys, and below half a
/// cell an instance is a pixel or two that reads as noise.
pub const IMPOSTOR_GONE_CELLS: f64 = 0.5;

/// Changes the world's change log keeps (kernel.rs): what a reader that fell
/// behind can still catch up from before it has to read everything again.
pub const JOURNAL_CAPACITY: u32 = 4096;



// ---- per-frame work caps (world/chunk/table.rs, io/policy.rs) ----

/// A page's chunks, so a page of flat ground starts building in the frame it arrives.
pub const STARTS_PER_FRAME: u32 = PAGE_CHUNKS * PAGE_CHUNKS;

pub const COMMITS_PER_FRAME: u32 = 8;


/// The most attempts one pump starts (io/policy.rs). The credits already
/// bound what is out; this bounds only how fast they are taken up.
pub const INTAKE: u32 = u32::MAX;

// ---- latency hiding ----

/// Most speculative pages a layer asks for at once.
pub const HINT_ALLOWANCE: u32 = 128;

// ---- I/O robustness ----

pub const RETRY_MS: f64 = 2_000.0;

/// Most fetches out at once, whatever the host grants.
pub const MAX_CREDITS: u32 = 16;

// ---- the turn's budget ----

/// Share of the frame left unspent for the host's own work; a share, so a
/// fast display does not lose most of its frame to it.
pub const QUANTUM_MARGIN_SHARE: f64 = 0.15;

/// Frames a non-interactive turn may take: with nothing moving, two vsyncs
/// only show the same image twice.
pub const QUANTUM_BATCH_FRAMES: f64 = 2.0;

/// How long after the last input a turn is still interactive, so a pause
/// mid-drag does not count.
pub const QUANTUM_SETTLE_MS: f64 = 150.0;

/// How far a pass may overrun, so a step that reports nothing done cannot walk the whole list.
pub const HARD_SLACK_MS: f64 = 4.0;

/// The goal walk's share of the visible budget; half until measured.
pub const VIEW_SHARE: f64 = 0.5;

/// Share of the budget held back from the visible phase for content, which
/// would otherwise starve while a load keeps the visible phase busy.
pub const CONTENT_SHARE: f64 = 0.2;

// ---- the drawn resolution ----

pub const RESOLUTION_STEP: f64 = 0.25;

/// Over budget this long before a step down, so one slow frame is not taken for load.
pub const RESOLUTION_OVER_HOLD_MS: f64 = 500.0;

pub const RESOLUTION_ROOM_HOLD_MS: f64 = 2000.0;

/// A step up adds at most (2.0 / 1.75)² ≈ 1.3× fill, so half the budget leaves room.
pub const RESOLUTION_ROOM: f64 = 0.5;

/// Interval in budgets that counts as a missed frame when the device reports no GPU time.
pub const RESOLUTION_MISSED: f64 = 1.5;

/// First probe up, and the longest backoff between probes, when the device
/// reports no GPU time.
pub const RESOLUTION_PROBE_HOLD_MS: f64 = 5000.0;
pub const RESOLUTION_PROBE_HOLD_MAX_MS: f64 = 60_000.0;

pub const RESOLUTION_PROBE_TRUST_MS: f64 = 2000.0;

/// The tightest budget: 60 Hz. Faster displays would trade sharpness for
/// frames the eye gains less from.
pub const RESOLUTION_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// A ratio change rebuilds the render targets, so changes are spaced out.
pub const RESOLUTION_CHANGE_GAP_MS: f64 = 1000.0;

// ---- the companion's figures ----

/// How many cells a walker steps up onto, from the cell it walks into. What
/// fills more than this beside it is a wall, and the step is not taken.
///
/// Four, set by the product owner by choice (2026-10-02), derived from
/// nothing: no precedent or measurement fixes it. It is not Minecraft's —
/// a walk there steps 0.6 of a block, and a jump makes a whole one.
pub const STEP_CELLS: i64 = 4;

// ---- places' zooms (layer/poi.rs, layer/poi/placement.rs) ----

/// The coarsest level every place is shown at: z14 (level k is z(16 − k)).
/// OpenMapTiles' `poi` layer holds every point POI from z14 and no general
/// one under it (layers/poi/poi.yaml, `etl_layer_points`), and Google's and
/// Naver's maps leave businesses off below about the same zoom.
pub const POI_ALL_LEVEL: u32 = 2;

/// The coarsest level a place is shown at at all, and only the most
/// prominent — a rank no more than `POI_MAJOR_RANK`: z12. OpenMapTiles
/// shows railway stations, halts and ferry terminals at z12 and z13 and
/// nothing else of the layer; under z12 there is none, and nothing is asked
/// for.
pub const POI_MAJOR_LEVEL: u32 = 4;

/// The rank a place needs to be shown between `POI_ALL_LEVEL` and
/// `POI_MAJOR_LEVEL`: a railway station's, OpenMapTiles' `poi_class_rank` 40
/// (layer/poi.rs `rank_of`).
pub const POI_MAJOR_RANK: u16 = 40;
