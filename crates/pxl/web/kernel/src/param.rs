// pxl-kernel/param.rs

//! The kernel's knobs: every number the view is tuned by that nothing
//! derives, each named here once with why it is what it is, grouped by the
//! policy whose degree of freedom it is, and handed to whatever uses it — so
//! tuning one moves no concern and no code, and no mechanism writes one of
//! its own. A number that follows from others — the fault bound's pages a
//! level, the frames residency needs, a slot's words, a pool's slots, the
//! arena's bytes, the workspace's — is not here but beside what it is
//! derived from, with its formula.
//!
//! None of them is a correctness bound. Residency's capacity is asserted
//! from the structure (world/paging.rs); the arena is never overrun and its
//! being full is never a hole, whatever it is sized at (world/chunk/
//! frontier.rs); and every value here can be set to its least and the view
//! still converges, only coarser or later.
//!
//! Compile-time throughout: some size a `.fixed` range or an arena group,
//! which is laid out before anything runs.
//!
//! Knobs defined outside the kernel, because what is outside shares them,
//! and tuned there:
//!
//! - pxl-spec space/lod.rs `SPLIT`: the criterion, in units of a cell's side,
//!   and so the finest cell angle the system is sized for, `1 / SPLIT`
//!   radians. The bake, the fault bound, the cover and the arena are sized
//!   by it, so it moves every level and every bound together; a quality
//!   coarser than it is a `Tau`, whose target a screen sets from
//!   `CENTER_CELL_LOGICAL_PX` here.
//! - pxl-spec bake/constant.rs `EXTENT_SPAN`: how many cells a page's index
//!   lane bounds at once, which the bake writes and the query reads.
//! - pxl-render look/lod.rs `REFERENCE_VIEWPORT_H` and `REFERENCE_FOV_DEG`:
//!   the view `SPLIT` was first stated at in pixels. And the mesher's work a step
//!   (pxl-render mesh/voxel_scan.rs `SCAN_ROWS_PER_STEP`,
//!   mesh/chunk_mesher.rs `BAKE_ROWS_PER_STEP`, `BAKE_BOXES_PER_STEP`),
//!   which is inside a step the kernel only calls.

use pxl_spec::space::pyramid::PAGE_CHUNKS;

// ---- product quality (world/quality.rs) ----

/// How many of the screen's logical pixels a cell of the observation spans
/// at the optical axis, where the arena has room for it. The cell is an
/// angle, the same step in every direction about the eye; this is only how
/// that angle is set from a screen: at the center of a perspective screen a
/// logical pixel subtends `2 · tan(fov / 2) / height`, and toward the edges
/// the same angle is drawn wider. A logical pixel — a CSS pixel on a page, a
/// point on a device — and not a device pixel, which is how finely the
/// screen is rasterized (world/frame/resolution.rs), not how finely the
/// world is told apart. Six is the reference view `SPLIT` was stated at
/// (pxl-render look/lod.rs): 1080 of them tall at 50°. A screen that asks
/// finer than `SPLIT` is held at it, since everything is sized there.
pub const CENTER_CELL_LOGICAL_PX: f64 = 6.0;

// ---- what refinement observes (world/observer.rs) ----

/// The side of the estimator's tile, in radians: a sixth, about ten
/// degrees, thirty-two cells of the target quality, sixty tiles and
/// seventy-seven rays on a 16:9 screen of 50°. A tile's footprint spans all
/// the ground its rays land between, so no size misses a piece seen; what
/// the size decides is how much ground hidden inside one tile — behind a
/// crest its top rays pass over — is taken to contribute and refined when
/// it need not be, against the rays a rebuild casts, one a corner, by the
/// inverse square. An eighth cast 135 rays for goals within one percent of
/// these over rolling ground (measured 2026-09-28). Not a cell: the
/// observation's cell is the quality's (`cell_angle`).
pub const OBSERVED_TILE: f64 = 1.0 / 6.0;

/// How far a tile's last answer may drift before it is cast again, as a
/// share of a tile: the eye turned by that angle, or moved by that angle
/// seen from the nearest thing the tile met, or the far edge moved by that
/// share. A quarter: a footprint then still lies within the tile it stands
/// for give or take a quarter of its side, which moves which pieces refine
/// at the tile's edges and nothing else. Smaller casts more often; larger
/// leaves a refinement a step behind the eye for longer.
pub const REUSE_SHIFT: f64 = 0.25;

// ---- temporal smoothness of the quality (world/quality.rs `Smooth`) ----

/// One step between two qualities the policy chooses among, as a factor on
/// the criterion: a half-level of every split distance, so a step coarser
/// is seen as ground drawn a little farther out rather than as a level
/// jumping, and a step finer is a small share of the pieces again.
pub const TAU_STEP: f64 = core::f64::consts::SQRT_2;

/// How many steps the policy may go coarser than the target: twelve
/// half-levels, six levels, past which a view is its top level and little
/// else, so a coarser one would save nothing.
pub const TAU_STEPS: usize = 12;

