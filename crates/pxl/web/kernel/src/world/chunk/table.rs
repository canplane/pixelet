// pxl-kernel/world/chunk/table.rs

//! The chunks' scheduler: which chunks are on screen, which tier each shows,
//! what to build and in what order, and what to give back.
//!
//! A tick is four passes over the slots. `mount` decides which chunks exist
//! at all, by range from the anchor; `refresh` measures each against the
//! frame and flips its IN_FRUSTUM; `recheck` asks the rule for the tier
//! that distance wants and marks the slot pending when its layout is not
//! that; `commit` applies a layout whose tiers are all in hand, or releases
//! one that has gone off screen. What is committed first is `priority`'s
//! order, and a build that is refused for want of memory waits for the heap
//! to release something before it is tried again.
//!
//! The chunk nodes (node.rs) say what each chunk is made of; the chunk
//! stage (stage.rs) composes the frame — the camera, the anchor root's
//! matrix, the frustum — and calls the tick. What comes back is events: a
//! chunk shown or hidden, the layout it should show (its tiers), how the two
//! tiers of a blend are weighted, a tier's mesh handed over and a tier to let
//! go. The nodes hold the meshes; this holds every decision about them.

use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pyramid::{chunk_span_of, Cell, CHUNK_CELLS, GROUND, PAGE_CELLS, PAGE_CHUNKS};
use std::collections::{BTreeMap, VecDeque};

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch, arena_write};
use pxl_render::look::globe::Globe;
use pxl_render::look::lod::COARSEST;
use pxl_render::mesh::chunk_mesher::{box_bounds, Heights};
use pxl_render::mesh::paint::terrain::adapter::ChunkSample;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::Pvf3;
use pxl_spec::space::pyramid::TOP_LEVEL;
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use crate::global::Global;
use crate::io::load::Models;
use crate::layer::billboard::field::Billboards;
use crate::layer::decor::build::{decor_density, Chunk as DecorChunk, DecorBuild, Kind as DecorKind, Rule as DecorRule};
use crate::layer::decor::instance::{decor_groups, DecorScratch};
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::frame_gizmo::DebugLines;
use crate::layer::marker::hover_cursor::GhostDraw;
use crate::layer::marker::location_puck::Markers;
use crate::shell::setting::store::hidden_ground;
use crate::world::chunk::mesh_slot::{group_box_words, group_bytes, group_is_fine, pool, slot_at, MeshSlot, FINE_BOXES, OCCUPANCY};
use crate::world::chunk::node::{BuildEvent, Builder, PageLanes, World, MAX_CHUNKS};

use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::window::{split_distance, LEVEL_CHUNKS, WINDOW_LEVELS};
use crate::world::pyramid::{Domain, Layer};
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pyramid::LevelPage;

/// The sphere a chunk nothing has measured is culled by.
const APPROXIMATE_CHUNK_RADIUS: f64 = chunk_span_of(0) * 0.75;
/// How long a settled view goes before every chunk is rechecked anyway.
///
/// Reconciliation and nothing else. What a chunk should be showing is a
/// function of the view, so a view that moved is rechecked in the same
/// frame it moved in (`view_dirty`); this is for what moves without the
/// view — the budget's pressure, a generation, a source told late — and for
/// anything that sets the flag and is missed. A camera is not polled for.
const RECHECK_INTERVAL_S: f64 = 0.15;
/// Layouts applied per frame on screen, and releases off it — more of the
/// latter after a frame that had a block refused: a release is cheap, and
/// what an off-screen chunk still shows is what the refused block needs
/// room among: a zoom-in leaves many chunks showing their last layout off
/// screen, and the fine tiers find no room until they are released.
const MAX_COMMITS_PER_FRAME: u32 = 8;
const MAX_RELEASES_PER_FRAME: u32 = 2;
const MAX_RELEASES_WHEN_REFUSED: u32 = 64;
/// Off-screen chunks that may give their built work back in one frame, and
/// how many more when a block was refused this frame: a refusal is a build
/// on screen going nowhere, and the heap has no spill to fall back on.
const MAX_RECLAIMS_PER_FRAME: u32 = 16;
const MAX_RECLAIMS_WHEN_REFUSED: u32 = 64;
/// Chunks mounted in one frame, which is a frame's worth of work: the chunk
/// nodes make a node for each. A page comes into range as 64 of them, so one
/// page is mounted in the frame it arrives in.
const MAX_MOUNTS_PER_FRAME: u32 = 64;
const _: () = assert!(MAX_MOUNTS_PER_FRAME == PAGE_CHUNKS * PAGE_CHUNKS, "a page mounts in the frame it arrives in");

/// The widest selection there is: the top level's chunks, each of whose
/// pages may be stood in for by its four children where it has not arrived,
/// and a level's chunks for every level under it (window.rs `LEVEL_CHUNKS`).
/// The terrain draws no more than that (layer/terrain/present.rs): a finer
/// page stands in only at the top, a level finer, and the balance a mesh
/// needs only refines toward what the view wants.
///
/// The point of writing it down is that it is under MAX_CHUNKS. The table is
/// a hard budget, so what it can refuse matters: with the widest selection
/// inside it, what a full table turns away is never the chunk the camera
/// wants — there is always a stale one to give up instead (`give_room`), and
/// the current view can always be represented whatever the last one left.
const WIDEST_SELECTION: usize = (4 * LEVEL_CHUNKS + LEVEL_CHUNKS * (WINDOW_LEVELS - 1)) as usize;

const _: () = assert!(WIDEST_SELECTION <= MAX_CHUNKS, "render: a view the chunk table cannot hold at once");
/// The heap's live share past which off-screen chunks are reclaimed
/// whatever the retain budget says.
const HEAP_PRESSURE: f64 = 0.9;
/// Tiers one chunk may keep: the active pair, the off-screen tier, and one
/// of hysteresis.
const MAX_CACHED_TIERS: usize = 4;
/// A wait halves a chunk's distance key every this many milliseconds.
const AGE_HALF_MS: f64 = 4000.0;
/// The byte budget, paid in LOD: when the heap refuses a block and reclaim
/// found nothing to give back, the rule's criterion is pulled in a step (the module's
/// pressure) and the set on screen gets coarser until it fits; a step at
/// most this often, so the coarser layouts have time to land and free
/// what they replace before the next is taken, and up to three tiers. A
/// step is let out again after this long with no refusal and the section
/// off its pressure mark.
const MAX_PRESSURE: u32 = 6;
const PRESSURE_STEP_MS: f64 = 500.0;
const PRESSURE_RELIEF_MS: f64 = 5000.0;

pub const IN_FRUSTUM: u16 = 1;
pub const EMPTY: u16 = 2;
/// Wanted, and under a stale chunk that is drawn: what it lacks is not a
/// hole, since the stale chunk shows that ground until it is ready.
pub const COVERED: u16 = 8;
/// Mounted and not yet rechecked: it has no layout, so it wants one, and its
/// first recheck is taken whatever the frame's budget (`tick`).
pub const FRESH: u16 = 128;
pub const PENDING: u16 = 4;
pub const BLEND_ACTIVE: u16 = 16;
pub const SETTLING: u16 = 32;
/// Mounted but no longer wanted: its ground is now some other chunk's — a
/// parent it merged into, the children it split into — and it is drawn until
/// every one of those is ready, then let go.
pub const STALE: u16 = 64;
/// Nothing coarser in the selection draws this chunk's ground, so the rule
/// is followed as far as it goes here rather than stopping one step past the
/// chunk's own cells (`relative`): the view wants its ground at the
/// window's top, which has no ring over it (window.rs `Domain::outermost`).
pub const OUTERMOST: u16 = 256;

/// A release, as a request: off screen, nothing built.
const RELEASE_REQUEST: i32 = -1;

fn build_request(primary: u32, secondary: u32) -> i32 {
    ((primary << 8) | secondary) as i32
}

/// What the chunk stage composes before a tick.
#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The anchor root's world matrix, column-major.
    pub root: [f64; 16],
    /// The map origin, in units.
    pub origin: V3,
    /// The frustum as six planes of (nx, ny, nz, constant).
    pub planes: [f64; 24],
    pub camera: V3,
    pub viewport_h: f64,
    pub fov_deg: f64,
    pub delta_s: f64,
    /// What is left of the turn's quota when the pass begins, which is not
    /// the quota itself (`FrameSample::budget_ms`).
    pub remaining_ms: f64,
    pub hard_slack_ms: f64,
    pub pixel_dither: bool,
    pub retain_budget_bytes: f64,
    /// The camera centre's height (view.rs's camera_centre), in units: what
    /// the camera's height over the sea is measured up from.
    pub centre_z: f64,
    /// The map centre's latitude's values, which the renderer lays the map
    /// back on the planet about (pxl-render's look/globe.rs); none for a flat
    /// world.
    pub globe: Option<Globe>,
    /// Whether procedural decor is scattered at all (the setting).
    pub show_decor: bool,
}

impl Frame {
    /// Where `at`, in the world the camera is written in, is in the map's
    /// units: the anchor root undone — a rotation and a translation, so its
    /// inverse is its rotation transposed — then the origin put back.
    pub fn map_of(&self, at: V3) -> V3 {
        let m = &self.root;
        let (x, y, z) = (at[0] - m[12], at[1] - m[13], at[2] - m[14]);
        [
            m[0] * x + m[1] * y + m[2] * z + self.origin[0],
            m[4] * x + m[5] * y + m[6] * z + self.origin[1],
            m[8] * x + m[9] * y + m[10] * z + self.origin[2],
        ]
    }

    /// Where `at`, in the map's units, is in the world the camera is
    /// written in: against the origin, then through the anchor root.
    pub fn world_of(&self, at: V3) -> V3 {
        let m = &self.root;
        let (lx, ly, lz) = (at[0] - self.origin[0], at[1] - self.origin[1], at[2] - self.origin[2]);
        [
            m[0] * lx + m[4] * ly + m[8] * lz + m[12],
            m[1] * lx + m[5] * ly + m[9] * lz + m[13],
            m[2] * lx + m[6] * ly + m[10] * lz + m[14],
        ]
    }
}

/// What the chunk nodes are told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Build(BuildEvent),
    /// A chunk came into range: its page and its page-local base, in the
    /// page's cells.
    Mount {
        id: u32,
        page: LevelPage,
        base_x: u32,
        base_y: u32,
    },
    /// A chunk left the range, and everything the module held for it is gone.
    Unmount {
        id: u32,
    },
    /// Show or hide the chunk's group.
    Visible {
        id: u32,
        on: bool,
    },
    /// The layout to show: its tiers, all of them already handed over, of
    /// `generation`. `ready` false is the release: nothing shown.
    Layout {
        id: u32,
        primary: u32,
        secondary: u32,
        generation: u32,
        ready: bool,
    },
    /// How the layout's two tiers are weighted, and which one decor takes.
    Blend {
        id: u32,
        primary: f32,
        secondary: f32,
        decor_secondary: bool,
    },
    /// Let go of a tier the chunk nodes hold, and the decor at its step, of
    /// `generation`; the layout keeps what it shows.
    Evict {
        id: u32,
        step: u32,
        generation: u32,
    },
    /// The chunk's decor at a tier of `generation`, as decor/build.rs's block of
    /// `bytes` at `at`, the chunk nodes' to release; every tier of a layout has
    /// its decor handed over before the layout.
    Decor {
        id: u32,
        step: u32,
        generation: u32,
        at: u32,
        bytes: u32,
    },
}

/// What a mount pass selected for: which making of the terrain's cover
/// (world/pyramid.rs `Pyramid::cover`), which is made again whenever the
/// view or the resident pages change. While it is the same the selection
/// stands, and the pass is skipped once nothing of it is left to mount.
type MountKey = u32;

/// Whether two squares, each a corner and a side, share any ground.
fn overlaps(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.2 && b.1 < a.1 + a.2
}

/// The ground two such squares share.
fn shared_area(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let w = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
    let h = (a.1 + a.2).min(b.1 + b.2) - a.1.max(b.1);
    w.max(0.0) * h.max(0.0)
}

