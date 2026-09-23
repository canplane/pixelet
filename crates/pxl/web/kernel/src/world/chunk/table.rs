// pxl-kernel/world/chunk/table.rs

//! The chunks' scheduler: which chunks are on screen, which tier each shows,
//! what to build and in what order, and what to give back.
//!
//! A tick is four passes over the slots. `mount` decides which chunks exist
//! at all, by range from the anchor; `refresh` measures each against the
//! frame and flips its IN_FRUSTUM; `recheck` asks the ladder for the tier
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

use std::collections::{BTreeMap, BTreeSet};

use pxl_render::frame::draw::{DrawList, PIPELINE_CHUNK};
use pxl_render::look::lod::{Ladder, COARSEST};
use pxl_render::mesh::paint::terrain::adapter::ChunkSample;
use pxl_spec::bake::constant::CHUNK_CELL_SPAN;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts};

use crate::global::Global;
use crate::io::load::Models;
use crate::layer::billboard::field::Billboards;
use crate::layer::decor::build::{decor_density, Chunk as DecorChunk, DecorBuild, Kind as DecorKind, Rule as DecorRule};
use crate::layer::decor::instance::{decor_groups, DecorScratch};
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::frame_gizmo::DebugLines;
use crate::layer::marker::hover_cursor::GhostDraw;
use crate::layer::marker::location_puck::Markers;
use crate::layer::terrain::level::{decor_reach, ring, LevelPage, DECOR_REACH, RENDER_AXIS};
use crate::shell::setting::store::hidden_ground;
use crate::world::chunk::node::{BuildEvent, Builder, PageLanes, World, MAX_CHUNKS};
use crate::world::chunk::range::{CHUNK_WORLD_SPAN, PAGE_CHUNK_AXIS, PAGE_SHAMT};
use pxl_spec::space::indexer::L2_IDX;

/// The sphere a chunk nothing has measured is culled by.
const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;
/// How often every chunk is rechecked, whatever is moving.
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
const _: () = assert!(MAX_MOUNTS_PER_FRAME == PAGE_CHUNK_AXIS * PAGE_CHUNK_AXIS, "a page mounts in the frame it arrives in");
/// The heap's live share past which off-screen chunks are reclaimed
/// whatever the retain budget says.
const HEAP_PRESSURE: f64 = 0.9;
/// Tiers one chunk may keep: the active pair, the off-screen tier, and one
/// of hysteresis.
const MAX_CACHED_TIERS: usize = 4;
/// A wait halves a chunk's distance key every this many milliseconds.
const AGE_HALF_MS: f64 = 4000.0;
/// The byte budget, paid in LOD: when the heap refuses a block and reclaim
/// found nothing to give back, the ladder is pulled in a step (the module's
/// pressure) and the set on screen gets coarser until it fits; a step at
/// most this often, so the coarser layouts have time to land and free
/// what they replace before the next is taken, and up to three tiers. A
/// step is let out again after this long with no refusal and the section
/// off its pressure mark.
const MAX_PRESSURE: u32 = 6;
const PRESSURE_STEP_MS: f64 = 500.0;
const PRESSURE_RELIEF_MS: f64 = 5000.0;

pub const IN_FRUSTUM: u8 = 1;
pub const EMPTY: u8 = 2;
pub const PENDING: u8 = 4;
pub const BLEND_ACTIVE: u8 = 16;
pub const SETTLING: u8 = 32;
/// Mounted but no longer wanted: its ground is now some other chunk's — a
/// parent it merged into, the children it split into — and it is drawn until
/// every one of those is ready, then let go.
pub const STALE: u8 = 64;

/// How far past a split's threshold a split chunk stays split, and short of
/// it a whole one splits: the band a view can breathe in without a chunk
/// splitting and merging back.
const SPLIT_HYSTERESIS: f64 = 0.1;

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
    /// Where the camera is heading.
    pub lead: V3,
    pub viewport_h: f64,
    pub fov_deg: f64,
    pub delta_s: f64,
    pub budget_ms: f64,
    pub hard_slack_ms: f64,
    pub pixel_dither: bool,
    pub retain_budget_bytes: f64,
    /// The range anchor — the map centre's chunk, in units — when there is
    /// one: what each level's ring is centred on.
    pub anchor: Option<[f64; 2]>,
    /// The ground's height under the view, in units: what a chunk's distance
    /// is measured to before its own bounds are known.
    pub ground_z: f64,
    /// The levels held, lowest first and one past the top (terrain/level.rs).
    pub window: (u32, u32),
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

/// What a mount pass ran for: the anchor, the eye's chunk, the window, the
/// page set and the ladder's pressure. The pass is skipped while it is the same.
type MountKey = ([f64; 2], [i64; 3], (u32, u32), u32, u32);

/// A chunk by where it is: its page, and its corner in the page's cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ChunkKey {
    page: LevelPage,
    bx: u32,
    by: u32,
}

impl ChunkKey {
    fn level(self) -> u32 {
        self.page.level
    }

    /// The chunk's corner and side, in units.
    fn rect(self) -> (f64, f64, f64) {
        let l = self.page.level;
        let x = ((ivec::X(self.page.page) << PAGE_SHAMT) + self.bx as i64) << l;
        let y = ((ivec::Y(self.page.page) << PAGE_SHAMT) + self.by as i64) << l;
        (x as f64, y as f64, CHUNK_WORLD_SPAN * (1u64 << l) as f64)
    }