/// How long a finer step has to have had room before it is taken, in
/// milliseconds: long enough that a turn or a pan does not step in and out
/// again, which would build and throw away a level's pieces each time.
/// Coarser is taken at once and waits for nothing. More than zero, so the
/// policy asked twice in one instant steps once.
pub const QUALITY_WAIT_MS: f64 = 1_000.0;

/// The share of each pool that has to stay free at a finer step before it
/// is taken, past the frontier and the headroom: what the eye's next move
/// may add before it is decided again. None needed for correctness — a view
/// past the arena is coarsened at once — so it only keeps a step finer from
/// being taken back the moment the eye moves.
pub const QUALITY_MARGIN: f64 = 0.05;

// ---- concurrency (world/chunk/frontier.rs) ----

/// Refinements — a drawn piece replaced by the pieces under it, or by
/// itself drawn in another cube — under way at once. Each holds the piece
/// it replaces on the screen until what replaces it is whole, so each is a
/// slot past the frontier, and the quality policy keeps this many free
/// beside it. More is more of the screen sharpening at once, and a coarser
/// quality for the same arena.
pub const REFINEMENTS_IN_FLIGHT: u32 = 32;

// ---- device budget: memory (world/chunk/mesh_slot.rs, mem/fixed.rs) ----

/// Slots in one arena group, and so the z16 chunks drawn at their own cells
/// at once, the leaves' tier having one group. A product number, said
/// aloud: the arena is the largest thing the device holds, and this is what
/// it gives a view. It is the z16 chunks over flat ground within where z16
/// ends, as a disc; where a view needs more, the quality policy draws it
/// coarser.
pub const ARENA_GROUP_SLOTS: u32 = 330;

/// Groups the coarse tiers share. A product number, said aloud: a slot for
/// each of some thousand chunks a view draws in a coarse tier, and half as
/// many again for what a change of view holds while it lasts — refinements
/// under way, a dissolve's other side. The goal at the target over rolling
/// ground is some 350 to 760 pieces, from high up to 2u over it (measured
/// 2026-09-28); what the leak rule asks past the arena the quality policy
/// draws coarser.
pub const ARENA_COARSE_GROUPS: u32 = 5;

/// Boxes a coarse slot holds, its mesh's size on the device. A product
/// number, said aloud: a mesh past it fills more slots, never a coarser
/// cube, and the coarsest fills one (world/chunk/mesh_slot.rs asserts it).
pub const COARSE_BOXES: u32 = 4551;

/// Bytes a run set holds (`.fixed.runs`, world/chunk/build.rs): a leaf
/// chunk's columns at eleven runs each, where the terrain is one and what is
/// placed on it rarely a handful more. A set past it is refused rather than
/// cut, since a run left out is a hole.
pub const RUN_SET_BYTES: u32 = 1 << 20;

/// Instances one chunk's decor holds at most at a tier (`.fixed.decor`,
/// layer/decor/instance.rs): a kind scatters at most one to a cell, and the
/// placed entries come past them. What does not fit is left out.
pub const DECOR_INSTANCES: usize = 1 << 16;

/// Changes the world's change log keeps (kernel.rs): what a reader that fell
/// behind can still catch up from before it has to read everything again.
pub const JOURNAL_CAPACITY: u32 = 4096;

/// The share of the heap off-screen decor may keep before it is given back
/// farthest first. Half: the other half is the on-screen decor, the volumes
/// a build composes in, and the room a large block needs in one piece.
/// Dropping it all on the first turn away makes a rotation scatter every
/// chunk's decor again.
pub const RETAIN_SHARE: f64 = 0.5;

/// The heap's live share past which it counts as pressed, and off-screen
/// decor is given back whatever the retain budget says: the rest is what a
/// large block needs in one piece.
pub const HEAP_PRESSURE: f64 = 0.9;

// ---- per-frame work caps (world/chunk/table.rs, io/policy.rs) ----

/// Pieces a frame starts building, which is a frame's worth of work: each
/// is a chunk mounted and a build told its sources. A page's chunks, so a
/// page of flat ground is started in the frame it arrives in; more go the
/// next frame, nearest first.
pub const STARTS_PER_FRAME: u32 = PAGE_CHUNKS * PAGE_CHUNKS;

/// Layouts applied in one frame: a layout whose tiers have landed may build
/// its decor before it is shown, which is a block's scatter each.
pub const COMMITS_PER_FRAME: u32 = 8;

/// Off-screen chunks whose decor one frame gives back while the heap is
/// pressed, and how many when a block was refused this frame: a refusal is
/// a build on screen going nowhere, and the heap has no spill to fall back
/// on.
pub const RECLAIMS_PER_FRAME: u32 = 16;
pub const RECLAIMS_WHEN_REFUSED: u32 = 64;

/// The most attempts one pump starts (io/policy.rs). The credits already
/// bound what is out; this bounds only how fast they are taken up.
pub const INTAKE: u32 = u32::MAX;