#[derive(Clone, Copy, Default)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy)]
struct Tier {
    step: u32,
    /// The telling of the chunk's sources it was built from (the builder's).
    generation: u32,
    bounds: Option<Bounds>,
    /// Where its boxes lie on the device, and how many.
    slot: MeshSlot,
    box_cnt: u32,
    /// What its boxes' heights are held in (pxl-render chunk_mesher.rs).
    heights: Heights,
}

/// A decor block the chunk nodes hold: its step and bytes, and the renderer's
/// handle. Each group's lanes are where the block's own header says.
#[derive(Clone, Copy, Default)]
pub struct Decor {
    step: u32,
    generation: u32,
    /// Where the block is in the memory, and its size.
    pub at: u32,
    pub bytes: u32,
    handle: u32,
}

impl Decor {
    /// Each group's first lane word in the block and its count
    /// (decor/instance.rs's decor_groups), the block read in `mem`.
    fn groups(&self, mem: *const u8) -> impl Iterator<Item = (u32, u32)> {
        // SAFETY: the block the scheduler wrote at `at`, `bytes` long, held
        // for as long as this decor is.
        let words: &[u32] = unsafe { core::slice::from_raw_parts(mem.add(self.at as usize) as *const u32, self.bytes as usize / 4) };
        decor_groups(words)
    }
}

#[derive(Clone)]
struct Slot {
    id: u32,
    /// The chunk it draws.
    chunk: Chunk,
    /// The level it is drawn from, and its side in units: 64 cells of 2^level.
    level: u32,
    span: f64,
    base_x: f64,
    base_y: f64,
    /// The bounding sphere in the chunk's own space.
    bc: V3,
    radius: f64,
    /// The nearest distance from the eye to its ground, squared: what its
    /// level was chosen by (window.rs `Domain::nearest_to`), and so what its
    /// work is ordered by.
    distance2: f64,
    flags: u16,
    pending_since: f64,
    request: i32,
    /// The tiers the chunk nodes hold, oldest built first. A source changed
    /// leaves them where they are: a step's tier of the new generation joins
    /// the old one's, and the old go when a layout of the new is applied.
    tiers: Vec<Tier>,
    /// The decor blocks the chunk nodes hold, by step and generation alike.
    decor: Vec<Decor>,
    layout_primary: u32,
    layout_secondary: u32,
    /// The generation the layout's tiers and decor are drawn from.
    layout_generation: u32,
    layout_ready: bool,
    desired: (u32, u32),
    /// The last blend told, so an unchanged one is not resent.
    told: Option<(f32, f32, bool)>,
    /// The heap's release count when its decor was last refused a
    /// block: not committed again until a block is given back.
    refused: Option<u32>,
    /// A stale chunk's ground as `retire` last found it covered: the area
    /// wanted chunks cover, whether every one of them is laid out, and
    /// whether one that is not is waiting on a mesh slot.
    cover: (f64, bool, bool),
}

impl Slot {
    /// The sphere the chunk is culled by until its mesh has measured it: the
    /// whole of its ground, which is `span` wide at its level. A leaf's
    /// sphere at a coarse chunk's corner culls the chunk from any view that
    /// misses the corner, and a chunk never in view is never built, so it is
    /// never measured and stays culled.
    fn unmeasured(&mut self) {
        self.bc = [self.span * 0.5, self.span * 0.5, 0.0];
        self.radius = APPROXIMATE_CHUNK_RADIUS * (1u64 << self.level) as f64;
    }

    fn has(&self, f: u16) -> bool {
        self.flags & f != 0
    }

    fn set(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f
        } else {
            self.flags &= !f
        }
    }

    /// What the chunk holds in the heap: its decor. Its runs are a set of
    /// `.fixed.runs` and its tiers are on the device.
    fn retained(&self) -> f64 {
        self.decor.iter().map(|d| d.bytes as f64).sum::<f64>()
    }

    fn tier(&self, step: u32, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.step == step && t.generation == generation)
    }

    fn decor_of(&self, step: u32, generation: u32) -> Option<&Decor> {
        self.decor.iter().find(|d| d.step == step && d.generation == generation)
    }
}

/// The census the ledger reads.
#[derive(Clone, Copy, Default)]
pub struct Census {
    /// Chunks the table holds, which is the number MAX_CHUNKS bounds.
    pub chunks: u32,
    pub in_frustum: u32,
    pub settling: u32,
    pub blend_active: u32,
    /// Chunks still pending once the frame is done. Not what the frame began
    /// with less what it committed, which would also count the chunks a build
    /// landed on this frame, pending or not when it began.
    pub pending: u32,
    pub offscreen_pending: u32,
    pub oldest_visible_ms: f64,
    pub oldest_release_ms: f64,
    pub released: u32,
    pub retained_on: f64,
    pub retained_off: f64,
    /// The budget's pressure: how far the byte budget has pulled the LOD in.
    pub pressure: u32,
    /// Stale chunks: mounted, no longer wanted, kept until what replaces
    /// them is whole.
    pub stale: u32,
    /// Of those, the ones drawn, and the levels they are of as bits.
    pub stale_drawn: u32,
    pub stale_levels: u32,
    /// Wanted chunks on screen with a layout that a drawn stale chunk hides.
    pub hidden: u32,
    /// Meshes thrown away this frame because the slot they go to is another
    /// chunk's, still drawn from; the levels of the chunks that waited and of
    /// those that held the slots, as bits.
    pub slot_waits: u32,
    pub wait_levels: u32,
    pub holder_levels: u32,
    /// Boxes cut from meshes past their slot since the last census.
    pub cut: u32,
    /// The largest mesh handed over since the last census, in boxes, of a
    /// fine slot and of a coarse one.
    pub fine_peak: u32,
    pub coarse_peak: u32,
    /// Stale chunks `retire` kept: what replaces them does not cover their
    /// ground yet; covers it but is not all laid out; and, of those, the
    /// ones where what is not laid out is a chunk waiting on a slot.
    pub stale_uncovered: u32,
    pub stale_unready: u32,
    pub stale_on_waiter: u32,
    /// The boxes the last draw listed, by the level of the chunk they are of.
    pub level_boxes: [u32; LEVELS],
    /// What the frame's ceilings left behind, which is how anyone tells
    /// whether a ceiling is the thing holding the screen back: chunks on
    /// screen still wanting a layout that the commit pass did not reach,
    /// chunks the selection wants that are not mounted, and whether the
    /// mount pass stopped because the table is full rather than because the
    /// frame's own cap was spent.
    pub commit_left: u32,
    pub mount_left: u32,
    pub mount_full: u32,
}

/// The pyramid's levels, a chunk's being one of them.
pub const LEVELS: usize = TOP_LEVEL as usize + 1;

/// Who holds each mesh slot (mesh_slot.rs), by chunk id, 0 for none; and the
/// arena epoch the tiers were written under.
pub struct Meshes {
    holder: Vec<u32>,
    surface: Vec<Box<[u32]>>,
    epoch: u32,
    /// Boxes cut from meshes past their slot, until a census takes them.
    pub cut: u32,
    /// The largest mesh handed to a slot of each kind, until a census takes
    /// them: what a slot's size is read against.
    fine_peak: u32,
    coarse_peak: u32,
    /// Meshes thrown away for a slot still drawn from, since the census; the
    /// levels of the waiters and the holders as bits; and the first waiters,
    /// as many as `retire` needs to tell a stale chunk kept for one.
    waits: u32,
    wait_levels: u32,
    holder_levels: u32,
    waiters: [u32; WAITERS],
    waiter_cnt: usize,
}

/// Waiters `Meshes` names between two `retire`s: past it, a wait is counted
/// and its chunk left unnamed.
const WAITERS: usize = 32;

pub struct Scheduler {
    pub meshes: Meshes,
    /// Wanted chunks the last draw left under a stale one (`Census::hidden`).
    hidden: u32,
    /// The boxes the last draw listed by level (`Census::level_boxes`).
    level_boxes: [u32; LEVELS],
    /// Why `retire` last kept its stale chunks (`Census::stale_uncovered`).
    kept: (u32, u32, u32),
    pub builder: Builder,
    /// What the renderer uploads, frees and draws (draw.rs).
    pub draw: DrawList,
    /// The hover ghost (marker/hover_cursor.rs).
    pub ghost: GhostDraw,
    /// Models and the actors that play them (io/load.rs).
    pub models: Models,
    /// The location puck (marker/location_puck.rs).
    pub markers: Markers,
    /// The billboards (billboard/field.rs).
    pub billboards: Billboards,
    /// The model each decor group is drawn with, by group, once the decor assets
    /// registered it: what a hit test reads the triangles of.
    pub decor_models: Vec<Option<u32>>,
    /// The sun, the moon, the light and the sky (layer/environment/state.rs).
    pub environment: EnvironmentDraw,
    /// The dev gizmos (marker/frame_gizmo.rs).
    pub debug: DebugLines,
    slots: Vec<Slot>,
    index: BTreeMap<u32, usize>,
    /// How many levels coarser than the rule the byte budget has pulled the
    /// tiers (`press`): it moves the criterion, never the rule.
    pressure: u32,
    recheck_elapsed: f64,
    frame: Option<Frame>,
    /// Something changed that a recheck has not seen: the view moved, a
    /// chunk came or went, a source was told. The next tick rechecks every
    /// chunk, so what each should be showing is never older than the frame
    /// that changed it. The canvas is asked for frames until that recheck
    /// has run, or the world stops at whatever the last frame left —
    /// nothing else asks for a frame.
    view_dirty: bool,
    retained_on: f64,
    retained_off: f64,
    pub census: Census,
    /// Pages whose records have gone since the last tick, whose chunks stop
    /// building before anything else in the next.
    evicted: Vec<LevelPage>,
    /// The chunks the module mounted, by page and page-local base.
    mounted: BTreeMap<Chunk, u32>,
    /// The mounted chunks a pass finds leaving, before they are let go.
    gone: Vec<(Chunk, u32)>,
    next_mount_id: u32,
    /// What the selection standing in `to_mount` was made for, and what of it
    /// is still to be mounted. The pass is done when the key matches and
    /// nothing is left.
    selected_for: Option<MountKey>,
    /// The chunks that selection wants and the module has not mounted, in
    /// the order the builds will take them: nearest first, a distance in the
    /// chunk's own sides.
    ///
    /// Held across frames because the selection is expensive and the frames
    /// that cannot finish mounting are exactly the frames that are already
    /// late. A pass mounts MAX_MOUNTS_PER_FRAME of them, so a window that
    /// slides by one level leaves some six hundred over, nine frames' worth
    /// — and walking the window's quadtree again for each of those nine is
    /// two thousand chunks re-derived at a tilt wide enough to want five
    /// levels, against six hundred looking down. The work a zoom costs grew
    /// with the tilt for no reason but this.
    to_mount: VecDeque<(Chunk, bool)>,
    /// The ground the view draws: the window's top-level chunks, each a
    /// corner and a side in units (window.rs `Domain::ground`).
    drawn: Option<Vec<(f64, f64, f64)>>,
    /// The decor kinds, in the order decor/build.rs taught them — the order of
    /// the groups in every decor block.
    decor_kinds: Vec<DecorKind>,
    /// A decor build's leaf columns' samples, one a column, kept for their
    /// capacity.
    decor_samples: Vec<ChunkSample>,
    /// Where a decor build gathers its instances, made at the first build:
    /// the scheduler is made before the sections are.
    decor_scratch: Option<DecorScratch>,
    /// The builder's refusals as of the last reclaim, so a refusal since
    /// is pressure now; and whether the last pass saw one, for the next
    /// frame's release cap.
    refused_seen: u32,
    refused_last: bool,
    /// The chunks the build task advances, in the one order, as the last
    /// tick left them.
    builds: Vec<u32>,
    landed: Vec<u32>,
    /// Chunks the build task committed since the last census.
    build_committed: u32,
    /// When the pressure last moved, and when a block was last refused.
    pressure_at: f64,
    refused_at: f64,
    /// The tick's lists, kept for their capacity: a tick over a thousand
    /// chunks fills six of them.
    scratch: Scratch,
}