    /// The four chunks a level down that cover it: a chunk is 64 cells of
    /// its level, which are 128 of the next, in one page of it.
    fn children(self) -> [ChunkKey; 4] {
        let (x, y) = (self.bx * 2, self.by * 2);
        let span = pxl_spec::bake::constant::PAGE_CELL_SPAN;
        let page = LevelPage::new(
            self.page.level - 1,
            ivec::gather(ivec::X(self.page.page) * 2 + (x / span) as i64, ivec::Y(self.page.page) * 2 + (y / span) as i64, 0),
        );
        let (x, y) = (x % span, y % span);
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| ChunkKey {
            page,
            bx: x + dx * CHUNK_CELL_SPAN,
            by: y + dy * CHUNK_CELL_SPAN,
        })
    }
}

/// Whether two squares, each a corner and a side, share any ground.
fn overlaps(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.2 && b.1 < a.1 + a.2
}

/// How much ground two squares share.
fn overlap_area(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let w = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
    let h = (a.1 + a.2).min(b.1 + b.2) - a.1.max(b.1);
    w.max(0.0) * h.max(0.0)
}

#[derive(Clone, Copy, Default)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy, Default)]
struct Tier {
    step: u32,
    /// The telling of the chunk's sources it was built from (the builder's).
    generation: u32,
    bytes: u32,
    bounds: Option<Bounds>,
    /// The renderer's name for the block, and the counts its lanes are
    /// read by.
    handle: u32,
    vertex_cnt: u32,
    index_cnt: u32,
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
    /// The level it is drawn from, and its side in units: 64 cells of 2^level.
    level: u32,
    span: f64,
    base_x: f64,
    base_y: f64,
    stable_hash: f64,
    /// The bounding sphere in the chunk's own space.
    bc: V3,
    radius: f64,
    distance2: f64,
    lead_distance2: f64,
    flags: u8,
    pending_since: f64,
    request: i32,
    runs_bytes: u32,
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

    fn has(&self, f: u8) -> bool {
        self.flags & f != 0
    }

    fn set(&mut self, f: u8, on: bool) {
        if on {
            self.flags |= f
        } else {
            self.flags &= !f
        }
    }

    fn retained(&self) -> f64 {
        self.runs_bytes as f64 + self.tiers.iter().map(|t| t.bytes as f64).sum::<f64>() + self.decor.iter().map(|d| d.bytes as f64).sum::<f64>()
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
    pub chunks: u32,
    pub in_frustum: u32,
    pub settling: u32,
    pub blend_active: u32,
    pub pending: u32,
    pub offscreen_pending: u32,
    pub oldest_visible_ms: f64,
    pub oldest_release_ms: f64,
    pub committed: u32,
    pub released: u32,
    pub retained_on: f64,
    pub retained_off: f64,
    /// The ladder's pressure: how far the byte budget has pulled the LOD in.
    pub pressure: u32,
}

pub struct Scheduler {
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
    ladder: Ladder,
    recheck_elapsed: f64,
    frame: Option<Frame>,
    /// Something changed that a recheck has not seen: the view moved, a
    /// chunk came or went, a source was told. The canvas is asked for
    /// frames until a full recheck has run over it, or the world stops at
    /// whatever the last frame left — nothing else asks for a frame.
    view_dirty: bool,
    retained_on: f64,
    retained_off: f64,
    pub census: Census,
    /// The pages with a terrain tile, which chunks are mounted from.
    pages: BTreeSet<LevelPage>,
    pages_revision: u32,
    /// Pages whose records have gone since the last tick, whose chunks go
    /// before anything else in the next.
    evicted: Vec<LevelPage>,
    /// The chunks the module mounted, by page and page-local base.
    mounted: BTreeMap<ChunkKey, u32>,
    /// The mounted chunks a pass finds leaving, before they are let go.
    gone: Vec<(ChunkKey, u32)>,
    next_mount_id: u32,
    mounted_for: Option<MountKey>,
    /// The ground the window draws: the top level's drawn ring, as a corner
    /// and a side in units.
    drawn: Option<(f64, f64, f64)>,
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
    /// The chunks the build task advances, in order: the ones whose sources
    /// changed first, then the visible pending, as the last tick left them.
    builds: Vec<u32>,
    /// Chunks whose sources changed, pending until their new layout is up.
    urgent: Vec<u32>,
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
/// and never past the hard deadline once one was tried. How many did something.
fn under_budget<T: Copy>(items: &[T], cap: u32, deadline: f64, hard: f64, now: &impl Fn() -> f64, step: impl FnMut(T) -> bool) -> u32 {
    let at = now();
    let opts = BudgetOpts {
        budget_ms: deadline - at,
        hard_budget_ms: Some(hard - at),
        max_steps: Some(cap),
    };
    run_under_budget(items.iter().copied(), step, opts).ran
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
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
            ladder: Ladder::default(),
            recheck_elapsed: RECHECK_INTERVAL_S,
            frame: None,
            view_dirty: true,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            pages: BTreeSet::new(),
            pages_revision: 0,
            evicted: Vec::new(),
            mounted: BTreeMap::new(),
            gone: Vec::with_capacity(MAX_CHUNKS),
            next_mount_id: MOUNT_ID_BASE,
            mounted_for: None,
            drawn: None,
            decor_kinds: Vec::new(),
            decor_samples: Vec::new(),
            decor_scratch: None,
            refused_seen: 0,
            scratch: Scratch::default(),
            refused_last: false,
            builds: Vec::new(),
            urgent: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            pressure_at: f64::NEG_INFINITY,
            refused_at: f64::NEG_INFINITY,
        }
    }