// ---- latency hiding (world/prefetch.rs, world/pyramid.rs) ----

/// How much wider than the reach prefetch asks, as a factor of each
/// level's reach radius (world/prefetch.rs `Reach`, which the kernel runs
/// only once it is chosen in world/pyramid.rs `PREFETCH`). 1 is the reach
/// itself.
pub const PREFETCH_SLACK: f64 = 1.0;

/// How far past the reach prefetch asks for the eye's motion, in units:
/// what a round trip at the eye's speed would carry it (world/prefetch.rs
/// `Reach`). Nothing measures the speed yet, so none.
pub const MOTION_MARGIN: f64 = 0.0;

/// The most speculative pages a layer asks for at once, past what free
/// frames already limit them to (world/pyramid.rs `Pyramid::observe`).
pub const HINT_ALLOWANCE: u32 = 128;

// ---- I/O robustness (io/policy.rs, io/sched.rs) ----

/// How long an identity whose attempt failed waits before another, in
/// milliseconds: long enough not to ask straight back into whatever turned
/// it away.
pub const RETRY_MS: f64 = 2_000.0;

/// Fetches out at once at most, whatever the host grants: the host's grant
/// is cut to it, and the ledger's table of attempts out is this long.
pub const MAX_CREDITS: u32 = 16;

// ---- visual (world/chunk/table.rs) ----

/// How long a change of what is drawn dissolves over, in milliseconds: a
/// tier for another, a replaced piece for what replaced it. 300 read as
/// slow (2026-09-26); half of it is what the dissolve takes now.
pub const FADE_MS: f64 = 150.0;

// ---- the turn's budget (sched/tick.rs) ----

/// What of the frame the quantum leaves unspent, as a share of it — the
/// host's own work, its collector, and the turns the estimate is wrong
/// about: a share rather than a constant, so a fast display does not lose
/// most of its frame to it.
pub const QUANTUM_MARGIN_SHARE: f64 = 0.15;

/// Frames a turn that is not interactive may take: past one on purpose —
/// with nothing moving, a turn that takes two vsyncs shows the same image
/// twice, and nothing the eye can follow is waiting on it.
pub const QUANTUM_BATCH_FRAMES: f64 = 2.0;

/// How long after the last input a turn is still interactive, in
/// milliseconds: long enough that a pause inside one drag does not count.
pub const QUANTUM_SETTLE_MS: f64 = 150.0;

/// How far a pass may run past what is left, in milliseconds, so a step that
/// costs real time and reports nothing done cannot walk the whole list.
pub const HARD_SLACK_MS: f64 = 4.0;

/// The share of the budget held back from the visible phase for content:
/// without one, content waits for as long as a load keeps the visible phase
/// busy. A fixed share, not a feedback loop.
pub const CONTENT_SHARE: f64 = 0.2;

// ---- the drawn resolution (world/frame/resolution.rs) ----

/// One step of the ratio up or down: a fifth to a quarter of the fill
/// between the two ends, small enough that a step is not seen as a jump.
pub const RESOLUTION_STEP: f64 = 0.25;

/// How long the frames run over before a step down, in milliseconds: long
/// enough that one slow frame — a chunk's upload, a page arriving — is not
/// taken for a load.
pub const RESOLUTION_OVER_HOLD_MS: f64 = 500.0;

/// How long the GPU's time stays under `RESOLUTION_ROOM` of the budget
/// before a step up, in milliseconds.
pub const RESOLUTION_ROOM_HOLD_MS: f64 = 2000.0;

/// The share of the budget under which the GPU has room for the next step:
/// a step up adds at most (2.0 / 1.75)² ≈ 1.3 times the fill, so half the
/// budget leaves the step room and the frame's other work its share.
pub const RESOLUTION_ROOM: f64 = 0.5;

/// An interval this many budgets long is a missed frame, where the device
/// reports no GPU time: a frame slips by whole vsyncs.
pub const RESOLUTION_MISSED: f64 = 1.5;

/// How long the frames hold their budget before a first probe up, where
/// the device reports no GPU time, and how long a probe waits at most after
/// the ones before it came back down, in milliseconds.
pub const RESOLUTION_PROBE_HOLD_MS: f64 = 5000.0;
pub const RESOLUTION_PROBE_HOLD_MAX_MS: f64 = 60_000.0;

/// How long a probe up has to carry the frames before it is trusted, in
/// milliseconds.
pub const RESOLUTION_PROBE_TRUST_MS: f64 = 2000.0;

/// The frame the budget is at most as tight as: a sixtieth of a second. A
/// faster display's own interval would trade the device's sharpness for
/// frames past sixty, which the eye gains less from than it loses.
pub const RESOLUTION_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// The least time between two changes of the ratio, in milliseconds: a
/// change makes the world's targets again (pxl-render gpu/renderer.rs
/// `targets`), which is work of its own.
pub const RESOLUTION_CHANGE_GAP_MS: f64 = 1000.0;