/// Empty by default: the tick takes it out with `mem::take` and puts it back,
/// so what `Default` makes is thrown away each tick, and allocating there
/// would allocate every tick.
#[derive(Default)]
struct Scratch {
    order: Vec<usize>,
    keys: Vec<Keyed>,
    visible: Vec<usize>,
    offscreen: Vec<usize>,
    eligible: Vec<usize>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new()
    }
}

static BUILDS: Global<Notify> = Global::new();

/// What the build task waits on while no tick has given it chunks to build.
pub fn builds() -> &'static mut Notify {
    BUILDS.get(Notify::new)
}

/// xpute's pass over `items` against the tick's absolute deadlines: the first
/// item always, then until `cap` items did something or the budget is spent,
/// and never past the hard deadline once one was tried. How many did
/// something, and how many were reached at all — the second is what says
/// whether a ceiling is what stopped it.
fn under_budget<T: Copy>(items: &[T], cap: u32, deadline: f64, hard: f64, now: &impl Fn() -> f64, step: impl FnMut(T) -> bool) -> PassStats {
    let at = now();
    let opts = BudgetOpts {
        budget_ms: deadline - at,
        hard_budget_ms: Some(hard - at),
        max_steps: Some(cap),
    };
    run_under_budget(items.iter().copied(), step, opts)
}

impl Scheduler {
    pub fn new() -> Scheduler {
        arena_configure(&group_bytes());
        Scheduler {
            hidden: 0,
            level_boxes: [0; LEVELS],
            kept: (0, 0, 0),
            meshes: Meshes {
                holder: vec![0; OCCUPANCY],
                surface: (0..OCCUPANCY).map(|_| Box::default()).collect(),
                epoch: arena_epoch(),
                cut: 0,
                fine_peak: 0,
                coarse_peak: 0,
                waits: 0,
                wait_levels: 0,
                holder_levels: 0,
                waiters: [0; WAITERS],
                waiter_cnt: 0,
            },
            builder: Builder::new(),
            draw: DrawList::default(),
            ghost: GhostDraw::default(),
            models: Models::default(),
            markers: Markers::default(),
            billboards: Billboards::default(),
            decor_models: Vec::new(),
            environment: EnvironmentDraw::default(),
            debug: DebugLines::default(),
            slots: Vec::with_capacity(MAX_CHUNKS),
            index: BTreeMap::new(),
            pressure: 0,
            recheck_elapsed: RECHECK_INTERVAL_S,
            frame: None,
            view_dirty: true,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            evicted: Vec::new(),
            mounted: BTreeMap::new(),
            gone: Vec::with_capacity(MAX_CHUNKS),
            next_mount_id: MOUNT_ID_BASE,
            selected_for: None,
            to_mount: VecDeque::new(),
            drawn: None,
            decor_kinds: Vec::new(),
            decor_samples: Vec::new(),
            decor_scratch: None,
            refused_seen: 0,
            scratch: Scratch::default(),
            refused_last: false,
            builds: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            pressure_at: f64::NEG_INFINITY,
            refused_at: f64::NEG_INFINITY,
        }
    }

    /// Teaches a decor kind; its index. Its rules start as `Any` on every
    /// channel, and `decor_rule` sets them.
    pub fn decor_kind(&mut self, kind: DecorKind) -> u32 {
        // Decoration is scattered from the leaves alone, and the rule draws
        // leaves only out to DECOR_REACH for certain: a kind whose own reach
        // runs past that is cut where the leaves end, until decoration is
        // scattered at whatever level the rule draws its ground at.
        self.decor_kinds.push(kind);
        (self.decor_kinds.len() - 1) as u32
    }

    /// Forgets every kind taught, so they can be taught again.
    pub fn decor_forget(&mut self) {
        self.decor_kinds.clear();
    }

    pub fn decor_rule(&mut self, kind: u32, channel: usize, rule: DecorRule) {
        if let Some(k) = self.decor_kinds.get_mut(kind as usize) {
            k.rules[channel] = rule;
        }
    }

    /// Allows one type on a channel of a kind's rule, which becomes `OneOf`
    /// if it was not.
    pub fn decor_allow(&mut self, kind: u32, channel: usize, t: u8) {
        let Some(k) = self.decor_kinds.get_mut(kind as usize) else { return };
        let mut bits = match k.rules[channel] {
            DecorRule::OneOf(b) => b,
            _ => [0; 4],
        };
        bits[(t >> 6) as usize] |= 1 << (t & 63);
        k.rules[channel] = DecorRule::OneOf(bits);
    }