    /// Teaches a decor kind; its index. Its rules start as `Any` on every
    /// channel, and `decor_rule` sets them.
    pub fn decor_kind(&mut self, kind: DecorKind) -> u32 {
        // Decoration is scattered from the leaves alone, which is sound only
        // while no kind is drawn further than the leaves are.
        assert!(
            decor_reach(kind.extent) <= DECOR_REACH,
            "a decor kind of extent {} is drawn to {}u, past the {}u the leaves reach",
            kind.extent,
            decor_reach(kind.extent),
            DECOR_REACH
        );
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

    /// A chunk at `base`, starting at page cell (tx0, ty0). Its first tier is
    /// guessed from the last frame, so the first recheck's hysteresis starts
    /// from where the camera is.
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, id: u32, level: u32, base: IVec, base_x: f64, base_y: f64, tx0: u32, ty0: u32, stable_hash: f64) {
        self.builder.add(id, level, base, tx0, ty0);
        let span = CHUNK_WORLD_SPAN * (1u64 << level) as f64;
        let mut slot = Slot {
            id,
            level,
            span,
            base_x,
            base_y,
            stable_hash,
            bc: [0.0; 3],
            radius: 0.0,
            distance2: f64::INFINITY,
            lead_distance2: f64::INFINITY,
            flags: EMPTY,
            pending_since: -1.0,
            request: 0,
            runs_bytes: 0,
            tiers: Vec::new(),
            decor: Vec::new(),
            layout_primary: COARSEST,
            layout_secondary: 0,
            layout_generation: 0,
            layout_ready: false,
            desired: (COARSEST, 0),
            told: None,
            refused: None,
        };
        slot.unmeasured();
        if let Some(frame) = self.frame {
            let (visible, _, d2, _) = measure(&frame, &slot);
            if visible {
                slot.layout_primary = relative(self.ladder.guide(d2.sqrt(), COARSEST).step, level);
                slot.desired = (slot.layout_primary, 0);
            }
        }
        self.index.insert(id, self.slots.len());
        assert!(self.slots.len() < MAX_CHUNKS, "more than {MAX_CHUNKS} chunks held");
        self.slots.push(slot);
        self.view_dirty = true;
    }