    fn account(&mut self, i: usize, before: f64) {
        let delta = self.slots[i].retained() - before;
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on += delta;
        } else {
            self.retained_off += delta;
        }
    }

    // ---- commands ----

    /// Chunk `k`, its first tier the one the rule wants for it now.
    fn add(&mut self, id: u32, k: Chunk, root: bool) {
        let level = k.level();
        let (base_x, base_y, _) = k.rect();
        self.builder.add(id, level, ivec::gather(base_x as i64, base_y as i64, 0), k.bx, k.by);
        let span = chunk_span_of(level);
        let mut slot = Slot {
            id,
            chunk: k,
            level,
            span,
            base_x,
            base_y,
            bc: [0.0; 3],
            radius: 0.0,
            distance2: f64::INFINITY,
            flags: EMPTY | FRESH,
            pending_since: -1.0,
            request: 0,
            tiers: Vec::new(),
            decor: Vec::new(),
            layout_primary: COARSEST,
            layout_secondary: 0,
            layout_generation: 0,
            layout_ready: false,
            desired: (COARSEST, 0),
            told: None,
            refused: None,
            cover: (0.0, true, false),
        };
        slot.set(OUTERMOST, root);
        slot.unmeasured();
        if let (Some(frame), Some(d)) = (self.frame, Terrain::pyramid().domain()) {
            if measure(&frame, &slot).0 {
                slot.layout_primary = relative(wanted_step(d, k, self.pressure), level, root);
                slot.desired = (slot.layout_primary, 0);
            }
        }
        self.index.insert(id, self.slots.len());
        assert!(self.slots.len() < MAX_CHUNKS, "more than {MAX_CHUNKS} chunks held");
        self.slots.push(slot);
        self.view_dirty = true;
    }

    /// Whether nothing coarser in the selection draws this chunk's ground, so
    /// its own tier is free to be as coarse as the rule says.
    ///
    /// A root of the selection's traversal (`select`), which used to be the
    /// same as being at the window's top ring and is not any more: a chunk
    /// standing in for a page that has not arrived is a root at whatever
    /// level it is, and capping it one step past its own cells would draw the
    /// far ground at the fine step the level over it was supposed to spare
    /// it. Cesium's is the same: a descendant rendering in place of an
    /// unloaded ancestor is measured against the screen on its own, not
    /// against the ancestor's.
    fn outermost(&self, i: usize) -> bool {
        self.slots[i].has(OUTERMOST)
    }

    pub fn remove(&mut self, world: &mut World, id: u32) {
        let Some(i) = self.index.remove(&id) else { return };
        self.view_dirty = true;
        self.builder.remove(world, id);
        let before = self.slots[i].retained();
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on -= before;
        } else {
            self.retained_off -= before;
        }
        for t in self.slots[i].tiers.iter() {
            let held = &mut self.meshes.holder[t.slot.index];
            if *held == id {
                *held = 0;
            }
        }
        for d in self.slots[i].decor.iter() {
            self.draw.free(d.handle);
        }
        self.slots.swap_remove(i);
        if i < self.slots.len() {
            self.index.insert(self.slots[i].id, i);
        }
    }

    /// The builder's generation for slot `i`: the telling of its sources a
    /// tier must be built from to count.
    fn generation(&self, i: usize) -> u32 {
        self.builder.generation_of(self.slots[i].id).unwrap_or(0)
    }

    /// The chunk's sources changed. What it shows stays up: the tiers and the
    /// decor it holds are an older generation now, drawn until a layout of
    /// the new one is whole and then let go at once, so a change is never a
    /// hole and never half of one. Where it stands among the builds is the one
    /// order's to say (`priority`'s `restale`), not this.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool) {
        let Some(&i) = self.index.get(&id) else { return };
        self.view_dirty = true;
        let before = self.slots[i].retained();
        self.builder.source(world, id, page, show_structures);
        let s = &mut self.slots[i];
        if !s.layout_ready {
            s.unmeasured();
        }
        self.account(i, before);
        if self.slots[i].layout_ready && self.slots[i].has(IN_FRUSTUM) {
            let (p, q) = self.slots[i].desired;
            self.set_pending(i, true, build_request(p, q), crate::host::now());
            // Asked now rather than waiting for the tick's commit pass, so
            // the builder knows what it wants before the next one.
            self.ask(world, i, p, q);
        }
    }

    /// The build task's step: the chunks the last tick ordered, holders of the
    /// scan and the bake slot first, advanced for what is left of the turn's
    /// quota and at least one step, and the chunks a tier landed on
    /// committed. Whether any did work; none did when all are built, blocked,
    /// or a block was refused — the heap is the same until the next tick
    /// reclaims.
    pub fn build_step(&mut self, world: &mut World, budget_ms: f64, emit: &mut impl FnMut(Event)) -> bool {
        let mem = world_mem(world);
        let (slots, index, meshes, landed) = (&mut self.slots, &self.index, &mut self.meshes, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick_capped(world, &self.builds, budget_ms, u32::MAX, &crate::host::now, &mut |e| {
            let BuildEvent::Tier { id, .. } = e;
            landed.push(id);
            self_record(slots, index, mem, meshes, e, emit);
        });
        let landed = core::mem::take(&mut self.landed);
        for &id in &landed {
            if let Some(&i) = self.index.get(&id) {
                if self.commit(world, i, emit) {
                    self.build_committed += 1;
                }
            }
        }
        self.landed = landed;
        worked
    }

    /// Only the chunk's decor changed (an entry placed or hidden, the
    /// setting): the decor the chunk nodes hold is stale and they let it go; the
    /// layout's decor is rebuilt now and the layout told again, the tiers
    /// untouched. Unless a block was refused, when it lands on a later tick.
    pub fn decor_now(&mut self, world: &mut World, id: u32, show_decor: bool, emit: &mut impl FnMut(Event)) {
        if let Some(f) = self.frame.as_mut() {
            f.show_decor = show_decor;
        }
        let Some(&i) = self.index.get(&id) else { return };
        let before = self.slots[i].retained();
        for d in core::mem::take(&mut self.slots[i].decor) {
            self.draw.free(d.handle);
        }
        self.account(i, before);
        if !self.slots[i].layout_ready {
            return;
        }
        let (p, s2, g) = (self.slots[i].layout_primary, self.slots[i].layout_secondary, self.slots[i].layout_generation);
        if self.ensure_decor(world, i, [p, s2], g, emit) {
            self.apply_layout(i, p, s2, g, true, emit);
        } else {
            let s = &mut self.slots[i];
            s.layout_ready = false;
            s.set(EMPTY, true);
        }
    }

    /// Whether the chunk nodes hold the decor of every tier in `steps` of
    /// `generation`, building and handing over what it lacks: false when the
    /// heap refused a block. A chunk with no page has no decor to build.
    fn ensure_decor(&mut self, world: &mut World, i: usize, steps: [u32; 2], generation: u32, emit: &mut impl FnMut(Event)) -> bool {
        let (id, level, x0, y0) = {
            let s = &self.slots[i];
            (s.id, s.level, s.base_x as i64, s.base_y as i64)
        };
        let show_decor = self.frame.is_some_and(|f| f.show_decor);
        if self.decor_scratch.is_none() {
            self.decor_scratch = DecorScratch::new();
            if self.decor_scratch.is_none() {
                self.builder.stat.starved += 1;
                self.slots[i].refused = Some(crate::host::heap_stat().releases);
                return false;
            }
        }
        for step in steps {
            if step == 0 || self.slots[i].decor_of(step, generation).is_some() {
                continue;
            }
            // Decor is scattered from the leaves whatever level draws the
            // ground: the leaf columns this chunk covers, at the world step
            // it is drawn at. A level whose step no kind is drawn at has none,
            // and past that nothing is built for it.
            let world_step = step << level;
            let scatters = show_decor && self.decor_kinds.iter().any(|k| k.density > 0.0 && decor_density(k.extent, world_step) > 0.0);
            if level > 0 && !scatters {
                continue;
            }
            let n = 1i64 << level;
            let mut sources = Vec::with_capacity((n * n) as usize);
            for cy in 0..n {
                for cx in 0..n {
                    let leaf = chunk_span_of(0) as i64;
                    let base = ivec::gather(x0 + cx * leaf, y0 + cy * leaf, 0);
                    if let Some(source) = self.builder.leaf_source(base) {
                        sources.push((base, source, ((cx * leaf) as f64, (cy * leaf) as f64)));
                    }
                }
            }
            if self.decor_samples.len() < sources.len() {
                self.decor_samples.resize_with(sources.len(), ChunkSample::default);
            }
            // SAFETY: each a resident leaf page, read within the command.
            let views: Vec<_> = sources.iter().map(|(_, src, _)| unsafe { Builder::page_view(world, src.page) }).collect();
            if scatters {
                for (k, (_, src, _)) in sources.iter().enumerate() {
                    self.decor_samples[k].read(&views[k], world.materials, src.tx0, src.ty0, hidden_ground(src.show_structures));
                }
            }
            let columns: Vec<(DecorChunk, &ChunkSample)> = sources
                .iter()
                .enumerate()
                .map(|(k, (base, src, (ox, oy)))| {
                    let col_idx = L2_IDX(*base);
                    let (col_x, col_y) = (ivec::X(*base), ivec::Y(*base));
                    let chunk = DecorChunk {
                        page: &views[k],
                        page_idx: L3_IDX(*base),
                        col_idx,
                        col_x,
                        col_y,
                        tx0: src.tx0,
                        ty0: src.ty0,
                        show_decor,
                        hide: self.builder.hide_in(col_idx),
                        ox: *ox,
                        oy: *oy,
                        hittable: level == 0,
                    };
                    (chunk, &self.decor_samples[k])
                })
                .collect();
            let Some(scratch) = self.decor_scratch.as_mut() else { return false };
            let built = DecorBuild::build(scratch, &self.decor_kinds, &columns, world.store, world_step);
            let bytes = built.bytes();
            let Some((at, bytes)) = alloc(bytes) else {
                self.builder.stat.starved += 1;
                self.slots[i].refused = Some(crate::host::heap_stat().releases);
                return false;
            };
            // SAFETY: a block of the bytes the build asked for, the chunk node's
            // once handed over.
            unsafe { built.write(world.volumes.mem().add(at)) };
            let before = self.slots[i].retained();
            let handle = self.draw.upload(at as u32, bytes, 0, 0);
            self.slots[i].decor.push(Decor {
                step,
                generation,
                at: at as u32,
                bytes,
                handle,
            });
            self.account(i, before);
            emit(Event::Decor {
                id,
                step,
                generation,
                at: at as u32,
                bytes,
            });
        }
        true
    }

    /// The model decor group `group` is drawn with.
    pub fn decor_model(&mut self, group: u32, model: u32) {
        let g = group as usize;
        if self.decor_models.len() <= g {
            self.decor_models.resize(g + 1, None);
        }
        self.decor_models[g] = Some(model);
    }

    /// Every on-screen chunk's decor as shown this frame — the block of the
    /// tier decor takes in its layout — with the chunk's id and base, in
    /// units: what a hit test walks.
    pub fn shown_decor(&self) -> impl Iterator<Item = (u32, f64, f64, &Decor)> {
        self.slots.iter().filter(|s| s.has(IN_FRUSTUM) && s.layout_ready).filter_map(|s| {
            let decor_step = match s.told {
                Some((_, _, true)) if s.layout_secondary != 0 => s.layout_secondary,
                _ => s.layout_primary,
            };
            s.decor_of(decor_step, s.layout_generation).map(|d| (s.id, s.base_x, s.base_y, d))
        })
    }

    /// A page's terrain tile arrived, or left. Once a record has gone — let
    /// go, or taken by another page whose bytes now lie where this page's
    /// did — nothing may be built from it again: a build would read what is no
    /// longer its page. What its chunks have built stays drawn, since the
    /// ground they cover may have nothing else to draw it yet; they go stale
    /// and leave as any stale chunk does, once covered (`retire`).
    pub fn page(&mut self, page: LevelPage, present: bool) {
        if !present {
            self.evicted.push(page);
        }
        self.view_dirty = true;
    }

    /// The run sets the builder has lent, and the bytes their runs fill.
    pub fn builder_run_sets(&self) -> (u32, u32) {
        self.builder.run_sets()
    }

    /// Decor instances the build's lanes had no room for, since the kernel
    /// started: the ledger's, so a chunk short of its decor says so.
    pub fn decor_dropped(&self) -> u32 {
        self.decor_scratch.as_ref().map_or(0, |s| s.dropped)
    }

    /// The chunks to draw and which of them are roots: what the terrain
    /// draws of the view (world/pyramid.rs `Cover`, layer/terrain/present.rs).
    fn select(&self, out: &mut BTreeMap<Chunk, bool>) {
        let terrain = Terrain::pyramid();
        let Some(d) = terrain.domain() else { return };
        out.extend(terrain.cover().0.drawn.iter().map(|&c| (c, d.outermost(c))));
    }

    /// Which chunks exist: the ones `select` draws, mounted as they are
    /// wanted, and the ones it no longer draws marked stale rather than let
    /// go, so the ground they cover is never a hole while what replaces them
    /// builds (`retire`). Rerun when the anchor crosses into another page
    /// at the window's base, the window slides or a page comes or goes
    /// (`MountKey`).
    /// Lets one stale chunk go so a wanted one can mount.
    ///
    /// The table is a hard budget and the widest selection fits inside it
    /// (WIDEST_SELECTION), so a full table with something still wanted is a
    /// table holding stale chunks — there is always one to give up, and the
    /// view the camera has now is never the thing turned away. It used to be:
    /// the pass stopped at the ceiling and tried again next frame, which left
    /// the last epoch's chunks in place and the current one outside.
    ///
    /// The one showing least goes: not drawn before drawn, and the furthest
    /// out among equals.
    fn give_room(&mut self, world: &mut World, emit: &mut impl FnMut(Event)) -> bool {
        let mut worst: Option<((bool, f64), Chunk, u32)> = None;
        for (&k, &id) in self.mounted.iter() {
            let Some(&i) = self.index.get(&id) else { continue };
            let s = &self.slots[i];
            if !s.has(STALE) {
                continue;
            }
            let key = (!(s.has(IN_FRUSTUM) && s.layout_ready), s.distance2);
            if worst.as_ref().is_none_or(|(low, _, _)| key > *low) {
                worst = Some((key, k, id));
            }
        }
        let Some((_, k, id)) = worst else { return false };
        self.mounted.remove(&k);
        self.remove(world, id);
        emit(Event::Unmount { id });
        true
    }

    fn mount(&mut self, world: &mut World, emit: &mut impl FnMut(Event)) {
        let key = Terrain::pyramid().cover().1;
        if self.selected_for == Some(key) && self.to_mount.is_empty() {
            return;
        }
        self.census.mount_full = 0;
        self.drawn = Terrain::pyramid().domain().map(Domain::ground);
        if self.selected_for != Some(key) {
            let mut wanted = BTreeMap::new();
            self.select(&mut wanted);

            // What is no longer wanted goes stale; what is again, fresh. And
            // whether a chunk is a root can change under it — a page
            // arriving puts a level over ground that had none — so it is
            // written here for every chunk the selection keeps.
            for (k, &id) in self.mounted.iter() {
                if let Some(&i) = self.index.get(&id) {
                    self.slots[i].set(STALE, !wanted.contains_key(k));
                    if let Some(&root) = wanted.get(k) {
                        self.slots[i].set(OUTERMOST, root);
                    }
                }
            }

            // In the order the builds will take them, which is a distance in
            // the chunk's own sides (`priority`), so a coarse chunk far off
            // goes with a fine one near. Left in the key's own order — a
            // `Chunk` sorts by its page and a `LevelPage` by its level —
            // the pass mounted every chunk of the finest level before any of
            // the next, and within a level in raw coordinate order rather
            // than by distance. A chunk cannot be built before it is mounted,
            // so that order, and not the build order, decided what the screen
            // filled in: at a tilt wanting five levels the top level's four
            // hundred chunks were the last seven frames of thirty-six, and
            // the ground at the horizon appeared last however badly the
            // builds wanted it.
            //
            // Nearest the eye first, by the distance the level was chosen
            // by (window.rs `Domain::nearest_to`).
            let Some(d) = Terrain::pyramid().domain() else { return };
            let mut near: Vec<(f64, Chunk, bool)> = wanted.iter().filter(|(k, _)| !self.mounted.contains_key(k)).map(|(&k, &root)| (d.nearest_to(k), k, root)).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            self.to_mount.clear();
            self.to_mount.extend(near.into_iter().map(|(_, k, root)| (k, root)));
            self.selected_for = Some(key);
        }

        let mut mounts = 0;
        while let Some(&(k, root)) = self.to_mount.front() {
            // At the ceiling a stale chunk gives up its place; only when
            // there is none does the pass stop, which is a table holding
            // nothing but what the camera wants and still wanting more.
            if self.slots.len() >= MAX_CHUNKS && !self.give_room(world, emit) {
                self.census.mount_full = 1;
                self.view_dirty = true;
                return;
            }
            if mounts >= MAX_MOUNTS_PER_FRAME {
                // The rest next frame, from where this one stopped.
                self.view_dirty = true;
                return;
            }
            self.to_mount.pop_front();
            mounts += 1;
            let id = self.next_mount_id;
            self.next_mount_id = self.next_mount_id.wrapping_add(1).max(MOUNT_ID_BASE);
            emit(Event::Mount {
                id,
                page: k.page,
                base_x: k.bx,
                base_y: k.by,
            });
            self.add(id, k, root);
            self.mounted.insert(k, id);
        }
    }

    /// Lets go of the stale chunks whose ground is ready to be drawn by what
    /// replaced them: wanted chunks cover all of it, and every one of them
    /// has a layout or is off screen and never will. A stale chunk wholly
    /// outside what the window draws — the view has moved on — goes at once;
    /// one inside it that nothing covers yet is ground whose pages have not
    /// come, and it stays.
    /// Stops every chunk of a page whose record has gone, before anything of
    /// the tick can build one: stale, its scan and bake given up, what it has
    /// built still drawn (`page` says why).
    fn stop_evicted(&mut self, world: &mut World) {
        if self.evicted.is_empty() {
            return;
        }
        for (k, &id) in self.mounted.iter() {
            if self.evicted.contains(&k.page) {
                if let Some(&i) = self.index.get(&id) {
                    self.slots[i].set(STALE, true);
                    self.builder.want(world, id, [0, 0]);
                }
            }
        }
        self.evicted.clear();
    }

    /// The device's meshes are gone — the arena was made anew, the device with
    /// it — so every tier is forgotten and built again from the records.
    fn lose_meshes(&mut self, emit: &mut impl FnMut(Event)) {
        let epoch = arena_epoch();
        if epoch == self.meshes.epoch {
            return;
        }
        self.meshes.epoch = epoch;
        self.meshes.holder.fill(0);
        for s in self.slots.iter_mut() {
            for t in core::mem::take(&mut s.tiers) {
                emit(Event::Evict {
                    id: s.id,
                    step: t.step,
                    generation: t.generation,
                });
            }
        }
        self.view_dirty = true;
    }

    fn retire(&mut self, world: &mut World, emit: &mut impl FnMut(Event)) {
        self.gone.clear();
        self.kept = (0, 0, 0);
        let waiters = self.meshes.waiters;
        let waiters = &waiters[..core::mem::take(&mut self.meshes.waiter_cnt)];
        // What covers each stale chunk, from the chunk tree rather than every
        // pair: the wanted chunks are a cut through it, so what covers a
        // stale chunk's ground is a wanted chunk above it, whole, or the
        // wanted chunks below it, each its own area. Each chunk walks up its
        // ancestors once — ten at most — and meets the other kind there.
        for s in self.slots.iter_mut().filter(|s| s.has(STALE)) {
            s.cover = (0.0, true, false);
        }
        let (slots, index, mounted) = (&mut self.slots, &self.index, &self.mounted);
        let at = |k: &Chunk| mounted.get(k).and_then(|id| index.get(id)).copied();
        for (&k, &id) in mounted.iter() {
            let Some(&i) = index.get(&id) else { continue };
            let s = &slots[i];
            let laid = s.layout_ready || !s.has(IN_FRUSTUM);
            let waiting = !laid && waiters.contains(&id);
            if s.has(STALE) {
                // A wanted chunk above covers it whole.
                if let Some(j) = k.ancestors().filter_map(|a| at(&a)).find(|&j| !slots[j].has(STALE)) {
                    let w = &slots[j];
                    let laid = w.layout_ready || !w.has(IN_FRUSTUM);
                    slots[i].cover = (f64::INFINITY, laid, !laid && waiters.contains(&w.id));
                }
                continue;
            }
            let area = s.span * s.span;
            // Its area to every stale chunk above it: one it covers part of.
            for j in k.ancestors().filter_map(|a| at(&a)) {
                let w = &mut slots[j];
                if w.has(STALE) && w.cover.0.is_finite() {
                    w.cover = (w.cover.0 + area, w.cover.1 && laid, w.cover.2 || waiting);
                }
            }
        }
        for (&k, &id) in self.mounted.iter() {
            let Some(&i) = self.index.get(&id) else { continue };
            let s = &self.slots[i];
            if !s.has(STALE) {
                continue;
            }
            let rect = k.rect();
            let (covered, ready, on_waiter) = s.cover;
            // What has to be covered is the ground this chunk still shows —
            // its own against what the rings draw — and not its whole extent.
            // A zoom in of several levels leaves a stale chunk far wider than
            // the window that replaced it: the wanted chunks cover the little
            // the view has, which is all of it that is seen, while the rest of
            // its extent has no chunks at all and never will. Measured against
            // the whole, that never comes to its area, and neither does the
            // other way out — the view lies inside it, so it never leaves the
            // drawn rect either. Both doors shut, and it stays drawn over
            // ground it no longer describes, for as long as the view is there.
            let gone_from_view = self.drawn.as_ref().is_none_or(|d| !d.iter().any(|&g| overlaps(rect, g)));
            let whole = covered >= self.drawn.as_ref().map_or(0.0, |d| d.iter().map(|&g| shared_area(rect, g)).sum());
            // A stale chunk is kept for what it is showing. One that never
            // got a layout is showing nothing, so there is nothing to keep
            // and no reason to wait on whatever covers it.
            //
            // A zoom that walks levels mounts a window it will leave before
            // anything of it is laid out, and `recheck` stops the work of a
            // chunk the moment it goes stale — so these never had a layout
            // and never will. Held, they waited on the cover being ready,
            // which is to say they left only once the current work was done,
            // which is exactly when the table is fullest: measured through a
            // tilted zoom out, 2,182 stale chunks of which 161 were drawing
            // anything, and 4,054 chunks held against a table of 4,096. The
            // population that is not on the screen is not a cache and not
            // coverage; it is the last epoch's unfinished work outliving its
            // authority.
            let shows_nothing = !s.layout_ready;
            if gone_from_view || shows_nothing || (whole && ready) {
                self.gone.push((k, id));
            } else if !whole {
                self.kept.0 += 1;
            } else {
                self.kept.1 += 1;
                self.kept.2 += on_waiter as u32;
            }
        }
        for g in 0..self.gone.len() {
            let (k, id) = self.gone[g];
            self.mounted.remove(&k);
            self.remove(world, id);
            emit(Event::Unmount { id });
        }
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, commit and reclaim under the
    /// frame's budget, and set what the build task takes. Whether the canvas has something new to draw.
    pub fn tick(&mut self, world: &mut World, frame: Frame, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) -> bool {
        let t0 = now();
        let deadline = t0 + frame.remaining_ms;
        let hard = deadline + frame.hard_slack_ms;
        if !self.frame.is_some_and(|prev| same_view(&prev, &frame)) {
            self.view_dirty = true;
        }
        self.frame = Some(frame);
        // The view decides what every chunk should be showing, so a view
        // that has moved is answered now and not on the next tick of a
        // timer. `view_dirty` is that question outstanding — set by a frame
        // that differs from the last, and by anything else a recheck has not
        // seen — and the interval below is only what catches a change that
        // set no flag.
        self.recheck_elapsed += frame.delta_s;
        let full_recheck = self.view_dirty || self.recheck_elapsed >= RECHECK_INTERVAL_S;
        if full_recheck {
            self.recheck_elapsed = 0.0;
        }

        self.stop_evicted(world);
        self.lose_meshes(emit);
        self.mount(world, emit);
        self.refresh(&frame, emit);
        self.retire(world, emit);
        let mut sc = core::mem::take(&mut self.scratch);
        self.order(t0, &mut sc.order, &mut sc.keys);
        let Scratch {
            order, visible, offscreen, eligible, ..
        } = &mut sc;

        // The pending lists from the flags as the frame found them, before
        // the recheck moves them, as the stage's pass had it.
        visible.clear();
        offscreen.clear();
        for &i in order.iter() {
            let s = &self.slots[i];
            if s.has(PENDING) {
                if s.has(IN_FRUSTUM) {
                    visible.push(i);
                } else {
                    offscreen.push(i);
                }
            }
        }

        // What each chunk should be showing, for every chunk that could have
        // changed: freshly mounted, mid-blend, or settling — and all of them
        // on the interval.
        //
        // Not under the budget, and that is the point. This decides what is
        // *wanted*; the budget is for the work that follows, and a pass that
        // states a demand cannot be cut without some chunks' demand going
        // unstated. Cut, it stopped at the order's head, and a chunk drawing
        // a coarse tier is not a hole — it shows something — so it sorts
        // behind every hole and was never reached: never told it wanted a
        // finer tier, so never pending, so never built. A zoom in along the
        // ground is the worst of it, since a window that wants five levels
        // has four times the chunks and its far half is holes, and the
        // coarse boxes under the camera sat there while the budget went on
        // the horizon.
        //
        // It costs a distance and a few flags a chunk, MAX_CHUNKS of
        // them at the worst, and only on the interval. The FRESH case was
        // already exempted for this exact reason; it is the whole pass that
        // needed to be.
        eligible.clear();
        eligible.extend(order.iter().copied().filter(|&i| full_recheck || self.slots[i].flags & (FRESH | BLEND_ACTIVE | SETTLING) != 0));
        for &i in eligible.iter() {
            self.recheck(world, i, t0, emit);
        }
        if full_recheck {
            self.view_dirty = false;
        }

        let pending_total = (visible.len() + offscreen.len()) as u32;
        // A commit on screen may build decor, and a block refused ends the
        // commits this tick as it ends the builds (Builder::tick_capped):
        // the heap is the same until reclaim, and the decor is scattered
        // before its block is asked for.
        let starved = self.builder.stat.starved;
        let commits = under_budget(visible, MAX_COMMITS_PER_FRAME, deadline, hard, now, |i| {
            self.builder.stat.starved == starved && self.commit(world, i, emit)
        });
        // What the pass never looked at, which is what a cap on how many may
        // land costs when there are more of them than it lets through.
        let commit_left = visible.len() as u32 - commits.visited;
        let mut committed = commits.ran;
        let releases = if self.refused_last { MAX_RELEASES_WHEN_REFUSED } else { MAX_RELEASES_PER_FRAME };
        committed += under_budget(offscreen, releases, deadline, hard, now, |i| self.commit(world, i, emit)).ran;

        // The builds are the build task's (`build_step`), on the turn's quota:
        // the visible pending, in the one order.
        //
        // A chunk whose sources had changed used to be held in a list of its
        // own and put at the front of this, which meant in the order its page
        // arrived — so a page answering for ground at the horizon sent its
        // sixty-four chunks ahead of everything under the camera, and what
        // filled the screen was the order the network replied in. The class
        // was real and is kept; it is a term of `priority` now (`restale`),
        // under the holes and over the refinements, where the distance
        // decides within it.
        let slots = &self.slots;
        self.builds.clear();
        self.builds.extend(visible.iter().filter(|&&i| slots[i].has(PENDING)).map(|&i| slots[i].id));
        if !self.builds.is_empty() {
            builds().notify();
        }
        committed += core::mem::take(&mut self.build_committed);

        let freed = self.reclaim(world, order, emit);
        self.scratch = sc;
        self.press(world, t0, freed);
        let animating = self.models.tick(world, &mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
        self.draw_frame(&frame, world_mem(world));

        let mut c = Census {
            chunks: self.slots.len() as u32,
            commit_left,
            mount_left: self.to_mount.len() as u32,
            mount_full: self.census.mount_full,
            ..Census::default()
        };
        for s in &self.slots {
            if s.has(IN_FRUSTUM) {
                c.in_frustum += 1;
            }
            if s.has(SETTLING) {
                c.settling += 1;
            }
            if s.has(BLEND_ACTIVE) {
                c.blend_active += 1;
            }
            if s.has(PENDING) {
                c.pending += 1;
            }
            if s.has(STALE) {
                c.stale += 1;
                if s.has(IN_FRUSTUM) && s.layout_ready {
                    c.stale_drawn += 1;
                    c.stale_levels |= 1 << s.level;
                }
            }
            if s.has(PENDING) && s.pending_since >= 0.0 {
                let waited = t0 - s.pending_since;
                if s.has(IN_FRUSTUM) {
                    c.oldest_visible_ms = c.oldest_visible_ms.max(waited);
                } else {
                    c.offscreen_pending += 1;
                    c.oldest_release_ms = c.oldest_release_ms.max(waited);
                }
            }
        }
        c.released = self.census.released;
        c.hidden = self.hidden;
        c.level_boxes = self.level_boxes;
        c.slot_waits = core::mem::take(&mut self.meshes.waits);
        c.cut = core::mem::take(&mut self.meshes.cut);
        c.fine_peak = core::mem::take(&mut self.meshes.fine_peak);
        c.coarse_peak = core::mem::take(&mut self.meshes.coarse_peak);
        c.wait_levels = core::mem::take(&mut self.meshes.wait_levels);
        c.holder_levels = core::mem::take(&mut self.meshes.holder_levels);
        (c.stale_uncovered, c.stale_unready, c.stale_on_waiter) = self.kept;
        c.retained_on = self.retained_on;
        c.retained_off = self.retained_off;
        c.pressure = self.pressure;
        let blending = c.blend_active > 0;
        self.census = c;
        committed > 0 || pending_total > committed || blending || self.view_dirty || animating
    }

    /// This frame's draws: every chunk on screen with a layout, its tiers at
    /// the weights the blend decided, placed where the chunk is in the frame
    /// the camera is written in.
    fn draw_frame(&mut self, frame: &Frame, mem: *const u8) {
        self.draw.begin_frame();
        self.hidden = 0;
        self.level_boxes = [0; LEVELS];
        self.ghost.draw(&mut self.draw, frame.origin);
        self.models.draw(&mut self.draw, frame.origin);
        self.markers.draw(&mut self.draw, frame.origin);
        self.debug.draw(&mut self.draw, frame.origin);
        // A stale chunk is drawn until what replaced it is whole, and what
        // replaced it waits under it: never both on one piece of ground. What
        // shares ground with a chunk is above or below it in the chunk tree,
        // so each chunk walks up once and marks the wanted chunk it meets
        // over a drawn stale one, or itself under one.
        for s in self.slots.iter_mut() {
            s.set(COVERED, false);
        }
        let (slots, index, mounted) = (&mut self.slots, &self.index, &self.mounted);
        let at = |k: &Chunk| mounted.get(k).and_then(|id| index.get(id)).copied();
        let drawn = |s: &Slot| s.has(STALE) && s.has(IN_FRUSTUM) && s.layout_ready;
        for (&k, &id) in mounted.iter() {
            let Some(&i) = index.get(&id) else { continue };
            if drawn(&slots[i]) {
                for j in k.ancestors().filter_map(|a| at(&a)) {
                    if !slots[j].has(STALE) {
                        slots[j].set(COVERED, true);
                    }
                }
            } else if !slots[i].has(STALE) && k.ancestors().filter_map(|a| at(&a)).any(|j| drawn(&slots[j])) {
                slots[i].set(COVERED, true);
            }
        }
        let holder = &self.meshes.holder;
        for s in &self.slots {
            if !s.has(IN_FRUSTUM) || !s.layout_ready {
                continue;
            }
            if s.has(COVERED) {
                self.hidden += 1;
                continue;
            }
            let (wp, ws) = s.told.map_or((1.0, 0.0), |t| (t.0, t.1));
            let translation = DrawList::translation([s.base_x, s.base_y, 0.0], frame.origin);
            for (step, visible) in [(s.layout_primary, wp), (s.layout_secondary, ws)] {
                if step == 0 {
                    continue;
                }
                let Some(t) = s.tier(step, s.layout_generation) else { continue };
                // A chunk draws only from a slot it holds. The slot is room
                // now and not an address (world/chunk/mesh_slot.rs), so it is
                // handed from one chunk to another while the kernel runs —
                // and a long-lived thing holding a recyclable address is how
                // one chunk comes to draw another's ground. It cannot here:
                // the only reference to a slot is the `Tier` inside the chunk
                // that took it, taking the slot removes that tier in the same
                // step, and this looks the tier up again every frame. The
                // assert is what says so out loud, at an array index a draw.
                assert_eq!(holder[t.slot.index], s.id, "a chunk drew from a slot another holds");
                if t.box_cnt == 0 {
                    continue;
                }
                // A mesh's heights are steps from its own floor, so the
                // floor rides in the translation with the chunk's corner.
                let at = [translation[0], translation[1], translation[2] + t.heights.floor as f32];
                self.draw.draw_arena(
                    t.slot.group,
                    t.slot.first_box,
                    t.box_cnt,
                    at,
                    visible,
                    (step << s.level) as f32,
                    t.heights.step as f32,
                    group_is_fine(t.slot.group),
                );
                self.level_boxes[s.level as usize] += t.box_cnt;
            }
            // Decor takes whichever tier the blend says the chunk is mostly
            // on: an instance is drawn or it is not.
            let decor_step = match s.told {
                Some((_, _, true)) if s.layout_secondary != 0 => s.layout_secondary,
                _ => s.layout_primary,
            };
            if let Some(d) = s.decor_of(decor_step, s.layout_generation) {
                for (group, (offset, count)) in d.groups(mem).enumerate() {
                    if count > 0 {
                        self.draw.draw_decor(d.handle, group as u32, count, offset, translation);
                    }
                }
            }
        }
    }

    /// Every chunk drawn now, as the block of the pyramid's cells it draws.
    /// During a crossfade both tiers are drawn and the one covering more is
    /// the chunk's; a chunk covered by another, off the screen or not laid
    /// out draws nothing a layer could stand on.
    pub fn drawn(&self) -> impl Iterator<Item = Drawn<'_>> {
        self.slots.iter().filter_map(|s| self.drawn_of(s))
    }

    fn drawn_of<'a>(&'a self, s: &'a Slot) -> Option<Drawn<'a>> {
        if !s.has(IN_FRUSTUM) || s.has(COVERED) || !s.layout_ready {
            return None;
        }
        let step = match s.told {
            Some((_, _, true)) if s.layout_secondary != 0 => s.layout_secondary,
            _ => s.layout_primary,
        };
        let t = s.tier(step, s.layout_generation)?;
        Some(Drawn {
            level: s.level,
            base: ivec::gather(s.base_x as i64, s.base_y as i64, 0),
            step,
            tops: &self.meshes.surface[t.slot.index],
            heights: t.heights,
        })
    }

    /// The ground drawn now at `addr`: the level of the cells it is drawn in
    /// there, and the top of that cell's surface in units. Read from the
    /// chunk `drawn` gives for that ground, so it is the surface on the
    /// screen — the tier a crossfade shows more of, a stale chunk while its
    /// replacement builds — and none where nothing drawn stands there.
    pub fn surface_at(&self, at: &Pvf3) -> Option<(u32, f64)> {
        (0..=TOP_LEVEL).find_map(|level| {
            let id = self.mounted.get(&Chunk::of(level, at))?;
            let d = self.drawn_of(&self.slots[*self.index.get(id)?])?;
            let cell = Cell::of(d.cell_level(), at);
            let first = Cell::of(d.cell_level(), &Pvf3 { i: d.base, ..Default::default() });
            let at = |i: u32| i % PAGE_CELLS;
            let (bx, by) = (at(cell.index()).wrapping_sub(at(first.index())), (cell.index() / PAGE_CELLS).wrapping_sub(first.index() / PAGE_CELLS));
            Some((d.cell_level(), d.top(bx, by)?))
        })
    }

    /// Where every chunk is and whether it is on screen, measured against the
    /// frame. A slot that crosses the frustum's edge moves its retained bytes
    /// between the on-screen and off-screen totals and says so with a
    /// `Visible` event, which is what the retention budget is spent from.
    fn refresh(&mut self, frame: &Frame, emit: &mut impl FnMut(Event)) {
        let domain = Terrain::pyramid().domain();
        for i in 0..self.slots.len() {
            let visible = measure(frame, &self.slots[i]).0;
            let near = domain.map_or(f64::INFINITY, |d| d.nearest_to(self.slots[i].chunk));
            let s = &mut self.slots[i];
            s.distance2 = near * near;
            let was = s.has(IN_FRUSTUM);
            if visible != was {
                let bytes = s.retained();
                s.set(IN_FRUSTUM, visible);
                if visible {
                    self.retained_off -= bytes;
                    self.retained_on += bytes;
                } else {
                    self.retained_on -= bytes;
                    self.retained_off += bytes;
                }
                emit(Event::Visible { id: s.id, on: visible });
                self.view_dirty = true;
            }
        }
    }

    /// Every slot in priority order (`priority`), the keys aged once rather
    /// than in the comparator.
    /// Every slot's index by priority into `idx`, the keys through `keys`.
    fn order(&self, now: f64, idx: &mut Vec<usize>, keys: &mut Vec<Keyed>) {
        let holders = self.builder.holders();
        keys.clear();
        keys.extend(self.slots.iter().map(|s| {
            Priority {
                holds: holders.contains(&Some(s.id)),
                in_frustum: s.has(IN_FRUSTUM),
                hole: s.has(EMPTY) && !s.has(COVERED),
                restale: s.layout_ready && s.layout_generation != self.builder.generation_of(s.id).unwrap_or(0),
                distance2: s.distance2,
                pending_since: s.pending_since,
            }
            .keyed(now)
        }));
        idx.clear();
        idx.extend(0..self.slots.len());
        // Ties broken by the slot, which is the order a stable sort keeps: the
        // same order, with no merge buffer taken each tick.
        idx.sort_unstable_by(|&a, &b| priority_keyed(&keys[a], &keys[b]).then(a.cmp(&b)));
    }

    fn set_pending(&mut self, i: usize, pending: bool, request: i32, now: f64) {
        let s = &mut self.slots[i];
        if pending && (!s.has(PENDING) || request != s.request) {
            s.pending_since = now;
        }
        if !pending {
            s.pending_since = -1.0;
        }
        s.request = request;
        s.set(PENDING, pending);
    }

    fn stop_work(&mut self, world: &mut World, i: usize) {
        self.builder.want(world, self.slots[i].id, [0, 0]);
    }

    fn decide(&mut self, i: usize, primary: f32, secondary: f32, emit: &mut impl FnMut(Event)) {
        let coverage = |v: f32| if v > 1.5 { 1.0 - (v - 2.0) } else { v };
        let decor_secondary = self.slots[i].layout_secondary != 0 && coverage(secondary) > coverage(primary);
        let s = &mut self.slots[i];
        let told = (primary, secondary, decor_secondary);
        if s.told == Some(told) {
            return;
        }
        s.told = Some(told);
        emit(Event::Blend {
            id: s.id,
            primary,
            secondary,
            decor_secondary,
        });
    }

    /// The tier this slot should be showing, and whether that makes it
    /// pending.
    ///
    /// Off screen it wants the coarsest tier and a release. On screen the
    /// rule answers for its nearest distance (`wanted_step`), and the slot is
    /// pending when what it shows is not that.
    /// SETTLING is pending with a layout already up, so a frame can tell a
    /// chunk that is changing tier from one that has nothing to show yet.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64, emit: &mut impl FnMut(Event)) {
        self.slots[i].set(FRESH, false);
        // A stale chunk shows what it shows until it is let go: nothing
        // more is built for it.
        if self.slots[i].has(STALE) {
            self.set_pending(i, false, 0, now);
            self.stop_work(world, i);
            return;
        }
        if !self.slots[i].has(IN_FRUSTUM) {
            self.slots[i].desired = (COARSEST, 0);
            let ready = self.slots[i].layout_ready;
            self.set_pending(i, ready, RELEASE_REQUEST, now);
            if !self.slots[i].has(PENDING) {
                self.stop_work(world, i);
            }
            let s = &mut self.slots[i];
            s.set(BLEND_ACTIVE, false);
            s.set(SETTLING, false);
            self.decide(i, 1.0, 0.0, emit);
            return;
        }
        let s = &self.slots[i];
        let (level, chunk) = (s.level, s.chunk);
        let outermost = self.outermost(i);
        let Some(d) = Terrain::pyramid().domain() else { return };
        let primary = relative(wanted_step(d, chunk, self.pressure), level, outermost);
        self.slots[i].desired = (primary, 0);
        let current = self.generation(i);
        let s = &self.slots[i];
        let pending = !s.layout_ready || s.layout_primary != primary || s.layout_secondary != 0 || s.layout_generation != current;
        self.set_pending(i, pending, build_request(primary, 0), now);
        let s = &mut self.slots[i];
        let settling = s.has(PENDING) && s.layout_ready;
        s.set(SETTLING, settling);
        s.set(BLEND_ACTIVE, false);
        if !s.has(PENDING) {
            self.stop_work(world, i);
        }
        self.decide(i, 1.0, 0.0, emit);
    }

    /// Whether the chunk nodes hold every tier of (primary, secondary) of the
    /// current generation. When not, the builder is asked for every tier the
    /// chunk lacks, those two first: its runs are let go once its tiers are
    /// baked (node.rs), so a tier asked later would cost the runs again.
    fn ask(&mut self, world: &mut World, i: usize, primary: u32, secondary: u32) -> bool {
        let generation = self.generation(i);
        let outermost = self.outermost(i);
        let s = &self.slots[i];
        let lacks = |st: u32| st != 0 && s.tier(st, generation).is_none();
        if !lacks(primary) && !lacks(secondary) {
            self.builder.want(world, s.id, [0, 0]);
            return true;
        }
        // What a chunk keeps besides what it shows: a leaf its own cells and
        // the step above, a level chunk the one step it is ever drawn at. The
        // outermost ring is drawn coarser than that (`relative`), and a step
        // finer than what it shows is a bake for nothing, so it keeps none.
        let tiers = match (s.level, outermost) {
            (0, _) => [1, 2],
            (_, true) => [0, 0],
            (_, false) => [2, 0],
        };
        let mut want = [0; 2];
        let mut n = 0;
        for st in [primary, secondary].into_iter().chain(tiers) {
            if n == want.len() {
                break;
            }
            if lacks(st) && !want.contains(&st) {
                want[n] = st;
                n += 1;
            }
        }
        self.builder.want(world, s.id, want);
        false
    }

    /// Shows (primary, secondary) of `generation`. A ready layout of a newer
    /// generation than the one shown lets every older tier and decor go in the
    /// same step, so what is drawn changes whole.
    fn apply_layout(&mut self, i: usize, primary: u32, secondary: u32, generation: u32, ready: bool, emit: &mut impl FnMut(Event)) {
        if ready && self.slots[i].layout_generation != generation {
            self.drop_older(i, generation, emit);
        }
        let s = &mut self.slots[i];
        s.layout_primary = primary;
        s.layout_secondary = if secondary == primary { 0 } else { secondary };
        if ready {
            s.layout_generation = generation;
        }
        s.layout_ready = ready;
        s.set(EMPTY, !ready);
        s.told = None;
        if ready {
            let mut b: Option<Bounds> = None;
            for st in [s.layout_primary, s.layout_secondary] {
                if let Some(Some(t)) = s.tier(st, generation).map(|t| t.bounds) {
                    b = Some(match b {
                        None => t,
                        Some(o) => Bounds {
                            min: [o.min[0].min(t.min[0]), o.min[1].min(t.min[1]), o.min[2].min(t.min[2])],
                            max: [o.max[0].max(t.max[0]), o.max[1].max(t.max[1]), o.max[2].max(t.max[2])],
                        },
                    });
                }
            }
            match b {
                Some(b) => {
                    let d = [(b.max[0] - b.min[0]) as f64, (b.max[1] - b.min[1]) as f64, (b.max[2] - b.min[2]) as f64];
                    s.bc = [(b.min[0] + b.max[0]) as f64 * 0.5, (b.min[1] + b.max[1]) as f64 * 0.5, (b.min[2] + b.max[2]) as f64 * 0.5];
                    s.radius = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() * 0.5;
                }
                None => s.unmeasured(),
            }
        }
        emit(Event::Layout {
            id: s.id,
            primary: s.layout_primary,
            secondary: s.layout_secondary,
            generation: s.layout_generation,
            ready,
        });
    }

    /// Lets go of every tier and decor of slot `i` older than `generation`.
    fn drop_older(&mut self, i: usize, generation: u32, emit: &mut impl FnMut(Event)) {
        let before = self.slots[i].retained();
        let s = &mut self.slots[i];
        let (id, draw) = (s.id, &mut self.draw);
        s.tiers.retain(|t| {
            let keep = t.generation == generation;
            if !keep {
                emit(Event::Evict {
                    id,
                    step: t.step,
                    generation: t.generation,
                });
            }
            keep
        });
        s.decor.retain(|d| {
            let keep = d.generation == generation;
            if !keep {
                draw.free(d.handle);
                emit(Event::Evict {
                    id,
                    step: d.step,
                    generation: d.generation,
                });
            }
            keep
        });
        self.account(i, before);
    }

    /// Lets go of tiers past MAX_CACHED_TIERS, oldest built first, never the
    /// active ones or the off-screen tier.
    fn trim(&mut self, i: usize, active: [u32; 2], emit: &mut impl FnMut(Event)) {
        let before = self.slots[i].retained();
        let s = &mut self.slots[i];
        let mut k = 0;
        while s.tiers.len() > MAX_CACHED_TIERS && k < s.tiers.len() {
            let (st, g) = (s.tiers[k].step, s.tiers[k].generation);
            if st == COARSEST || active.contains(&st) {
                k += 1;
                continue;
            }
            s.tiers.remove(k);
            let draw = &mut self.draw;
            s.decor.retain(|d| {
                let hit = d.step == st && d.generation == g;
                if hit {
                    draw.free(d.handle);
                }
                !hit
            });
            emit(Event::Evict { id: s.id, step: st, generation: g });
        }
        self.account(i, before);
    }

    /// Applies what `recheck` asked for, if it can: off screen, the coarsest
    /// tier and the memory back; on screen, the desired layout once every
    /// tier it names is built. A request that was refused for want of a block
    /// is not retried until the heap has released something, so a full heap
    /// costs one attempt rather than one per frame.
    fn commit(&mut self, world: &mut World, i: usize, emit: &mut impl FnMut(Event)) -> bool {
        let s = &self.slots[i];
        if !s.has(PENDING) {
            return false;
        }
        let in_frustum = s.has(IN_FRUSTUM);
        if (s.request == RELEASE_REQUEST) == in_frustum {
            self.stop_work(world, i);
            return false;
        }
        if !in_frustum {
            self.stop_work(world, i);
            let g = self.slots[i].layout_generation;
            self.apply_layout(i, COARSEST, 0, g, false, emit);
            self.set_pending(i, false, RELEASE_REQUEST, 0.0);
            self.census.released += 1;
            return true;
        }
        let (p, q) = s.desired;
        if s.refused.is_some_and(|r| r == crate::host::heap_stat().releases) {
            return false;
        }
        let asked = self.ask(world, i, p, q);
        let generation = self.generation(i);
        if !asked || !self.ensure_decor(world, i, [p, q], generation, emit) {
            return false;
        }
        self.trim(i, [p, q], emit);
        self.apply_layout(i, p, q, generation, true, emit);
        self.set_pending(i, false, RELEASE_REQUEST, 0.0);
        true
    }

    /// The retain budget: off-screen decor past it, or the heap past
    /// HEAP_PRESSURE, is given back farthest first. A block refused since the
    /// last pass is pressure too, whatever the numbers say: the heap has room
    /// in bytes but not in one piece, and what gives it back in pieces that
    /// join is the off-screen decor, farthest first. Decor is all a chunk
    /// holds in the heap: its runs are `.fixed.runs`'s and its tiers are on
    /// the device, where a slot is taken over rather than given back.
    fn reclaim(&mut self, world: &mut World, order: &[usize], emit: &mut impl FnMut(Event)) -> u32 {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let refused = self.builder.stat.starved != self.refused_seen;
        self.refused_seen = self.builder.stat.starved;
        self.refused_last = refused;
        let pressed = heap_pressed;
        let cap = if refused { MAX_RECLAIMS_WHEN_REFUSED } else { MAX_RECLAIMS_PER_FRAME };
        let mut freed = 0;
        let _ = world;
        // The order already puts off-screen chunks farthest first.
        for &i in order {
            if freed >= cap || !(refused || self.retained_off > budget || pressed(world)) {
                break;
            }
            if self.slots[i].has(IN_FRUSTUM) || self.slots[i].retained() <= 0.0 {
                continue;
            }
            let before = self.slots[i].retained();
            self.drop_decor(i, |_, _| false, emit);
            self.account(i, before);
            freed += 1;
        }
        if !refused || freed >= cap {
            return freed;
        }
        // Nothing off screen gave enough, or nothing was off screen: the
        // decor of steps an on-screen chunk does not show is given back,
        // farthest first, from chunks not mid-build, until the frame's share
        // is spent.
        for &i in order.iter().rev() {
            if freed >= cap {
                break;
            }
            let s = &self.slots[i];
            if !s.has(IN_FRUSTUM) || s.has(PENDING) {
                continue;
            }
            let (p, q, g) = (s.layout_primary, s.layout_secondary, s.layout_generation);
            let shown = move |step: u32, generation: u32| generation == g && (step == p || step == q);
            if s.decor.iter().all(|d| shown(d.step, d.generation)) {
                continue;
            }
            let before = s.retained();
            self.drop_decor(i, shown, emit);
            self.account(i, before);
            freed += 1;
        }
        freed
    }

    /// Gives back chunk `i`'s decor save what `keep` names.
    fn drop_decor(&mut self, i: usize, keep: impl Fn(u32, u32) -> bool, emit: &mut impl FnMut(Event)) {
        let s = &mut self.slots[i];
        let id = s.id;
        let draw = &mut self.draw;
        s.decor.retain(|d| {
            let kept = keep(d.step, d.generation);
            if !kept {
                draw.free(d.handle);
                emit(Event::Evict {
                    id,
                    step: d.step,
                    generation: d.generation,
                });
            }
            kept
        });
    }

    /// The byte budget paid in LOD (MAX_PRESSURE): a step in when a block
    /// was refused and reclaim gave nothing back, a step out when the
    /// refusals have stopped and the section is off its mark. A change is
    /// a new criterion, and every chunk is rechecked against it.
    fn press(&mut self, world: &World, now: f64, freed: u32) {
        if self.refused_last {
            self.refused_at = now;
        }
        let pressure = self.pressure;
        let next = if self.refused_last && freed == 0 && pressure < MAX_PRESSURE && now - self.pressure_at >= PRESSURE_STEP_MS {
            pressure + 1
        } else if pressure > 0 && now - self.refused_at >= PRESSURE_RELIEF_MS && now - self.pressure_at >= PRESSURE_RELIEF_MS && !heap_pressed(world) {
            pressure - 1
        } else {
            return;
        };
        self.pressure = next;
        self.pressure_at = now;
        self.recheck_elapsed = RECHECK_INTERVAL_S;
        self.view_dirty = true;
    }
}

/// Whether the heap is live past HEAP_PRESSURE.
fn heap_pressed(_world: &World) -> bool {
    let h = crate::host::heap_stat();
    h.live as f64 > h.range as f64 * HEAP_PRESSURE
}

/// Ids the scheduler gives the chunks it mounts.
const MOUNT_ID_BASE: u32 = 1 << 31;
/// Whether two frames look at the world the same way: what a recheck's
/// answer depends on, not the budget or the elapsed time.
fn same_view(a: &Frame, b: &Frame) -> bool {
    a.root == b.root
        && a.origin == b.origin
        && a.planes == b.planes
        && a.camera == b.camera
        && a.viewport_h == b.viewport_h
        && a.fov_deg == b.fov_deg
        && a.pixel_dither == b.pixel_dither
        && a.show_decor == b.show_decor
}

/// The world step the rule wants `c` shown at: the level wanted at its
/// nearest point (window.rs `split_distance`), as that level's cells averaged
/// 2×2 — step 2^(want + 1) — and coarser by the byte budget's `pressure`.
/// Nearer than level 0's split distance the rule wants a level the ground
/// has no data for, and the leaves' own cells are what there is.
fn wanted_step(d: &Domain, c: Chunk, pressure: u32) -> u32 {
    let want = (d.nearest_to(c) / split_distance(0)).log2().floor().max(-1.0) as i32 + pressure as i32;
    1 << (want + 1).clamp(0, (COARSEST.trailing_zeros() + TOP_LEVEL) as i32)
}