    pub fn remove(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        let Some(i) = self.index.remove(&id) else { return };
        self.view_dirty = true;
        self.builder.remove(world, id, &mut |e| emit(Event::Build(e)));
        let before = self.slots[i].retained();
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on -= before;
        } else {
            self.retained_off -= before;
        }
        for t in self.slots[i].tiers.iter() {
            self.draw.free(t.handle);
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
    /// hole and never half of one. The chunk goes to the front of the builds.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool, emit: &mut impl FnMut(Event)) {
        let Some(&i) = self.index.get(&id) else { return };
        self.view_dirty = true;
        let before = self.slots[i].retained();
        self.builder.source(world, id, page, show_structures, &mut |e| emit(Event::Build(e)));
        let s = &mut self.slots[i];
        s.runs_bytes = 0;
        if !s.layout_ready {
            s.unmeasured();
        }
        self.account(i, before);
        if self.slots[i].layout_ready && self.slots[i].has(IN_FRUSTUM) {
            let (p, q) = self.slots[i].desired;
            self.set_pending(i, true, build_request(p, q), crate::host::now());
            // Asked now rather than at the tick's commit pass, which a spent
            // budget may not reach this chunk in.
            self.ask(world, i, p, q);
            if !self.urgent.contains(&id) {
                self.urgent.push(id);
            }
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
        let (slots, index, ron, roff, draw, landed) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off, &mut self.draw, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick_capped(world, &self.builds, budget_ms, u32::MAX, &crate::host::now, &mut |e| {
            if let BuildEvent::Tier { id, .. } = e {
                landed.push(id);
            }
            self_record(slots, index, mem, ron, roff, draw, e, emit);
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
                self.slots[i].refused = Some(crate::host::heap_releases());
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
                    let base = ivec::gather(x0 + cx * CHUNK_CELL_SPAN as i64, y0 + cy * CHUNK_CELL_SPAN as i64, 0);
                    if let Some(source) = self.builder.leaf_source(base) {
                        sources.push((base, source, ((cx * CHUNK_CELL_SPAN as i64) as f64, (cy * CHUNK_CELL_SPAN as i64) as f64)));
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
                        page_idx: ivec::gather(col_x >> PAGE_SHAMT, col_y >> PAGE_SHAMT, 0),
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
                self.slots[i].refused = Some(crate::host::heap_releases());
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

    /// A page's terrain tile arrived, or left. A chunk lives no longer than
    /// the record it is built from: once the record has gone — let go, or
    /// taken by another page whose bytes now lie where this page's did — a
    /// build of its chunk would read what is no longer its page, and a mesh
    /// kept past it is memory the record's range no longer accounts for.
    pub fn page(&mut self, page: LevelPage, present: bool) {
        if !present {
            self.evicted.push(page);
        }
        let changed = if present { self.pages.insert(page) } else { self.pages.remove(&page) };
        if changed {
            self.pages_revision = self.pages_revision.wrapping_add(1);
            self.view_dirty = true;
        }
    }

    /// Decor instances the build's lanes had no room for, since the kernel
    /// started: the ledger's, so a chunk short of its decor says so.
    pub fn decor_dropped(&self) -> u32 {
        self.decor_scratch.as_ref().map_or(0, |s| s.dropped)
    }

    /// How far step 2^`level` is wanted: the ladder's threshold, carried
    /// past its last by the doubling every threshold is.
    fn reach(&self, level: u32) -> f64 {
        let last = self.ladder.threshold_count() as u32 - 1;
        self.ladder.threshold(level.min(last) as usize) * (1u64 << level.saturating_sub(last)) as f64
    }

    /// Whether `page` can be drawn: it is resident, or it is above the
    /// window's base and all four pages under it can be. A level holds no
    /// page whose ground the level below holds whole (level.rs `held`), so a
    /// chunk over such a page is drawn only as the chunks under it.
    fn drawable(&self, page: LevelPage, base: u32) -> bool {
        self.pages.contains(&page) || (page.level > base && page.children().iter().all(|&c| self.drawable(c, base)))
    }

    /// Whether `key` is to be drawn as its four children rather than itself:
    /// its page is not resident, so only the pages under it can draw its
    /// ground, or some of its ground wants a step finer than the one it draws
    /// (`relative`), 2^(level + 1), and the pages under it can. A chunk drawn as its children stays so a band
    /// further out than a whole one splits at.
    fn splits(&self, key: ChunkKey, eye: V3, ground_z: f64, base: u32) -> bool {
        if key.level() <= base {
            return false;
        }
        if !self.pages.contains(&key.page) {
            return true;
        }
        let children = key.children();
        let (x0, y0, span) = key.rect();
        let dx = (x0 - eye[0]).max(0.0).max(eye[0] - (x0 + span));
        let dy = (y0 - eye[1]).max(0.0).max(eye[1] - (y0 + span));
        let d = (dx * dx + dy * dy + (eye[2] - ground_z).powi(2)).sqrt();
        let split = children.iter().any(|c| self.mounted.contains_key(c));
        let band = if split { 1.0 + SPLIT_HYSTERESIS } else { 1.0 - SPLIT_HYSTERESIS };
        d < self.reach(key.level()) * band && self.drawable(children[0].page, base)
    }

    /// The chunks to draw: from every chunk of the window's top level the view's
    /// ring draws from, each drawn whole or as its children, down to the
    /// window's base (issue/20260921-level-step-and-zoom-are-one-number.md).
    fn select(&self, frame: &Frame, anchor: IVec, eye: V3, out: &mut BTreeSet<ChunkKey>) {
        let (base, end) = frame.window;
        let mut stack: Vec<ChunkKey> = Vec::new();
        // A page that cannot be drawn yet is drawn from the finer pages under
        // it that can: a window that slides up asks for coarse pages it
        // lacks, and what it already holds below them is the ground until
        // they come.
        let mut pages: Vec<LevelPage> = ring(end - 1, anchor, RENDER_AXIS);
        while let Some(page) = pages.pop() {
            if !self.drawable(page, base) {
                if page.level > base {
                    pages.extend(page.children());
                }
                continue;
            }
            for cy in 0..PAGE_CHUNK_AXIS {
                for cx in 0..PAGE_CHUNK_AXIS {
                    stack.push(ChunkKey {
                        page,
                        bx: cx * CHUNK_CELL_SPAN,
                        by: cy * CHUNK_CELL_SPAN,
                    });
                }
            }
        }
        while let Some(key) = stack.pop() {
            if self.splits(key, eye, frame.ground_z, base) {
                stack.extend(key.children());
            } else {
                out.insert(key);
            }
        }
    }

    /// Which chunks exist: the ones `select` draws, mounted as they are
    /// wanted, and the ones it no longer draws marked stale rather than let
    /// go, so the ground they cover is never a hole while what replaces them
    /// builds (`retire`). Rerun when the anchor moves, the eye crosses a chunk,
    /// the window slides or a page comes or goes.
    fn mount(&mut self, frame: &Frame, emit: &mut impl FnMut(Event)) {
        let Some(anchor) = frame.anchor else { return };
        let eye = frame.map_of(frame.camera);
        let cell = |v: f64| (v / CHUNK_WORLD_SPAN).floor() as i64;
        let key = (
            anchor,
            [cell(eye[0]), cell(eye[1]), cell(eye[2] - frame.ground_z)],
            frame.window,
            self.pages_revision,
            self.ladder.pressure(),
        );
        if self.mounted_for == Some(key) {
            return;
        }
        let anchor_i = ivec::gather(anchor[0] as i64, anchor[1] as i64, 0);
        let top = frame.window.1 - 1;
        let first = ring(top, anchor_i, 1)[0];
        let span = crate::layer::terrain::level::span_of(top);
        let r = (RENDER_AXIS / 2) as f64 * span;
        let x0 = ivec::X(first.page) as f64 * span - r;
        let y0 = ivec::Y(first.page) as f64 * span - r;
        self.drawn = Some((x0, y0, span * RENDER_AXIS as f64));
        let mut wanted = BTreeSet::new();
        self.select(frame, anchor_i, eye, &mut wanted);

        // What is no longer wanted goes stale; what is again, fresh.
        for (k, &id) in self.mounted.iter() {
            if let Some(&i) = self.index.get(&id) {
                self.slots[i].set(STALE, !wanted.contains(k));
            }
        }

        let mut mounts = 0;
        for &k in &wanted {
            if self.mounted.contains_key(&k) {
                continue;
            }
            if mounts >= MAX_MOUNTS_PER_FRAME || self.slots.len() >= MAX_CHUNKS {
                // The rest next frame: the key stays unrecorded. How many
                // chunks a range holds is the viewport's and the anchor's,
                // not a number the kernel hands itself, so at the ceiling
                // the pass mounts no more rather than trapping.
                self.view_dirty = true;
                return;
            }
            mounts += 1;
            let id = self.next_mount_id;
            self.next_mount_id = self.next_mount_id.wrapping_add(1).max(MOUNT_ID_BASE);
            let (x0, y0, _) = k.rect();
            let base = ivec::gather(x0 as i64, y0 as i64, 0);
            emit(Event::Mount {
                id,
                page: k.page,
                base_x: k.bx,
                base_y: k.by,
            });
            self.add(id, k.level(), base, x0, y0, k.bx, k.by, blend_point(k));
            self.mounted.insert(k, id);
        }
        self.mounted_for = Some(key);
    }

    /// Lets go of the stale chunks whose ground is ready to be drawn by what
    /// replaced them: wanted chunks cover all of it, and every one of them
    /// has a layout or is off screen and never will. A stale chunk wholly
    /// outside what the window draws — the view has moved on — goes at once;
    /// one inside it that nothing covers yet is ground whose pages have not
    /// come, and it stays.
    /// Lets go of every chunk of a page whose record has gone, stale or not,
    /// before anything of the tick can build one: `page` says why.
    fn drop_evicted(&mut self, world: &mut World, emit: &mut impl FnMut(Event)) {
        if self.evicted.is_empty() {
            return;
        }
        self.gone.clear();
        for (&k, &id) in self.mounted.iter() {
            if self.evicted.contains(&k.page) {
                self.gone.push((k, id));
            }
        }
        for g in 0..self.gone.len() {
            let (k, id) = self.gone[g];
            self.mounted.remove(&k);
            self.remove(world, id, emit);
            emit(Event::Unmount { id });
        }
        self.evicted.clear();
    }

    fn retire(&mut self, world: &mut World, emit: &mut impl FnMut(Event)) {
        self.gone.clear();
        for (&k, &id) in self.mounted.iter() {
            let Some(&i) = self.index.get(&id) else { continue };
            if !self.slots[i].has(STALE) {
                continue;
            }
            let rect = k.rect();
            let mut covered = 0.0;
            let mut ready = true;
            for w in self.slots.iter().filter(|w| !w.has(STALE)) {
                let area = overlap_area(rect, (w.base_x, w.base_y, w.span));
                if area > 0.0 {
                    covered += area;
                    ready &= w.layout_ready || !w.has(IN_FRUSTUM);
                }
            }
            let whole = covered >= rect.2 * rect.2;
            let gone_from_view = self.drawn.is_none_or(|d| !overlaps(rect, d));
            if gone_from_view || (whole && ready) {
                self.gone.push((k, id));
            }
        }
        for g in 0..self.gone.len() {
            let (k, id) = self.gone[g];
            self.mounted.remove(&k);
            self.remove(world, id, emit);
            emit(Event::Unmount { id });
        }
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, commit and reclaim under the
    /// frame's budget, and set what the build task takes. Whether the canvas has something new to draw.
    pub fn tick(&mut self, world: &mut World, frame: Frame, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) -> bool {
        let t0 = now();
        let deadline = t0 + frame.budget_ms;
        let hard = deadline + frame.hard_slack_ms;
        if !self.frame.is_some_and(|prev| same_view(&prev, &frame)) {
            self.view_dirty = true;
        }
        self.frame = Some(frame);
        self.ladder.set_projection(frame.viewport_h, frame.fov_deg);
        self.recheck_elapsed += frame.delta_s;
        let full_recheck = self.recheck_elapsed >= RECHECK_INTERVAL_S;
        if full_recheck {
            self.recheck_elapsed = 0.0;
        }

        self.drop_evicted(world, emit);
        self.mount(&frame, emit);
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

        eligible.clear();
        eligible.extend(order.iter().copied().filter(|&i| full_recheck || self.slots[i].flags & (BLEND_ACTIVE | SETTLING) != 0));
        let rechecked = under_budget(eligible, u32::MAX, deadline, hard, now, |i| {
            self.recheck(world, i, t0, &frame, emit);
            true
        });
        if full_recheck && rechecked as usize == eligible.len() {
            self.view_dirty = false;
        }

        let pending_total = (visible.len() + offscreen.len()) as u32;
        // A commit on screen may build decor, and a block refused ends the
        // commits this tick as it ends the builds (Builder::tick_capped):
        // the heap is the same until reclaim, and the decor is scattered
        // before its block is asked for.
        let starved = self.builder.stat.starved;
        let mut committed = under_budget(visible, MAX_COMMITS_PER_FRAME, deadline, hard, now, |i| {
            self.builder.stat.starved == starved && self.commit(world, i, emit)
        });
        let releases = if self.refused_last { MAX_RELEASES_WHEN_REFUSED } else { MAX_RELEASES_PER_FRAME };
        committed += under_budget(offscreen, releases, deadline, hard, now, |i| self.commit(world, i, emit));

        // The builds are the build task's (`build_step`), on the turn's quota:
        // what they take is set here — the chunks whose sources changed first,
        // while still pending, then the visible pending in order.
        let (slots, index) = (&self.slots, &self.index);
        let pending = |id: &u32| index.get(id).is_some_and(|&i| slots[i].has(PENDING));
        self.urgent.retain(pending);
        self.builds.clear();
        self.builds.extend(self.urgent.iter().copied());
        let urgent = &self.urgent;
        self.builds
            .extend(visible.iter().filter(|&&i| slots[i].has(PENDING)).map(|&i| slots[i].id).filter(|id| !urgent.contains(id)));
        if !self.builds.is_empty() {
            builds().notify();
        }
        committed += core::mem::take(&mut self.build_committed);

        let freed = self.reclaim(world, order, emit);
        self.scratch = sc;
        self.press(world, t0, freed);
        let animating = self.models.tick(world, &mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
        self.draw_frame(&frame, world_mem(world));

        let mut c = Census { committed, ..Census::default() };
        c.chunks = self.slots.len() as u32;
        c.pending = pending_total;
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
        c.retained_on = self.retained_on;
        c.retained_off = self.retained_off;
        c.pressure = self.ladder.pressure();
        let blending = c.blend_active > 0;
        self.census = c;
        committed > 0 || pending_total > committed || blending || self.view_dirty || animating
    }

    /// This frame's draws: every chunk on screen with a layout, its tiers at
    /// the weights the blend decided, placed where the chunk is in the frame
    /// the camera is written in.
    fn draw_frame(&mut self, frame: &Frame, mem: *const u8) {
        self.draw.begin_frame();
        self.ghost.draw(&mut self.draw, frame.origin);
        self.models.draw(&mut self.draw, frame.origin);
        self.markers.draw(&mut self.draw, frame.origin);
        self.billboards.draw(&mut self.draw, &self.models, frame.origin);
        self.debug.draw(&mut self.draw, frame.origin);
        // A stale chunk is drawn until what replaced it is whole, and what
        // replaced it waits under it: never both on one piece of ground.
        let covering: Vec<(f64, f64, f64)> = self
            .slots
            .iter()
            .filter(|s| s.has(STALE) && s.has(IN_FRUSTUM) && s.layout_ready)
            .map(|s| (s.base_x, s.base_y, s.span))
            .collect();
        for s in &self.slots {
            if !s.has(IN_FRUSTUM) || !s.layout_ready {
                continue;
            }
            if !s.has(STALE) && covering.iter().any(|&c| overlaps(c, (s.base_x, s.base_y, s.span))) {
                continue;
            }
            let (wp, ws) = s.told.map_or((1.0, 0.0), |t| (t.0, t.1));
            let translation = DrawList::translation([s.base_x, s.base_y, 0.0], frame.origin);
            for (step, visible) in [(s.layout_primary, wp), (s.layout_secondary, ws)] {
                if step == 0 {
                    continue;
                }
                let Some(t) = s.tier(step, s.layout_generation) else { continue };
                if t.vertex_cnt == 0 {
                    continue;
                }
                self.draw.draw(PIPELINE_CHUNK, t.handle, t.vertex_cnt, t.index_cnt, translation, visible, 1.0);
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

    /// Where every chunk is and whether it is on screen, measured against the
    /// frame. A slot that crosses the frustum's edge moves its retained bytes
    /// between the on-screen and off-screen totals and says so with a
    /// `Visible` event, which is what the retention budget is spent from.
    fn refresh(&mut self, frame: &Frame, emit: &mut impl FnMut(Event)) {
        for i in 0..self.slots.len() {
            let (visible, _, d2, ld2) = measure(frame, &self.slots[i]);
            let s = &mut self.slots[i];
            s.distance2 = d2;
            s.lead_distance2 = ld2;
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
                empty: s.has(EMPTY),
                lead_distance2: s.lead_distance2,
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
    /// ladder answers for its distance — one tier, or two and a weight while
    /// it crosses — and the slot is pending when what it shows is not that.
    /// SETTLING is pending with a layout already up, so a frame can tell a
    /// chunk that is changing tier from one that has nothing to show yet.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64, frame: &Frame, emit: &mut impl FnMut(Event)) {
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
        let level = s.level;
        let guide = self.ladder.guide(s.distance2.sqrt(), s.layout_primary << level);
        // The ladder speaks in world steps; a chunk builds in its own, and a
        // blend whose two ends are one step of the chunk's is no blend.
        let (primary, secondary) = match guide.blend {
            Some((finer, coarser, _)) if relative(finer, level) != relative(coarser, level) => (relative(finer, level), relative(coarser, level)),
            _ => (relative(guide.step, level), 0),
        };
        self.slots[i].desired = (primary, secondary);
        let current = self.generation(i);
        let s = &self.slots[i];
        let pending = !s.layout_ready || s.layout_primary != primary || s.layout_secondary != secondary || s.layout_generation != current;
        self.set_pending(i, pending, build_request(primary, secondary), now);
        let s = &mut self.slots[i];
        let settling = s.has(PENDING) && s.layout_ready;
        s.set(SETTLING, settling);
        let pending = s.has(PENDING);
        s.set(BLEND_ACTIVE, guide.blend.is_some());
        if !pending {
            self.stop_work(world, i);
        }
        let Some((finer_step, _, f)) = guide.blend.filter(|_| secondary != 0) else {
            self.decide(i, 1.0, 0.0, emit);
            return;
        };
        let finer_step = relative(finer_step, level);
        if pending {
            self.decide(i, 1.0, 0.0, emit);
            return;
        }
        let (finer, coarser) = if frame.pixel_dither {
            (2.0 + (f * 2.0 - 1.0).max(0.0), (f * 2.0).min(1.0))
        } else {
            let coarser = self.slots[i].stable_hash < f;
            (if coarser { 0.0 } else { 1.0 }, if coarser { 1.0 } else { 0.0 })
        };
        let primary_is_finer = self.slots[i].layout_primary == finer_step;
        let (p, q) = if primary_is_finer { (finer, coarser) } else { (coarser, finer) };
        self.decide(i, p as f32, q as f32, emit);
    }

    /// Whether the chunk nodes hold every tier of (primary, secondary) of the
    /// current generation, asking the builder for the ones it lacks when not.
    fn ask(&mut self, world: &mut World, i: usize, primary: u32, secondary: u32) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let steps = if secondary == 0 || secondary == primary { [primary, 0] } else { [primary, secondary] };
        let mut want = [0; 2];
        let mut n = 0;
        for st in steps.into_iter().filter(|&st| st != 0 && s.tier(st, generation).is_none()) {
            want[n] = st;
            n += 1;
        }
        self.builder.want(world, s.id, want);
        n == 0
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
                draw.free(t.handle);
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
            let gone = s.tiers.remove(k);
            self.draw.free(gone.handle);
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
        if s.refused.is_some_and(|r| r == crate::host::heap_releases()) {
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

    /// The retain budget: off-screen built work past it, or the heap past
    /// HEAP_PRESSURE, is given back farthest first. A block refused since the
    /// last pass is pressure too, whatever the numbers say: the heap has room in bytes but not in one piece, and what gives it
    /// back in pieces that join is the off-screen work, farthest first.
    fn reclaim(&mut self, world: &mut World, order: &[usize], emit: &mut impl FnMut(Event)) -> u32 {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let refused = self.builder.stat.starved != self.refused_seen;
        self.refused_seen = self.builder.stat.starved;
        self.refused_last = refused;
        let pressed = heap_pressed;
        let cap = if refused { MAX_RECLAIMS_WHEN_REFUSED } else { MAX_RECLAIMS_PER_FRAME };
        let mut freed = 0;
        // The order already puts off-screen chunks farthest first.
        for &i in order {
            if freed >= cap || !(refused || self.retained_off > budget || pressed(world)) {
                break;
            }
            if self.slots[i].has(IN_FRUSTUM) || self.slots[i].retained() <= 0.0 {
                continue;
            }
            let id = self.slots[i].id;
            let before = self.slots[i].retained();
            self.builder.reclaim(world, id, &mut |e| emit(Event::Build(e)));
            let s = &mut self.slots[i];
            s.runs_bytes = 0;
            for t in core::mem::take(&mut s.tiers) {
                self.draw.free(t.handle);
                emit(Event::Evict {
                    id,
                    step: t.step,
                    generation: t.generation,
                });
            }
            for d in core::mem::take(&mut s.decor) {
                self.draw.free(d.handle);
                emit(Event::Evict {
                    id,
                    step: d.step,
                    generation: d.generation,
                });
            }
            self.account(i, before);
            freed += 1;
        }
        if !refused || freed >= cap {
            return freed;
        }
        // Nothing off screen gave enough, or nothing was off screen — a
        // zoomed-out view has a thousand chunks in the frustum, every one
        // keeping its runs and up to four tiers, and the section is theirs.
        // What they show is their layout's tiers; the runs and the other
        // tiers are a convenience for the next zoom, given back farthest
        // first, from chunks not mid-build, until the frame's share is spent.
        for &i in order.iter().rev() {
            if freed >= cap {
                break;
            }
            let s = &self.slots[i];
            if !s.has(IN_FRUSTUM) || s.has(PENDING) {
                continue;
            }
            let (p, q, g) = (s.layout_primary, s.layout_secondary, s.layout_generation);
            let shown = |step: u32, generation: u32| generation == g && (step == p || step == q);
            let spare = s.runs_bytes > 0 || s.tiers.iter().any(|t| !shown(t.step, t.generation));
            if !spare {
                continue;
            }
            let id = s.id;
            let before = s.retained();
            self.builder.reclaim(world, id, &mut |e| emit(Event::Build(e)));
            let s = &mut self.slots[i];
            s.runs_bytes = 0;
            let draw = &mut self.draw;
            s.tiers.retain(|t| {
                let keep = shown(t.step, t.generation);
                if !keep {
                    draw.free(t.handle);
                    emit(Event::Evict {
                        id,
                        step: t.step,
                        generation: t.generation,
                    });
                }
                keep
            });
            s.decor.retain(|d| {
                let keep = shown(d.step, d.generation);
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
            freed += 1;
        }
        freed
    }

    /// The byte budget paid in LOD (MAX_PRESSURE): a step in when a block
    /// was refused and reclaim gave nothing back, a step out when the
    /// refusals have stopped and the section is off its mark. A change is
    /// a new ladder, and every chunk is rechecked against it.
    fn press(&mut self, world: &World, now: f64, freed: u32) {
        if self.refused_last {
            self.refused_at = now;
        }
        let pressure = self.ladder.pressure();
        let next = if self.refused_last && freed == 0 && pressure < MAX_PRESSURE && now - self.pressure_at >= PRESSURE_STEP_MS {
            pressure + 1
        } else if pressure > 0 && now - self.refused_at >= PRESSURE_RELIEF_MS && now - self.pressure_at >= PRESSURE_RELIEF_MS && !heap_pressed(world) {
            pressure - 1
        } else {
            return;
        };
        self.ladder.set_pressure(next);
        self.pressure_at = now;
        self.recheck_elapsed = RECHECK_INTERVAL_S;
        self.view_dirty = true;
    }
}

/// Whether the heap is live past HEAP_PRESSURE.
fn heap_pressed(_world: &World) -> bool {
    crate::host::heap_live() as f64 > crate::host::heap_bytes() as f64 * HEAP_PRESSURE
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
        && a.lead == b.lead
        && a.viewport_h == b.viewport_h
        && a.fov_deg == b.fov_deg
        && a.pixel_dither == b.pixel_dither
        && a.anchor == b.anchor
        && a.window == b.window
        && a.show_decor == b.show_decor
}

/// A world step as a chunk of `level` builds it: its own cells are 2^level,
/// and it draws them averaged 2×2, its second tier, the step the level above
/// would otherwise draw as its vote (level.rs `level_at`). The leaves alone
/// are drawn as they are too, their first tier, and blend between the two.
/// Finer than that a chunk is not drawn — the split serves it — and coarser
/// it is merged into its parent.
fn relative(world_step: u32, level: u32) -> u32 {
    (world_step >> level).clamp(if level == 0 { 1 } else { 2 }, 2)
}

/// Where a chunk sits in `[0, 1)`, from `<level>/<page index>:<base x>,<base y>`.
///
/// `recheck` compares this against the blend's weight, so neighbouring chunks
/// cross a tier boundary at different moments instead of the whole ring
/// switching at once. It only has to be spread and to be the same for a chunk
/// every time it is mounted; nothing reads the number itself.
fn blend_point(k: ChunkKey) -> f64 {
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, format!("{}/{}:{},{}", k.page.level, k.page.page, k.bx, k.by).as_bytes());
    std::hash::Hasher::finish(&h) as f64 / 2f64.powi(64)
}

/// What a chunk's place in the order is decided by.
#[derive(Clone, Copy)]
pub struct Priority {
    pub holds: bool,
    pub in_frustum: bool,
    pub empty: bool,
    pub lead_distance2: f64,
    pub pending_since: f64,
}

/// The order work is taken in: the slots' holders, then what is on screen
/// nearest to where the camera is heading, then what is off it
/// farthest first — a release takes memory from what is least likely to be
/// wanted again; a wait halves the distance key every AGE_HALF_MS, within
/// its class; at the same key, a hole before a chunk asking to be sharper.
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
        let key = if self.pending_since < 0.0 {
            self.lead_distance2
        } else {
            self.lead_distance2 * 2f64.powf(-2.0 * (now - self.pending_since) / AGE_HALF_MS)
        };
        Keyed { p: self, key }
    }
}

fn priority_keyed(a: &Keyed, b: &Keyed) -> core::cmp::Ordering {
    b.p.holds
        .cmp(&a.p.holds)
        .then(b.p.in_frustum.cmp(&a.p.in_frustum))
        .then_with(|| if a.p.in_frustum { a.key.total_cmp(&b.key) } else { b.key.total_cmp(&a.key) })
        .then(b.p.empty.cmp(&a.p.empty))
}

/// Where a slot is from a frame: on screen, its world centre, and its two
/// squared distances.
fn measure(frame: &Frame, s: &Slot) -> (bool, V3, f64, f64) {
    let [x, y, z] = frame.world_of([s.base_x + s.bc[0], s.base_y + s.bc[1], s.bc[2]]);
    let c = frame.camera;
    let d2 = (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2);
    let l = frame.lead;
    let ld2 = (x - l[0]).powi(2) + (y - l[1]).powi(2) + (z - l[2]).powi(2);
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
    }
    (visible, [x, y, z], d2, ld2)
}

fn world_mem(world: &World) -> *const u8 {
    world.volumes.mem()
}

/// Records what a build handed over on the slot it belongs to, and passes
/// it on.
#[allow(clippy::too_many_arguments)]
fn self_record(slots: &mut [Slot], index: &BTreeMap<u32, usize>, mem: *const u8, retained_on: &mut f64, retained_off: &mut f64, draw: &mut DrawList, e: BuildEvent, emit: &mut impl FnMut(Event)) {
    match e {
        BuildEvent::Tier {
            id,
            generation,
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
        } => {
            if let Some(&i) = index.get(&id) {
                let s = &mut slots[i];
                if s.tier(step, generation).is_none() {
                    // SAFETY: the mesh's position lane, the first three floats
                    // a vertex of the block the bake just wrote.
                    let bounds = (vertex_cnt > 0).then(|| unsafe {
                        let p = core::slice::from_raw_parts(mem.add(at as usize) as *const f32, vertex_cnt as usize * 3);
                        let mut b = Bounds {
                            min: [f32::INFINITY; 3],
                            max: [f32::NEG_INFINITY; 3],
                        };
                        for v in p.as_chunks::<3>().0 {
                            for ((lo, hi), &x) in b.min.iter_mut().zip(b.max.iter_mut()).zip(v) {
                                *lo = lo.min(x);
                                *hi = hi.max(x);
                            }
                        }
                        b
                    });
                    // A mesh of nothing has no block and is never drawn.
                    let handle = if vertex_cnt > 0 { draw.upload(at, bytes, vertex_cnt, index_cnt) } else { 0 };
                    s.tiers.push(Tier {
                        step,
                        generation,
                        bytes,
                        bounds,
                        handle,
                        vertex_cnt,
                        index_cnt,
                    });
                    let on = s.has(IN_FRUSTUM);
                    if on {
                        *retained_on += bytes as f64
                    } else {
                        *retained_off += bytes as f64
                    }
                }
            }
        }
        BuildEvent::Runs { id, bytes } => {
            if let Some(&i) = index.get(&id) {
                let s = &mut slots[i];
                let delta = bytes as f64 - s.runs_bytes as f64;
                s.runs_bytes = bytes;
                if s.has(IN_FRUSTUM) {
                    *retained_on += delta
                } else {
                    *retained_off += delta
                }
            }
        }
    }
    emit(Event::Build(e));
}

#[cfg(test)]
#[path = "table.test.rs"]
mod test;