/// A world step as a chunk of `level` builds it: its own cells are 2^level,
/// and it draws them averaged 2×2, its second tier, the step the level above
/// would otherwise draw as its vote (level.rs `level_at`). The leaves alone
/// are drawn as they are too, their first tier, and blend between the two.
/// Finer than that a chunk is not drawn — the split serves it — and coarser
/// it is merged into its parent.
fn relative(world_step: u32, level: u32, outermost: bool) -> u32 {
    // A level draws its own cells one step coarser than they are, and no
    // coarser: step 4 of level k is the ground level k+1 draws at step 2, and
    // that ring is the one holding it. The outermost ring has no level over
    // it, so nothing else would draw that ground — there the rule is
    // followed as far as it goes, which is what makes a far ring cheap
    // (issue/20260923-the-ground-runs-to-the-horizon.md).
    let cap = if outermost { COARSEST } else { 2 };
    (world_step >> level).clamp(if level == 0 { 1 } else { 2 }, cap)
}

/// What a chunk's place in the order is decided by. `hole`: nothing is drawn
/// on its ground, neither its own layout nor a stale chunk it replaces.
#[derive(Clone, Copy)]
pub struct Priority {
    pub holds: bool,
    pub in_frustum: bool,
    pub hole: bool,
    /// Showing a generation older than its sources: the ground it draws is
    /// not what the page under it now says. Worse than a chunk asking only
    /// to be sharper, better than one showing nothing at all.
    pub restale: bool,
    /// The nearest distance from the eye to its ground, squared.
    pub distance2: f64,
    pub pending_since: f64,
}

/// The order work is taken in: the slots' holders; then what is on screen,
/// every hole first, then every chunk showing a generation its sources have
/// moved past, then the ones asking only to be sharper, as progressive
/// refinement goes — a coarse chunk a zoom out has just brought covers ground
/// nothing shows, a fine one ground a stale chunk still shows; within each,
/// the nearest the eye, by the distance its level was chosen by, so the work
/// follows the rule that asked for it; then what is off screen farthest first — a release
/// takes memory from what is least likely to be wanted again. A wait halves
/// the distance key every AGE_HALF_MS, within its class.
///
/// A plain distance and not one in the chunk's own sides. That was "the
/// biggest on the screen first", and it is the same thing for every level:
/// the rule puts a chunk where its own cells are the right size, so a
/// coarse one at the horizon subtends about what a fine one under the camera
/// does, and they came out equal. The screen then filled everywhere at once
/// rather than from the middle — and everywhere at once means the middle
/// waits for all of it. At the two hundred chunks a second the builder
/// manages when it is behind, a five-level window's two thousand three
/// hundred is eleven seconds; its innermost ring alone is under three.
pub fn priority(a: &Priority, b: &Priority, now: f64) -> core::cmp::Ordering {
    priority_keyed(&a.keyed(now), &b.keyed(now))
}

/// A priority with its distance key aged for one `now`.
#[derive(Clone, Copy)]
pub struct Keyed {
    p: Priority,
    key: f64,
}

impl Priority {
    pub fn keyed(self, now: f64) -> Keyed {
        let d2 = self.distance2;
        let key = if self.pending_since < 0.0 {
            d2
        } else {
            d2 * 2f64.powf(-2.0 * (now - self.pending_since) / AGE_HALF_MS)
        };
        Keyed { p: self, key }
    }
}

fn priority_keyed(a: &Keyed, b: &Keyed) -> core::cmp::Ordering {
    b.p.holds
        .cmp(&a.p.holds)
        .then(b.p.in_frustum.cmp(&a.p.in_frustum))
        .then_with(|| if a.p.in_frustum { b.p.hole.cmp(&a.p.hole) } else { core::cmp::Ordering::Equal })
        .then_with(|| if a.p.in_frustum { b.p.restale.cmp(&a.p.restale) } else { core::cmp::Ordering::Equal })
        .then_with(|| if a.p.in_frustum { a.key.total_cmp(&b.key) } else { b.key.total_cmp(&a.key) })
}

/// Where a slot is from a frame: on screen, and its world centre.
fn measure(frame: &Frame, s: &Slot) -> (bool, V3) {
    let [x, y, z] = frame.world_of([s.base_x + s.bc[0], s.base_y + s.bc[1], s.bc[2]]);
    // Where the renderer draws it: laid back on the planet about the view
    // centre, the world's origin here, in the map's own axes — the view's
    // turned back by the azimuth and forward again. A far chunk culled where
    // the flat world has it would leave a hole at the horizon. The root's
    // first column is the azimuth's cosine and sine.
    let [x, y, z] = match frame.globe {
        Some(globe) => globe.place_turned([x, y, z], frame.root[1], frame.root[0]),
        None => [x, y, z],
    };
    let c = frame.camera;
    let half = s.span * 0.5;
    let mut visible = (c[0] - x).abs() <= half && (c[1] - y).abs() <= half;
    if !visible {
        visible = true;
        for p in frame.planes.as_chunks::<4>().0 {
            if p[0] * x + p[1] * y + p[2] * z + p[3] < -s.radius {
                visible = false;
                break;
            }
        }
        // Past the horizon the planet itself is in the way, and nothing draws
        // it: ground there would come up under the horizon line with no
        // occluder to hide it. A point of a sphere of radius `r` is over the
        // eye's horizon while `E·P >= r²`, the eye and the point taken from
        // the planet's centre — the virtual globe's own cull (Cozzi & Ring),
        // widened by the chunk's radius so a chunk half over it stays.
        if visible {
            if let Some(globe) = frame.globe {
                let r = globe.radius_u();
                let (e, p) = ([c[0], c[1], c[2] + r], [x, y, z + r]);
                let along = e[0] * p[0] + e[1] * p[1] + e[2] * p[2];
                let eye = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
                visible = along + s.radius * eye >= r * r
            }
        }
    }
    (visible, [x, y, z])
}

fn world_mem(world: &World) -> *const u8 {
    world.volumes.mem()
}

/// A slot for a tier of `step` on the chunk at `i`, and whoever it is taken
/// from.
///
/// Free first, then one whose holder is not drawing from it — off screen,
/// or laid out from other tiers than this one. A holder that is on screen
/// and drawing from its slot is the one case nothing may take: its ground
/// has nothing else covering it, and what would cover it is the mesh asking,
/// which cannot land while the holder is drawn.
///
/// Scale used to decide this. It cannot any more and should never have: with
/// a slot handed out rather than derived from a coordinate, two chunks
/// meeting at one say nothing about each other's ground, and what matters is
/// only whether the one holding it is showing it.
fn claim(slots: &[Slot], index: &BTreeMap<u32, usize>, meshes: &Meshes, i: usize, step: u32) -> Option<(MeshSlot, Option<u32>)> {
    let mut spare = None;
    for at in pool(slots[i].level, step) {
        let holder = meshes.holder[at];
        if holder == 0 {
            return Some((slot_at(at), None));
        }
        let Some(&j) = index.get(&holder) else {
            // The chunk that held it is gone; so is its claim.
            return Some((slot_at(at), None));
        };
        let s = &slots[j];
        let showing = s.has(IN_FRUSTUM) && s.layout_ready && s.tiers.iter().any(|t| t.slot.index == at);
        if !showing && spare.is_none() {
            spare = Some((slot_at(at), Some(holder)));
        }
    }
    spare
}

/// Records what a build handed over on the slot it belongs to, and passes
/// it on.
fn self_record(slots: &mut [Slot], index: &BTreeMap<u32, usize>, mem: *const u8, meshes: &mut Meshes, e: BuildEvent, emit: &mut impl FnMut(Event)) {
    match e {
        BuildEvent::Tier {
            id,
            generation,
            step,
            at,
            box_cnt,
            heights,
            ..
        } => {
            let Some(&i) = index.get(&id) else { return emit(Event::Build(e)) };
            if slots[i].tier(step, generation).is_some() {
                return emit(Event::Build(e));
            }
            let s = &slots[i];
            // The slot this tier is rebuilt into, or one the pool can give
            // it. A tier keeps the slot it took, so a rebuild writes over its
            // own boxes and nothing else moves.
            let (slot, turned_out) = match s.tiers.iter().find(|t| t.step == step).map(|t| t.slot) {
                Some(slot) => (slot, None),
                None => match claim(slots, index, meshes, i, step) {
                    Some(claimed) => claimed,
                    None => {
                        // Every slot of the pool is held by a chunk drawing
                        // from it. This mesh waits, thrown away and asked for
                        // again, rather than take ground from under one that
                        // has nothing else covering it.
                        meshes.waits += 1;
                        meshes.wait_levels |= 1 << slots[i].level;
                        let named = &meshes.waiters[..meshes.waiter_cnt];
                        if !named.contains(&id) && meshes.waiter_cnt < WAITERS {
                            meshes.waiters[meshes.waiter_cnt] = id;
                            meshes.waiter_cnt += 1;
                        }
                        return;
                    }
                },
            };
            if let Some(holder) = turned_out {
                if let Some(&j) = index.get(&holder) {
                    meshes.holder_levels |= 1 << slots[j].level;
                    let other = &mut slots[j];
                    let mut k = 0;
                    while k < other.tiers.len() {
                        if other.tiers[k].slot.index == slot.index {
                            let t = other.tiers.remove(k);
                            emit(Event::Evict {
                                id: holder,
                                step: t.step,
                                generation: t.generation,
                            });
                        } else {
                            k += 1;
                        }
                    }
                }
            }
            let boxes = box_cnt.min(slot.capacity);
            meshes.cut += box_cnt - boxes;
            let asked = if slot.capacity == FINE_BOXES { &mut meshes.fine_peak } else { &mut meshes.coarse_peak };
            *asked = (*asked).max(box_cnt);
            // SAFETY: the mesh's boxes, a least corner and a size in the
            // first six floats of each, in `.fixed.bake` where the bake
            // just wrote them.
            // The format is the group's, not the mesh's: a slot is a
            // stride into its group's arena (mesh_slot.rs).
            let box_words = group_box_words(slot.group);
            let words = unsafe { core::slice::from_raw_parts(mem.add(at as usize) as *const u32, (boxes * box_words) as usize) };
            let bounds = (boxes > 0).then(|| {
                let mut b = Bounds {
                    min: [f32::INFINITY; 3],
                    max: [f32::NEG_INFINITY; 3],
                };
                let unit = (step << slots[i].level) as f64;
                for r in words.chunks_exact(box_words as usize) {
                    let (min, size) = box_bounds(r, unit, heights);
                    for a in 0..3 {
                        let (lo, size) = (min[a] as f32, size[a] as f32);
                        b.min[a] = b.min[a].min(lo);
                        b.max[a] = b.max[a].max(lo + size);
                    }
                }
                b
            });
            if boxes > 0 {
                let before = arena_epoch();
                // SAFETY: the same words, as bytes.
                let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr() as *const u8, words.len() * 4) };
                // No device to write to: asked for again once there is one.
                if !arena_write(slot.group as usize, slot.first_box * box_words * 4, bytes) {
                    return;
                }
                // The write that made the arena lost nothing written before it.
                if arena_epoch() != before && before == meshes.epoch {
                    meshes.epoch = arena_epoch();
                }
            }
            meshes.surface[slot.index] = surface_centres(words, box_words as usize, step, slots[i].level, heights);
            meshes.holder[slot.index] = id;
            let s = &mut slots[i];
            // A tier of an older generation in the same slot shows these boxes
            // now: they were written over its own.
            for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
                t.box_cnt = boxes;
                t.bounds = bounds;
                t.heights = heights;
            }
            s.tiers.push(Tier {
                step,
                generation,
                bounds,
                slot,
                box_cnt: boxes,
                heights,
            });
        }
    }
    emit(Event::Build(e));
}

/// A cell-centre height query needs no CPU copy of the mesh. Quantized tops
/// use the mesh's own height scale, with zero reserved for an empty centre.
/// Allocation is bounded by mesh slots: 64² words for fine, at most 32²
/// for coarse, independent of source or POI cardinality.
fn surface_centres(words: &[u32], stride: usize, step: u32, level: u32, heights: Heights) -> Box<[u32]> {
    let axis = (CHUNK_CELLS / step) as usize;
    let unit = step as f64 * GROUND.scale(level).side();
    let mut tops = vec![0; axis * axis];
    for r in words.chunks_exact(stride) {
        let (min, size) = box_bounds(r, unit, heights);
        let x0 = ((min[0] / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let y0 = ((min[1] / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let x1 = (((min[0] + size[0]) / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let y1 = (((min[1] + size[1]) / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let top = (r[2] & 0xffff) + (r[2] >> 16) + 1;
        for y in y0..y1 {
            for x in x0..x1 {
                tops[y * axis + x] = tops[y * axis + x].max(top);
            }
        }
    }
    tops.into_boxed_slice()
}

/// A chunk as it is drawn: its level, the unit address its first cell
/// begins at, the step its drawn tier's cells are, and that tier's surface.
/// What a layer standing things on the drawn ground reads (`Scheduler::drawn`).
pub struct Drawn<'a> {
    pub level: u32,
    pub base: IVec,
    step: u32,
    tops: &'a [u32],
    heights: Heights,
}

impl Drawn<'_> {
    /// The level of the cells the chunk is drawn in: a block of 2^s cells of
    /// the chunk's level is a cell s levels up (pxl-spec space/pyramid.rs
    /// `Cell`). The rule's steps are powers of two, so there is no step
    /// this does not name.
    pub fn cell_level(&self) -> u32 {
        debug_assert!(self.step.is_power_of_two(), "a ladder step is a power of two");
        self.level + self.step.trailing_zeros()
    }

    /// The top of the drawn surface at the centre of the `step` block at
    /// (bx, by), in units; none where that centre is empty. Samples share the
    /// GPU mesh slot's lifetime, including a stale holder's.
    pub fn top(&self, bx: u32, by: u32) -> Option<f64> {
        let axis = CHUNK_CELLS / self.step;
        let top = (*self.tops.get((by * axis + bx) as usize)?).checked_sub(1)?;
        Some(self.heights.floor + top as f64 * self.heights.step)
    }
}

#[cfg(test)]
#[path = "table.test.rs"]
mod test;
