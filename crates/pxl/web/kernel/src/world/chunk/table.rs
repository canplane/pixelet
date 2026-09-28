// pxl-kernel/world/chunk/table.rs

//! The chunks' table: materialization as it runs (world/chunk/frontier.rs
//! is its law). Which chunks are mounted, what role each has in the
//! frontier, which tier each shows, what to build and in what order, and
//! what to give back.
//!
//! A mounted chunk is one of three things to the frontier: a piece drawn,
//! a chunk built for a move under way — not drawn until the move is whole —
//! or a piece a move replaced, drawn on the other half of the pattern while
//! what replaced it comes in. Every one holds a slot of the arena or has one
//! held for it (`free`), so the table is never larger than the arena.
//!
//! A tick measures every chunk against the frame, asks each for the tier the
//! rule wants it drawn in at the effective quality (`recheck`), takes the
//! frontier's step (`materialize`), and applies what has landed
//! (`commit`); the builds are the build task's (`build_step`), in the one
//! order.
//!
//! A chunk has one owner, its slot here: the tiers built for it, its decor
//! blocks — a block's memory and its handle let go together — and what its
//! tiers and decor were made from, against which `sources_step` finds a
//! change. A chunk keeps only the tiers it draws and the one it is building:
//! every other is made again from the records if it is wanted, so a slot is
//! never held by what is not on the screen. The builder (build.rs) makes the
//! tiers; the chunk stage (stage.rs) composes the frame — the camera, the
//! anchor root's matrix, the frustum — and calls the tick.

use pxl_spec::space::indexer::FROM_L2_IDX;
use pxl_spec::space::pyramid::{chunk_span_of, CHUNK_CELLS, GROUND};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use pxl_render::mesh::paint::object::adapter::Entries;
use pxl_render::mesh::paint::object::fold::fold_columns;

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch, arena_write};
use pxl_render::look::globe::Globe;
use pxl_render::mesh::chunk_mesher::{box_bounds, box_z, Heights};
use pxl_render::mesh::paint::terrain::adapter::ChunkSample;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::Pvf3;
use pxl_spec::space::pyramid::TOP_LEVEL;
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use crate::global::Global;
use crate::layer::billboard::field::Billboards;
use crate::layer::decor::build::{decor_density, DecorBuild, Ground, Kind as DecorKind, Placed};
use crate::layer::decor::instance::{decor_groups, DecorScratch};
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::debug_lines::DebugLines;
use crate::layer::marker::hover_cursor::GhostDraw;
use crate::layer::marker::location_puck::Markers;
use crate::layer::model::store::Models;
use crate::layer::object::store::{render_edits, EntryAt, RenderEdits};
use crate::mem::fixed::{lane, range, Range};
use crate::param;
use crate::shell::setting::store::{hidden_ground, settings_store};
use crate::world::chunk::build::{mesh_step, release_block, BuildEvent, Builder, Cube, World};
use crate::world::chunk::frontier::{self, drawn_cube, pool_of, wanted_cube, Env, Now, Slots};
use crate::world::chunk::mesh_slot::{group_box_words, group_bytes, group_is_fine, slot_at, surface_of, MeshSlot, Pool, ARENA, FINE_BOXES, OCCUPANCY};
use crate::world::chunk::source::{decor_draws, decor_revision_of, draws_band, objects_revision_of, tiers_draw};
use xpute_core::math::scalar::round;

use crate::world::observation::Order;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::indexer::L2_IDX;

/// Whether the heap is live past `param::HEAP_PRESSURE`.
fn heap_pressed() -> bool {
    let h = crate::host::heap_stat();
    h.live as f64 > h.range as f64 * param::HEAP_PRESSURE
}

pub const IN_FRUSTUM: u16 = 1;
pub const PENDING: u16 = 4;
pub const BLEND_ACTIVE: u16 = 16;
pub const SETTLING: u16 = 32;
/// Mounted and not yet rechecked: it has no layout, so it wants one, and its
/// first recheck is taken whatever the frame's budget (`tick`).
pub const FRESH: u16 = 128;

/// What a mounted chunk is to the frontier (world/chunk/frontier.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// A piece of the frontier: drawn, and what the ground under it is
    /// drawn by until a move replaces it.
    Drawn,
    /// Built for a move under way, and not drawn until every chunk of that
    /// move is ready and the move is made.
    Building,
    /// Replaced by a move, and drawn on the other half of the pattern while
    /// what replaced it comes in; let go when the dissolve is over.
    Leaving,
}

/// A dissolve under way, and when it began on the frames' clock
/// (`Scheduler::clock_ms`). The two sides of one draw with complementary
/// halves of the screen-space pattern (pxl-render chunk.wgsl's blend), so a
/// pixel shows one of them and never both or neither.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fade {
    /// The layout's secondary tier, what the chunk showed, going as its
    /// primary comes.
    Tier(f64),
    /// A piece coming in over ground a replaced one is leaving.
    In(f64),
    /// A replaced piece going as what replaced it comes.
    Out(f64),
}

impl Fade {
    /// How far the dissolve has got, 0 to 1.
    fn progress(self, now: f64) -> f64 {
        let (Fade::Tier(since) | Fade::In(since) | Fade::Out(since)) = self;
        ((now - since) / param::FADE_MS).clamp(0.0, 1.0)
    }
}

/// The tier slot `s`'s layout mostly shows: of the two a dissolve draws, the
/// one covering more of the pixels, which is what a layer stands on and a
/// pick hits.
fn shown_cube(s: &Slot) -> Cube {
    match s.told {
        Some((_, _, true)) => s.layout_secondary.unwrap_or(s.layout_primary),
        _ => s.layout_primary,
    }
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
    pub delta_s: f64,
    /// What is left of the turn's quota when the pass begins, which is not
    /// the quota itself (`FrameSample::budget_ms`).
    pub remaining_ms: f64,
    pub hard_slack_ms: f64,
    pub retain_budget_bytes: f64,
    /// The camera center's height (view.rs's camera_center), in units: what
    /// the camera's height over the sea is measured up from.
    pub center_z: f64,
    /// The map center's latitude's values, which the renderer lays the map
    /// back on the planet about (pxl-render's look/globe.rs); none for a flat
    /// world.
    pub globe: Option<Globe>,
}

impl Frame {
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

#[derive(Clone, Copy, Default)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy)]
struct Tier {
    cube: Cube,
    /// The telling of the chunk's sources it was built from (the builder's).
    generation: u32,
    bounds: Option<Bounds>,
    /// Where its boxes lie on the device, and how many.
    slot: MeshSlot,
    box_cnt: u32,
    /// What its boxes' heights are held in (pxl-render chunk_mesher.rs).
    heights: Heights,
}

/// Chunks that appeared, disappeared and were laid out since the last frame
/// sample: dev-only counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkCounts {
    pub added: u32,
    pub removed: u32,
    pub layouts: u32,
}

/// What the chunks hold, for the profiler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkRetention {
    /// Chunks holding a run set, and what those sets cost in the heap.
    pub run_sets: u32,
    pub run_set_mb: u32,
    /// Built tiers held across every chunk.
    pub tiers: u32,
    /// Decor: per-tier blocks.
    pub decor_steps: u32,
    /// The instances those blocks hold, which is what they cost to draw.
    pub decor_instances: u32,
}

/// A decor block a chunk holds: its cube and bytes, and the
/// renderer's handle. Each group's lanes are where the block's own header
/// says.
#[derive(Clone, Copy)]
pub struct Decor {
    cube: Cube,
    generation: u32,
    /// Where the block is in the memory, and its size.
    pub at: u32,
    pub bytes: u32,
    handle: u32,
}

impl Decor {
    /// Gives the block back and its handle with it: one lifetime, the slot's.
    fn release(self, draw: &mut DrawList) {
        draw.free(self.handle);
        release_block(self.at as usize, self.bytes);
    }

    /// Each group's first lane word in the block and its count
    /// (decor/instance.rs's decor_groups).
    fn groups(&self) -> impl Iterator<Item = (u32, u32)> {
        // SAFETY: the block the scheduler wrote at `at`, `bytes` long, held
        // for as long as this decor is.
        let words: &[u32] = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(self.at as usize), self.bytes as usize / 4) };
        decor_groups(words)
    }
}

#[derive(Clone)]
struct Slot {
    id: u32,
    /// The chunk it draws.
    chunk: Chunk,
    role: Role,
    /// The bounding sphere in the chunk's own space.
    bc: V3,
    radius: f64,
    /// Its place in the order every layer's work is taken in (world/observation.rs
    /// `Order`).
    order: Order,
    flags: u16,
    pending_since: f64,
    /// The finest cube a tier of this chunk fits its slot at, of the sources
    /// generation it was found for: where a finer one had more boxes than a
    /// slot holds (build.rs `BuildEvent::Coarser`). A new generation finds it
    /// again.
    fit: Option<(u32, Cube)>,
    /// A drawn piece the frontier has let build a tier other than the one it
    /// shows: a slot is held for it (frontier.rs `Plan::recube`).
    recube: bool,
    /// The tiers built for it: those its layout draws, and the one it is
    /// building. A source changed leaves them where they are: a cube's tier
    /// of the new generation joins the old one's, and the old go when a
    /// layout of the new is applied.
    tiers: Vec<Tier>,
    /// Its decor blocks, by cube and generation alike.
    decor: Vec<Decor>,
    layout_primary: Cube,
    layout_secondary: Option<Cube>,
    /// The generation the layout's tiers and decor are drawn from.
    layout_generation: u32,
    layout_ready: bool,
    /// Whether the layout's decor is built: the reclaim gives an off-screen
    /// chunk's back, and it is built again once the chunk is on screen.
    decor_whole: bool,
    /// The cube the rule wants it drawn in now.
    desired: Cube,
    /// The last blend told, so an unchanged one is not resent.
    told: Option<(f32, f32, bool)>,
    /// The heap's release count when its decor was last refused a
    /// block: not committed again until a block is given back.
    refused: Option<u32>,
    /// The dissolve it is in, if any.
    fade: Option<Fade>,
    /// What the builder was last told the chunk is made of, and what its
    /// decor was last built from: a change of either is a rebuild of the
    /// one it names (`sources_step`).
    sources: Sources,
    decor_sources: DecorSources,
}

/// What a chunk's tiers are made of, past its page: the settings it paints
/// with, the revision of the objects folded into it, and the eraser's edits
/// that fall on them (object/store.rs `render_edits`).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    structures: bool,
    /// The texture setting the tiers were baked under. A leaf's box carries
    /// its layer and the shader samples or does not, but a tier over it
    /// carries one color and no layer, so which color that is was decided
    /// when it was baked (mesh/chunk_mesher.rs).
    textured: bool,
    objects_revision: u32,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
    /// The level of the page the chunk is read from: its own, or the nearest
    /// resident page over it (world/pyramid.rs `drawn_from`), so its own
    /// page arriving builds it again from its own ground.
    read_from: Option<u32>,
}

/// What a chunk's decor is made of: the revision of the columns it draws the
/// placed decor of, the setting, and the hidden entry where it is decor
/// there.
#[derive(Clone, Copy, PartialEq, Eq)]
struct DecorSources {
    revision: u32,
    shown: bool,
    hide: Option<EntryAt>,
}

impl Sources {
    fn of(k: Chunk, edits: RenderEdits) -> Sources {
        let settings = settings_store();
        let (level, base) = (k.level(), k.corner());
        let drawn = |at: &EntryAt| tiers_draw(level, base, at.col_idx) && draws_band(k, at.band_idx);
        Sources {
            structures: settings.structures,
            textured: settings.textures,
            objects_revision: objects_revision_of(k),
            hide: edits.hide.filter(|at| edits.hide_in.0 && drawn(at)),
            carve: edits.carve.filter(drawn),
            read_from: Terrain::pyramid().drawn_from(k.page()).map(|p| p.level),
        }
    }
}

impl DecorSources {
    fn of(k: Chunk, edits: RenderEdits) -> DecorSources {
        let (level, base) = (k.level(), k.corner());
        DecorSources {
            revision: decor_revision_of(k),
            shown: settings_store().decor,
            hide: edits.hide.filter(|at| edits.hide_in.1 && decor_draws(level, base, at.col_idx) && draws_band(k, at.band_idx)),
        }
    }
}

impl Slot {
    /// The level it is drawn from.
    fn level(&self) -> u32 {
        self.chunk.level()
    }

    /// Its side in units: 64 cells of 2^level.
    fn span(&self) -> f64 {
        chunk_span_of(self.chunk.level())
    }

    /// Its corner, in units.
    fn base_x(&self) -> f64 {
        self.chunk.rect().0
    }

    fn base_y(&self) -> f64 {
        self.chunk.rect().1
    }

    /// The sphere the chunk is culled by until its mesh has measured it: the
    /// box the view measured it by (domain.rs `Domain::heights`), its whole
    /// footprint and the heights its ground and objects span there. A chunk
    /// culled until it is built is never built, so it is never measured and
    /// stays culled: a sphere at a corner missed every view that missed the
    /// corner, and one at sea level missed every chunk of a mountain, whose
    /// ground stood hundreds of units over it, and left the middle of the
    /// view empty with nothing left to fetch.
    fn unmeasured(&mut self) {
        let span = self.span();
        let (z0, z1) = self.chunk.z_range();
        let (lo, hi) = Domain::heights(self.chunk.page(), self.chunk.column()).map_or((z0, z1), |(lo, hi)| (lo.max(z0), hi.min(z1).max(z0)));
        self.bc = [span * 0.5, span * 0.5, (lo + hi) * 0.5];
        self.radius = (2.0 * span * span + (hi - lo) * (hi - lo)).sqrt() * 0.5;
    }

    /// The finest cube this chunk's `generation` fits its slot at, where one
    /// is known (`fit`).
    fn fit_at(&self, generation: u32) -> Option<Cube> {
        self.fit.filter(|&(g, _)| g == generation).map(|(_, f)| f)
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

    fn tier(&self, cube: Cube, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.cube == cube && t.generation == generation)
    }

    fn decor_of(&self, cube: Cube, generation: u32) -> Option<&Decor> {
        self.decor.iter().find(|d| d.cube == cube && d.generation == generation)
    }

    /// The pool its desired tier takes.
    fn desired_pool(&self) -> Pool {
        Pool::of(self.level(), mesh_step(self.desired, self.level()))
    }

    /// Whether it draws a cube other than the one it wants.
    fn recubes(&self) -> bool {
        self.layout_ready && self.layout_primary != self.desired
    }

    /// The cube it builds now: the one it wants, but for a drawn piece the
    /// frontier has not yet let change cube, the one it shows — rebuilt in
    /// its own slot when its sources change, so an edit shows whatever the
    /// arena's room.
    fn target(&self) -> Cube {
        if self.role == Role::Drawn && self.recubes() && !self.recube {
            self.layout_primary
        } else {
            self.desired
        }
    }

    /// The pool a slot is held in for it and not yet taken: what it builds
    /// has not landed, and it has no tier of that cube to build into.
    fn reserves(&self) -> Option<Pool> {
        let cube = self.target();
        (self.role != Role::Leaving && !self.tiers.iter().any(|t| t.cube == cube)).then(|| Pool::of(self.level(), mesh_step(cube, self.level())))
    }
}

/// The census the profiler reads.
#[derive(Clone, Copy, Default)]
pub struct Census {
    /// Chunks the table holds, which the arena bounds.
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
    pub oldest_offscreen_ms: f64,
    pub retained_on: f64,
    pub retained_off: f64,
    /// Pieces replaced and dissolving out; of those, the ones drawn, and the
    /// levels they are of as bits.
    pub leaving: u32,
    pub leaving_drawn: u32,
    pub leaving_levels: u32,
    /// Chunks built for a move and ready, waiting on the rest of it.
    pub ready_waiting: u32,
    /// Meshes thrown away since the last census because their pool had no
    /// free slot, and the levels of the chunks they were for, as bits.
    pub slot_waits: u32,
    pub wait_levels: u32,
    /// Tiers baked a step coarser than asked since the last census: they had
    /// more boxes than their slot holds (mesh_slot.rs `capacity`).
    pub coarser: u32,
    /// The largest mesh handed over since the last census, in boxes, of a
    /// fine slot and of a coarse one.
    pub fine_peak: u32,
    pub coarse_peak: u32,
    /// Moves of the frontier the arena had no room for this frame, and moves
    /// under way and not all ready (frontier.rs `Plan`).
    pub refused: u32,
    pub waiting: u32,
    /// The boxes the last draw listed, by the level of the chunk they are of.
    pub level_boxes: [u32; LEVELS],
    /// Chunks wanting a layout that the commit pass did not reach: whether
    /// the frame's cap is what holds the screen back.
    pub commit_left: u32,
}

/// The pyramid's levels, a chunk's being one of them.
pub const LEVELS: usize = TOP_LEVEL as usize + 1;

/// Who holds each mesh slot (mesh_slot.rs), by chunk and mounting, none for
/// a free one; and the arena epoch the tiers were written under.
pub struct Meshes {
    holder: Vec<Option<(Chunk, u32)>>,
    epoch: u32,
    /// Tiers baked a step coarser than asked, until a census takes them.
    pub coarser: u32,
    /// Which telling of the fits this is: it moves whenever a chunk finds
    /// its tiers do not fit their slot, which the quality policy counts.
    fits: u32,
    /// The largest mesh handed to a slot of each kind, until a census takes
    /// them: what a slot's size is read against.
    fine_peak: u32,
    coarse_peak: u32,
    /// Meshes thrown away for want of a free slot, since the census, and the
    /// levels of the chunks they were for as bits.
    waits: u32,
    wait_levels: u32,
}

pub struct Scheduler {
    pub meshes: Meshes,
    /// The boxes the last draw listed by level (`Census::level_boxes`).
    level_boxes: [u32; LEVELS],
    pub builder: Builder,
    /// What the renderer uploads, frees and draws (draw.rs).
    pub draw: DrawList,
    /// The hover ghost (marker/hover_cursor.rs).
    pub ghost: GhostDraw,
    /// Models and the actors that play them (layer/model/store.rs).
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
    /// The debug gizmos' segments (marker/debug_lines.rs).
    pub debug: DebugLines,
    slots: Vec<Slot>,
    /// Where each mounted chunk's slot is: the one way a chunk is found. A
    /// slot's `id` is which mounting of the chunk it is, so what was built
    /// for one that has gone is told from what is built for it now.
    index: BTreeMap<Chunk, usize>,
    /// The domain's pieces the last recheck saw (world/pyramid/domain.rs
    /// `revision`): what a chunk should show is read off it, and it is made
    /// again when the eye, the resident pages, the heights or the quality
    /// move.
    domain_seen: u32,
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
    counts: ChunkCounts,
    next_mount_id: u32,
    /// The decor kinds, in the order decor/build.rs taught them — the order of
    /// the groups in every decor block.
    decor_kinds: Vec<DecorKind>,
    /// A decor build's sample of its chunk's ground, and its chunk's objects
    /// folded to its level, kept for their capacity.
    decor_sample: ChunkSample,
    decor_folded: HashMap<[i64; 3], u32>,
    /// The frames' time, in milliseconds: what a dissolve runs on, as every
    /// animation does, so it takes the frames it says rather than whatever
    /// the wall clock did between them.
    clock_ms: f64,
    /// Where a decor build gathers its instances, made at the first build:
    /// the scheduler is made before the sections are.
    decor_scratch: Option<DecorScratch>,
    /// The builder's refusals as of the last reclaim, so a refusal since
    /// is pressure now.
    refused_seen: u32,
    /// The chunks the build task advances, in the one order, as the last
    /// tick left them.
    builds: Vec<u32>,
    landed: Vec<(Chunk, u32)>,
    /// Chunks the build task committed since the last census.
    build_committed: u32,
    /// What the frontier's last step left: moves refused for room, and
    /// moves under way.
    moves: (u32, u32),
    /// The tick's lists, kept for their capacity: a tick over a thousand
    /// chunks fills several of them.
    scratch: Scratch,
}

/// Empty by default: the tick takes it out with `mem::take` and puts it back,
/// so what `Default` makes is thrown away each tick, and allocating there
/// would allocate every tick.
#[derive(Default)]
struct Scratch {
    order: Vec<usize>,
    keys: Vec<Priority>,
    pending: Vec<usize>,
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

/// What the frontier reads of the chunks the table has not drawn: the pool
/// each takes, whether its ground is resident, and where it stands in the
/// view's order.
struct Pieces<'a> {
    d: &'a Domain,
    table: &'a Scheduler,
}

impl Env for Pieces<'_> {
    fn pool(&self, c: Chunk) -> Pool {
        pool_of(c, self.d.outermost(c), self.d.nearest_to(c), self.d.tau(), self.table.fit_of(c))
    }

    fn buildable(&self, c: Chunk) -> bool {
        Terrain::pyramid().drawn_from(c.page()).is_some()
    }

    fn order(&self, c: Chunk) -> Order {
        self.d.order_of(c)
    }
}

impl Scheduler {
    pub fn new() -> Scheduler {
        arena_configure(&group_bytes());
        Scheduler {
            level_boxes: [0; LEVELS],
            meshes: Meshes {
                holder: vec![None; OCCUPANCY],
                epoch: arena_epoch(),
                coarser: 0,
                fits: 0,
                fine_peak: 0,
                coarse_peak: 0,
                waits: 0,
                wait_levels: 0,
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
            slots: Vec::new(),
            index: BTreeMap::new(),
            domain_seen: u32::MAX,
            frame: None,
            view_dirty: true,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            counts: ChunkCounts::default(),
            next_mount_id: MOUNT_ID_BASE,
            decor_kinds: Vec::new(),
            decor_sample: ChunkSample::default(),
            decor_folded: HashMap::new(),
            clock_ms: 0.0,
            decor_scratch: None,
            refused_seen: 0,
            scratch: Scratch::default(),
            builds: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            moves: (0, 0),
        }
    }

    /// Teaches a decor kind.
    pub fn decor_kind(&mut self, kind: DecorKind) {
        self.decor_kinds.push(kind);
    }

    /// Forgets every kind taught, so they can be taught again.
    pub fn decor_forget(&mut self) {
        self.decor_kinds.clear();
    }

    fn account(&mut self, i: usize, before: f64) {
        let delta = self.slots[i].retained() - before;
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on += delta;
        } else {
            self.retained_off += delta;
        }
    }

    /// The finest cube chunk `c`'s current sources fit its slot at, where a
    /// finer one was found not to: what the quality policy counts its pool
    /// by (frontier.rs `need`).
    pub fn fit_of(&self, c: Chunk) -> Option<Cube> {
        let &i = self.index.get(&c)?;
        self.slots[i].fit_at(self.generation(i))
    }

    /// Which telling of the fits this is (`fit_of`).
    pub fn fits_revision(&self) -> u32 {
        self.meshes.fits
    }

    // ---- commands ----

    /// Chunk `k`, mounted for a move of the frontier, its tier the one the
    /// rule wants for it now.
    fn add(&mut self, world: &mut World, id: u32, k: Chunk) {
        let level = k.level();
        let edits = render_edits();
        let sources = Sources::of(k, edits);
        self.builder.add(id, k);
        self.builder.source(world, id, sources.structures);
        let mut slot = Slot {
            id,
            chunk: k,
            role: Role::Building,
            bc: [0.0; 3],
            radius: 0.0,
            order: Order::LAST,
            flags: FRESH,
            pending_since: -1.0,
            fit: None,
            recube: false,
            tiers: Vec::new(),
            decor: Vec::new(),
            layout_primary: frontier::coarsest(level),
            layout_secondary: None,
            layout_generation: 0,
            layout_ready: false,
            decor_whole: false,
            desired: frontier::coarsest(level),
            told: None,
            refused: None,
            fade: None,
            sources,
            decor_sources: DecorSources::of(k, edits),
        };
        slot.unmeasured();
        if let Some(frame) = self.frame {
            slot.set(IN_FRUSTUM, measure(&frame, &slot).0);
        }
        if let Some(d) = domain::current() {
            slot.order = d.order_of(k);
        }
        // Every chunk mounted holds a slot of the arena or has one held for
        // it (`Slot::reserves`), and the frontier starts none it has no room
        // for: more than the arena's slots is a bug in that accounting.
        xpute_core::ensure!(self.slots.len() < OCCUPANCY, ENOSPC, OCCUPANCY);
        self.index.insert(k, self.slots.len());
        self.slots.push(slot);
        let i = self.slots.len() - 1;
        self.recheck(world, i, crate::host::now());
        self.view_dirty = true;
    }

    fn remove(&mut self, world: &mut World, k: Chunk) {
        let Some(i) = self.index.remove(&k) else { return };
        let id = self.slots[i].id;
        self.view_dirty = true;
        self.counts.removed += 1;
        self.builder.remove(world, id);
        let before = self.slots[i].retained();
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on -= before;
        } else {
            self.retained_off -= before;
        }
        for t in self.slots[i].tiers.iter() {
            let held = &mut self.meshes.holder[t.slot.index];
            if held.is_some_and(|(_, h)| h == id) {
                *held = None;
            }
        }
        for d in self.slots[i].decor.iter() {
            d.release(&mut self.draw);
        }
        self.slots.swap_remove(i);
        if i < self.slots.len() {
            self.index.insert(self.slots[i].chunk, i);
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
    /// hole and never half of one. A tier of the new generation is built
    /// into the slot of the old one's cube, so a rebuild takes no slot.
    fn source(&mut self, world: &mut World, i: usize) {
        self.view_dirty = true;
        let (id, structures) = (self.slots[i].id, self.slots[i].sources.structures);
        self.builder.source(world, id, structures);
        let s = &mut self.slots[i];
        if !s.layout_ready {
            s.unmeasured();
        }
        s.set(FRESH, true);
    }

    /// Every chunk's sources against what it was built from: a chunk whose
    /// tiers' sources moved is told again, and one whose decor's did has its
    /// decor rebuilt, the tiers untouched. The terrain page is not a source
    /// that changes: a bake is the same whenever it is fetched. A page that
    /// goes is no change either: a build of its chunk finds no page by its
    /// key and waits (build.rs), and what is drawn stays drawn.
    pub fn sources_step(&mut self, world: &mut World) {
        let edits = render_edits();
        for i in 0..self.slots.len() {
            let k = self.slots[i].chunk;
            let sources = Sources::of(k, edits);
            if sources != self.slots[i].sources {
                self.slots[i].sources = sources;
                self.source(world, i);
            }
            let decor = DecorSources::of(k, edits);
            if decor != self.slots[i].decor_sources {
                self.slots[i].decor_sources = decor;
                self.decor_now(world, i);
            }
        }
    }

    /// The build task's step: the chunks the last tick ordered, holders of the
    /// scan and the bake slot first, advanced for what is left of the turn's
    /// quota and at least one step, and the chunks a tier landed on
    /// committed. Whether any did work; none did when all are built, blocked,
    /// or a block was refused — the heap is the same until the next tick
    /// reclaims.
    pub fn build_step(&mut self, world: &mut World, budget_ms: f64) -> bool {
        let (slots, index, meshes, landed) = (&mut self.slots, &self.index, &mut self.meshes, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick(world, &self.builds, budget_ms, &crate::host::now, &mut |e| match e {
            BuildEvent::Tier { chunk, id, .. } => {
                landed.push((chunk, id));
                self_record(slots, index, meshes, e);
            }
            BuildEvent::Coarser { chunk, id, generation, cube } => {
                let Some(i) = slot_of(slots, index, chunk, id) else { return };
                let s = &mut slots[i];
                let finer = s.fit_at(generation);
                s.fit = Some((generation, cube.up(1).max(finer.unwrap_or(cube.up(1)))));
                // Asked again at the cube it fits, on the next tick.
                s.set(FRESH, true);
                meshes.coarser += 1;
                meshes.fits = meshes.fits.wrapping_add(1);
            }
        });
        let landed = core::mem::take(&mut self.landed);
        for &(k, id) in &landed {
            if let Some(i) = slot_of(&self.slots, &self.index, k, id) {
                if self.commit(world, i) {
                    self.build_committed += 1;
                }
            }
        }
        self.landed = landed;
        worked
    }

    /// Only the chunk's decor changed (an entry placed or hidden, the
    /// setting): the decor it holds is stale and let go; the layout's decor is
    /// rebuilt now and the layout applied again, the tiers untouched. Unless a
    /// block was refused, when the ground stays drawn without it and it
    /// lands on a later tick.
    fn decor_now(&mut self, world: &mut World, i: usize) {
        let before = self.slots[i].retained();
        for d in core::mem::take(&mut self.slots[i].decor) {
            d.release(&mut self.draw);
        }
        self.account(i, before);
        self.slots[i].decor_whole = false;
        if !self.slots[i].layout_ready {
            return;
        }
        let (p, s2, g) = (self.slots[i].layout_primary, self.slots[i].layout_secondary, self.slots[i].layout_generation);
        if self.ensure_decor(world, i, [Some(p), s2], g) {
            self.apply_layout(i, p, s2, g);
            self.slots[i].decor_whole = true;
        } else {
            self.slots[i].set(FRESH, true);
            self.view_dirty = true;
        }
    }

    /// Whether the chunk holds the decor of every tier in `cubes` of
    /// `generation`, building what it lacks: false when the
    /// heap refused a block. A chunk with no page has no decor to build.
    fn ensure_decor(&mut self, world: &mut World, i: usize, cubes: [Option<Cube>; 2], generation: u32) -> bool {
        let (id, level, corner) = (self.slots[i].id, self.slots[i].level(), self.slots[i].chunk.corner());
        let (x0, y0) = (ivec::X(corner), ivec::Y(corner));
        // Whether it scatters is one of what its decor is made of, as told.
        let decor = self.slots[i].decor_sources.shown;
        if self.decor_scratch.is_none() {
            self.decor_scratch = DecorScratch::new();
        }
        for cube in cubes.into_iter().flatten() {
            if self.slots[i].decor_of(cube, generation).is_some() {
                continue;
            }
            // A chunk scatters from its own page at its own level, at the
            // world step its cube is. A kind is drawn, scattered or placed,
            // only at a step where it covers the pixels it is drawn at, so a
            // step no kind is drawn at has no block. The setting is the
            // scatter's alone: what the user placed is drawn whatever it says.
            let world_step = GROUND.scale(0).count_in(cube.max(GROUND.scale(0)));
            // Every column whose corner lies in the chunk: its own, at z16.
            let cols = world.store.cols_under(self.slots[i].chunk.column());
            let drawn = |k: &&DecorKind| decor_density(k.extent, world_step) > 0.0;
            let scatters = decor && self.decor_kinds.iter().filter(drawn).any(|k| k.density > 0.0);
            let places = !cols.is_empty() && self.decor_kinds.iter().filter(drawn).any(|k| k.decor_value.is_some());
            if !scatters && !places {
                continue;
            }
            let source = self.builder.ground_of(id);
            let view = source.map(|(_, _, page, _)| Builder::page_view(page));
            if let (Some((tx0, ty0, _, show)), Some(view)) = (source, view.as_ref()) {
                if scatters {
                    self.decor_sample.read(view, [tx0 as i64, ty0 as i64], 1, hidden_ground(show));
                }
            }
            let placed: Vec<Placed> = cols
                .into_iter()
                .map(|col_idx| {
                    let o = FROM_L2_IDX(col_idx);
                    Placed {
                        col_idx,
                        ox: (ivec::X(o) - x0) as f64,
                        oy: (ivec::Y(o) - y0) as f64,
                        hittable: level == 0,
                    }
                })
                .collect();
            // Above z16 the objects are drawn folded to the level's cell, the
            // fold the chunk's runs are made with (build.rs `level_runs`): what
            // keeps a candidate off a cell is that fold, as a z16 chunk's is
            // the store's own cell.
            self.decor_folded.clear();
            if level > 0 && scatters {
                let columns: Vec<Entries> = placed.iter().filter_map(|p| world.store.entries(p.col_idx)).collect();
                fold_columns(world.painter, &columns, GROUND.scale(level), self.builder.hide(), &mut self.decor_folded);
            }
            // The surface the tier is drawn with, which an instance stands
            // on: its tier is built before its decor (`commit`).
            let surface = self.slots[i].tier(cube, generation).map(|t| Drawn {
                level,
                base: ivec::gather(x0, y0, 0),
                step: mesh_step(cube, level),
                tops: surface(t.slot.index, mesh_step(cube, level)),
                heights: t.heights,
            });
            let ground = source.zip(view.as_ref()).map(|((tx0, ty0, _, _), view)| Ground {
                page: view,
                sample: &self.decor_sample,
                level,
                x0,
                y0,
                tx0,
                ty0,
                col_idx: (level == 0).then(|| L2_IDX(ivec::gather(x0, y0, 0))),
                surface: surface.as_ref(),
                folded: (level > 0).then_some(&self.decor_folded),
            });
            let Some(scratch) = self.decor_scratch.as_mut() else { return false };
            let heights = self.slots[i].chunk.z_range();
            let built = DecorBuild::build(scratch, &self.decor_kinds, ground.as_ref(), &placed, self.builder.hide(), world.store, world_step, decor, heights);
            let bytes = built.bytes();
            let Some((at, bytes)) = alloc(bytes) else {
                self.builder.stat.starved += 1;
                self.slots[i].refused = Some(crate::host::heap_stat().releases);
                return false;
            };
            // SAFETY: a block of the bytes the build asked for, the slot's from
            // here until its decor is let go.
            unsafe { built.write(ptr_at::<u8>(at)) };
            let before = self.slots[i].retained();
            let handle = self.draw.upload(at as u32, bytes, 0, 0);
            self.slots[i].decor.push(Decor {
                cube,
                generation,
                at: at as u32,
                bytes,
                handle,
            });
            self.account(i, before);
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

    /// Every drawn piece's decor on screen as shown this frame — the block
    /// of the tier its layout mostly shows — with the chunk and its base, in
    /// units: what a hit test walks.
    pub fn shown_decor(&self) -> impl Iterator<Item = (Chunk, f64, f64, &Decor)> {
        self.slots
            .iter()
            .filter(|s| s.role == Role::Drawn && s.has(IN_FRUSTUM) && s.layout_ready)
            .filter_map(|s| s.decor_of(shown_cube(s), s.layout_generation).map(|d| (s.chunk, s.base_x(), s.base_y(), d)))
    }

    /// The counts since the last frame sample, taken.
    pub fn take_counts(&mut self) -> ChunkCounts {
        core::mem::take(&mut self.counts)
    }

    /// What the chunks hold, with the run sets `.fixed.runs` has lent as the
    /// builder counts them (`Builder::run_sets`).
    pub fn retention(&self) -> ChunkRetention {
        let (run_sets, run_bytes) = self.builder.run_sets();
        let mut out = ChunkRetention {
            run_sets,
            run_set_mb: round(run_bytes as f64 / 1_048_576.0) as u32,
            ..ChunkRetention::default()
        };
        for s in &self.slots {
            out.tiers += s.tiers.len() as u32;
            out.decor_steps += s.decor.len() as u32;
            out.decor_instances += s.decor.iter().flat_map(|d| d.groups()).map(|(_, n)| n).sum::<u32>();
        }
        out
    }

    /// Decor instances the build's lanes had no room for, since the kernel
    /// started: the profiler's, so a chunk short of its decor says so.
    pub fn decor_dropped(&self) -> u32 {
        self.decor_scratch.as_ref().map_or(0, |s| s.dropped)
    }

    /// Slots each pool has that no tier holds and no chunk mounted will
    /// take: what the frontier may start moves in.
    fn free(&self) -> Slots {
        let mut free = ARENA;
        for (at, h) in self.meshes.holder.iter().enumerate() {
            if h.is_some() {
                let p = Pool::at(at) as usize;
                free[p] = free[p].saturating_sub(1);
            }
        }
        for s in &self.slots {
            if let Some(p) = s.reserves() {
                free[p as usize] = free[p as usize].saturating_sub(1);
            }
        }
        free
    }

    /// The frontier's step (world/chunk/frontier.rs): the drawn pieces moved
    /// toward the view's pieces at the effective quality, as far as what is
    /// ready and the arena's room let them.
    fn materialize(&mut self, world: &mut World) {
        let Some(d) = domain::current() else { return };
        let mut drawn = BTreeSet::new();
        let mut building = BTreeMap::new();
        let mut recubing = 0;
        let mut recube = Vec::new();
        for s in &self.slots {
            match s.role {
                Role::Drawn => {
                    drawn.insert(s.chunk);
                    if s.recube {
                        recubing += 1;
                    } else if s.recubes() {
                        recube.push((s.chunk, s.desired_pool()));
                    }
                }
                Role::Building => {
                    building.insert(s.chunk, s.layout_ready);
                }
                Role::Leaving => {}
            }
        }
        let now = Now {
            goal: d.pieces(),
            drawn: &drawn,
            building: &building,
            recubing,
            recube: &recube,
            free: self.free(),
        };
        let plan = frontier::plan(&now, &Pieces { d, table: self });
        self.moves = (plan.refused, plan.waiting);

        for &k in plan.cancel.iter().chain(&plan.drop) {
            self.remove(world, k);
        }
        let fades = settings_store().lod_crossfade;
        let clock = self.clock_ms;
        for swap in &plan.swaps {
            // A move dissolves where what leaves was on the screen; where it
            // was not, it is made at once.
            let shown = fades
                && swap
                    .leave
                    .iter()
                    .any(|k| self.index.get(k).is_some_and(|&i| self.slots[i].layout_ready && self.slots[i].has(IN_FRUSTUM)));
            for &k in &swap.leave {
                let Some(&i) = self.index.get(&k) else { continue };
                if shown && self.slots[i].layout_ready {
                    let s = &mut self.slots[i];
                    s.role = Role::Leaving;
                    s.recube = false;
                    s.fade = Some(Fade::Out(clock));
                    s.set(PENDING, false);
                    self.builder.want(world, s.id, [None, None]);
                } else {
                    self.remove(world, k);
                }
            }
            for &k in &swap.enter {
                let Some(&i) = self.index.get(&k) else { continue };
                let s = &mut self.slots[i];
                s.role = Role::Drawn;
                if shown {
                    s.layout_secondary = None;
                    s.fade = Some(Fade::In(clock));
                }
            }
            self.view_dirty = true;
        }
        let now = crate::host::now();
        for &k in &plan.start {
            match self.index.get(&k) {
                // A piece dissolving out that a move wants again comes back
                // as it is, ready.
                Some(&i) => {
                    let s = &mut self.slots[i];
                    s.role = Role::Building;
                    s.fade = None;
                    s.set(FRESH, true);
                }
                None => {
                    let id = self.next_mount_id;
                    self.next_mount_id = self.next_mount_id.wrapping_add(1).max(MOUNT_ID_BASE);
                    self.add(world, id, k);
                    self.counts.added += 1;
                }
            }
        }
        for &k in &plan.recube {
            if let Some(&i) = self.index.get(&k) {
                self.slots[i].recube = true;
                self.recheck(world, i, now);
            }
        }
    }

    /// The device's meshes are gone — the arena was made anew, the device with
    /// it — so every tier is forgotten and built again from the records.
    ///
    /// A layout is what a chunk shows, and it showed those tiers: kept ready
    /// over tiers that are gone, the recheck finds nothing to ask for and the
    /// chunk draws its decor over no ground until its cube happens to change.
    /// With its layout gone it is a hole, which the recheck asks for first.
    fn lose_meshes(&mut self, world: &mut World) {
        let epoch = arena_epoch();
        if epoch == self.meshes.epoch {
            return;
        }
        self.meshes.epoch = epoch;
        self.meshes.holder.fill(None);
        let leaving: Vec<Chunk> = self.slots.iter().filter(|s| s.role == Role::Leaving).map(|s| s.chunk).collect();
        for k in leaving {
            self.remove(world, k);
        }
        for s in self.slots.iter_mut() {
            s.tiers.clear();
            s.layout_ready = false;
            s.layout_secondary = None;
            s.fade = None;
            s.told = None;
        }
        self.view_dirty = true;
    }

    /// Lets go of the pieces whose dissolve out is over, and of any leaving
    /// piece that shows nothing.
    fn retire(&mut self, world: &mut World, now: f64) {
        let gone: Vec<Chunk> = self
            .slots
            .iter()
            .filter(|s| s.role == Role::Leaving && (!s.layout_ready || s.fade.is_none_or(|f| f.progress(now) >= 1.0)))
            .map(|s| s.chunk)
            .collect();
        for k in gone {
            self.remove(world, k);
        }
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, move the frontier, commit and
    /// reclaim under the frame's budget, and set what the build task takes.
    /// Whether the canvas has something new to draw.
    pub fn tick(&mut self, world: &mut World, frame: Frame, now: &impl Fn() -> f64) -> bool {
        let t0 = now();
        let deadline = t0 + frame.remaining_ms;
        let hard = deadline + frame.hard_slack_ms;
        if !self.frame.is_some_and(|prev| same_view(&prev, &frame)) {
            self.view_dirty = true;
        }
        self.frame = Some(frame);
        self.clock_ms += frame.delta_s * 1000.0;
        // What every chunk should be showing is read off the frame, the
        // domain and what the chunk was told, so a change to any of them is
        // answered now: `view_dirty` is that question outstanding — set by a
        // frame that differs from the last, a domain made again, and every
        // other input where it changes. Nothing is left to a timer to catch.
        let domains = domain::revision();
        if domains != self.domain_seen {
            self.domain_seen = domains;
            self.view_dirty = true;
        }

        self.lose_meshes(world);
        self.refresh(&frame);
        let mut sc = core::mem::take(&mut self.scratch);
        self.order(&mut sc.order, &mut sc.keys);

        // What each chunk should be showing, for every chunk that could have
        // changed: freshly mounted, mid-blend, or settling — and all of them
        // when an input they all read moved.
        //
        // Not under the budget, and that is the point. This decides what is
        // *wanted*; the budget is for the work that follows, and a pass that
        // states a demand cannot be cut without some chunks' demand going
        // unstated. Cut, it stopped at the order's head, and a chunk drawing
        // a coarse tier sorts behind every one drawing nothing, so it was
        // never reached: never told it wanted a finer tier, so never pending,
        // so never built.
        //
        // It costs a distance and a few flags a chunk, the arena's slots of
        // them at the worst, and only when an input moved.
        let full_recheck = self.view_dirty;
        sc.eligible.clear();
        sc.eligible
            .extend(sc.order.iter().copied().filter(|&i| full_recheck || self.slots[i].flags & (FRESH | BLEND_ACTIVE | SETTLING) != 0));
        for k in 0..sc.eligible.len() {
            let i = sc.eligible[k];
            self.recheck(world, i, t0);
        }
        if full_recheck {
            self.view_dirty = false;
        }

        self.materialize(world);
        self.retire(world, self.clock_ms);
        // Mounting and letting go move chunks in the table.
        self.order(&mut sc.order, &mut sc.keys);

        // The pending chunks, in the one order, on screen or not: a piece
        // off the screen is the frontier's as much as one on it, and is what
        // a turn finds drawn.
        sc.pending.clear();
        sc.pending.extend(sc.order.iter().copied().filter(|&i| self.slots[i].has(PENDING)));
        let pending_total = sc.pending.len() as u32;

        // A commit may build decor, and a block refused ends the commits this
        // tick as it ends the builds (Builder::tick): the heap is the same
        // until reclaim, and the decor is scattered before its block is
        // asked for.
        let starved = self.builder.stat.starved;
        let commits = under_budget(&sc.pending, param::COMMITS_PER_FRAME, deadline, hard, now, |i| {
            self.builder.stat.starved == starved && self.commit(world, i)
        });
        // What the pass never looked at, which is what a cap on how many may
        // land costs when there are more of them than it lets through.
        let commit_left = sc.pending.len() as u32 - commits.visited;
        let mut committed = commits.ran;

        // The builds are the build task's (`build_step`), on the turn's quota:
        // the pending, in the one order.
        let slots = &self.slots;
        self.builds.clear();
        self.builds.extend(sc.pending.iter().filter(|&&i| slots[i].has(PENDING)).map(|&i| slots[i].id));
        if !self.builds.is_empty() {
            builds().notify();
        }
        committed += core::mem::take(&mut self.build_committed);

        self.reclaim(&sc.order);
        self.scratch = sc;
        let animating = self.models.tick(&mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
        self.fade(self.clock_ms);
        self.draw_frame(&frame);

        let mut c = Census {
            chunks: self.slots.len() as u32,
            commit_left,
            refused: self.moves.0,
            waiting: self.moves.1,
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
            match s.role {
                Role::Leaving => {
                    c.leaving += 1;
                    if s.has(IN_FRUSTUM) && s.layout_ready {
                        c.leaving_drawn += 1;
                        c.leaving_levels |= 1 << s.level();
                    }
                }
                Role::Building if s.layout_ready => c.ready_waiting += 1,
                _ => {}
            }
            if s.has(PENDING) && s.pending_since >= 0.0 {
                let waited = t0 - s.pending_since;
                if s.has(IN_FRUSTUM) {
                    c.oldest_visible_ms = c.oldest_visible_ms.max(waited);
                } else {
                    c.offscreen_pending += 1;
                    c.oldest_offscreen_ms = c.oldest_offscreen_ms.max(waited);
                }
            }
        }
        c.level_boxes = self.level_boxes;
        c.slot_waits = core::mem::take(&mut self.meshes.waits);
        c.wait_levels = core::mem::take(&mut self.meshes.wait_levels);
        c.coarser = core::mem::take(&mut self.meshes.coarser);
        c.fine_peak = core::mem::take(&mut self.meshes.fine_peak);
        c.coarse_peak = core::mem::take(&mut self.meshes.coarse_peak);
        c.retained_on = self.retained_on;
        c.retained_off = self.retained_off;
        let blending = c.blend_active > 0 || c.leaving > 0;
        let moving = c.waiting > 0 || c.ready_waiting > 0;
        self.census = c;
        committed > 0 || pending_total > committed || blending || moving || self.view_dirty || animating
    }

    /// This frame's draws: every piece drawn or leaving that is on screen
    /// with a layout, its tiers at the weights the blend decided, placed where
    /// the chunk is in the frame the camera is written in. A chunk built for
    /// a move is not drawn until the move is made: the piece it replaces is
    /// what covers its ground until then.
    fn draw_frame(&mut self, frame: &Frame) {
        self.draw.begin_frame();
        self.level_boxes = [0; LEVELS];
        self.ghost.draw(&mut self.draw, frame.origin);
        self.models.draw(&mut self.draw, frame.origin);
        self.markers.draw(&mut self.draw, frame.origin);
        self.debug.draw(&mut self.draw, frame.origin);
        let holder = &self.meshes.holder;
        for s in &self.slots {
            if s.role == Role::Building || !s.has(IN_FRUSTUM) || !s.layout_ready {
                continue;
            }
            let (wp, ws) = s.told.map_or((1.0, 0.0), |t| (t.0, t.1));
            let translation = DrawList::translation([s.base_x(), s.base_y(), 0.0], frame.origin);
            for (cube, visible) in [(Some(s.layout_primary), wp), (s.layout_secondary, ws)] {
                let Some(cube) = cube else { continue };
                // A tier's decor dissolves with its tier, on the same half of
                // the pattern, so an instance of both is on one pixel as the
                // ground is and never stands twice.
                if let Some(d) = s.decor_of(cube, s.layout_generation) {
                    for (group, (offset, count)) in d.groups().enumerate() {
                        if count > 0 {
                            self.draw.draw_decor(d.handle, group as u32, count, offset, translation, visible);
                        }
                    }
                }
                let Some(t) = s.tier(cube, s.layout_generation) else { continue };
                // A chunk draws only from a slot it holds. The slot is room
                // and not an address (world/chunk/mesh_slot.rs), so it is
                // handed from one chunk to another while the kernel runs —
                // and a long-lived thing holding a recyclable address is how
                // one chunk comes to draw another's ground. It cannot here:
                // the only reference to a slot is the `Tier` inside the chunk
                // that took it, a slot is free only once that tier is gone,
                // and this looks the tier up again every frame. The assert is
                // what says so out loud, at an array index a draw.
                xpute_core::ensure!(holder[t.slot.index].map(|h| h.1) == Some(s.id), ENOTRECOVERABLE);
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
                    (mesh_step(cube, s.level()) << s.level()) as f32,
                    t.heights.step as f32,
                    group_is_fine(t.slot.group),
                );
                self.level_boxes[s.level() as usize] += t.box_cnt;
            }
        }
    }

    /// Piece `s` as it is drawn now, as the block of the pyramid's cells it
    /// draws. During a crossfade both tiers are drawn and the one covering
    /// more is the chunk's; a chunk that is not a drawn piece, or has no
    /// layout, draws nothing a layer could stand on.
    fn drawn_of<'a>(&'a self, s: &'a Slot) -> Option<Drawn<'a>> {
        if s.role != Role::Drawn || !s.layout_ready {
            return None;
        }
        let cube = shown_cube(s);
        let t = s.tier(cube, s.layout_generation)?;
        Some(Drawn {
            level: s.level(),
            base: s.chunk.corner(),
            step: mesh_step(cube, s.level()),
            tops: surface(t.slot.index, mesh_step(cube, s.level())),
            heights: t.heights,
        })
    }

    /// The ground drawn now at `addr`: the level of the cells it is drawn in
    /// there, and the top of that cell's surface in units. Read from the
    /// piece `drawn_of` gives for that ground, so it is the surface on the
    /// screen — the tier a crossfade shows more of, a coarser piece while
    /// what replaces it builds — and none where nothing drawn stands there.
    ///
    /// A column is drawn by every chunk over it that holds something, each
    /// cut to its own heights (world/pyramid/chunk.rs), so a chunk under the
    /// surface tops out where it is cut. The highest one drawn there is the
    /// one whose top is the surface's.
    pub fn surface_at(&self, at: &Pvf3) -> Option<(u32, f64)> {
        (0..=TOP_LEVEL).find_map(|level| {
            self.index.range(Chunk::column_of(level, at)).rev().find_map(|(_, &i)| {
                let d = self.drawn_of(&self.slots[i])?;
                // The chunk lies in one page at every level over its own, so
                // the point's cell and the chunk's first are of one page.
                let [cx, cy] = GROUND.cell_of(d.cell_level(), at).in_page();
                let [fx, fy] = GROUND.cell_of(d.cell_level(), &Pvf3 { i: d.base, ..Default::default() }).in_page();
                Some((d.cell_level(), d.top(cx - fx, cy - fy)?))
            })
        })
    }

    /// Where every chunk is and whether it is on screen, measured against the
    /// frame. A slot that crosses the frustum's edge moves its retained bytes
    /// between the on-screen and off-screen totals, which is what the
    /// retention budget is spent from.
    fn refresh(&mut self, frame: &Frame) {
        let domain = domain::current();
        for i in 0..self.slots.len() {
            let visible = measure(frame, &self.slots[i]).0;
            let order = domain.map_or(Order::LAST, |d| d.order_of(self.slots[i].chunk));
            let s = &mut self.slots[i];
            s.order = order;
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
                self.view_dirty = true;
            }
        }
    }

    /// Every slot's index in priority order (`priority`) into `idx`, each
    /// slot's key read once into `keys` rather than in the comparator.
    fn order(&self, idx: &mut Vec<usize>, keys: &mut Vec<Priority>) {
        let holders = self.builder.holders();
        keys.clear();
        keys.extend(self.slots.iter().map(|s| Priority {
            holds: holders.contains(&Some(s.id)),
            order: s.order,
        }));
        idx.clear();
        idx.extend(0..self.slots.len());
        // Ties broken by the slot, which is the order a stable sort keeps: the
        // same order, with no merge buffer taken each tick.
        idx.sort_unstable_by(|&a, &b| priority(&keys[a], &keys[b]).then(a.cmp(&b)));
    }

    fn set_pending(&mut self, i: usize, pending: bool, now: f64) {
        let s = &mut self.slots[i];
        if pending && !s.has(PENDING) {
            s.pending_since = now;
        }
        if !pending {
            s.pending_since = -1.0;
        }
        s.set(PENDING, pending);
    }

    fn stop_work(&mut self, world: &mut World, i: usize) {
        self.builder.want(world, self.slots[i].id, [None, None]);
    }

    /// Every dissolve's weights as it stands at `now`, and the ends of those
    /// that are over: a tier's secondary let go, a piece come in shown whole.
    /// A leaving piece whose dissolve is over is `retire`'s to let go. A
    /// chunk in one is blending, which asks for the next frame.
    fn fade(&mut self, now: f64) {
        for i in 0..self.slots.len() {
            let Some(fade) = self.slots[i].fade else { continue };
            let t = fade.progress(now) as f32;
            let active = match fade {
                Fade::Tier(_) if t < 1.0 && self.slots[i].layout_secondary.is_some() => {
                    self.decide(i, t, 2.0 + t);
                    true
                }
                Fade::Tier(_) => {
                    let s = &self.slots[i];
                    let (p, g, ready) = (s.layout_primary, s.layout_generation, s.layout_ready);
                    self.slots[i].fade = None;
                    if ready {
                        self.apply_layout(i, p, None, g);
                        self.trim(i);
                    }
                    self.decide(i, 1.0, 0.0);
                    false
                }
                Fade::In(_) if t < 1.0 => {
                    self.decide(i, t, 0.0);
                    true
                }
                Fade::In(_) => {
                    self.slots[i].fade = None;
                    self.decide(i, 1.0, 0.0);
                    false
                }
                Fade::Out(_) => {
                    self.decide(i, 2.0 + t, 0.0);
                    true
                }
            };
            self.slots[i].set(BLEND_ACTIVE, active);
        }
    }

    fn decide(&mut self, i: usize, primary: f32, secondary: f32) {
        let coverage = |v: f32| if v > 1.5 { 1.0 - (v - 2.0) } else { v };
        let mostly_secondary = self.slots[i].layout_secondary.is_some() && coverage(secondary) > coverage(primary);
        self.slots[i].told = Some((primary, secondary, mostly_secondary));
    }

    /// The tier this chunk should be showing, and whether that makes it
    /// pending: the cube the rule wants at its nearest distance at the
    /// effective quality (frontier.rs `wanted_cube`), no finer than its tiers
    /// fit their slot. Pending while what it shows is not what it builds
    /// (`Slot::target`), of its sources as they are now, with its decor on
    /// screen: a drawn piece wanting another cube is the frontier's to let
    /// (`materialize`), and is not pending until it is. A chunk built for a
    /// move drops a tier the rule no longer wants rather than hold two, as
    /// nothing of it is drawn yet. SETTLING is pending with a layout already
    /// up, so a frame can tell a chunk that is changing tier from one that
    /// has nothing to show yet.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64) {
        self.slots[i].set(FRESH, false);
        if self.slots[i].role == Role::Leaving {
            self.set_pending(i, false, now);
            self.stop_work(world, i);
            return;
        }
        let Some(d) = domain::current() else { return };
        let s = &self.slots[i];
        let (level, chunk) = (s.level(), s.chunk);
        let objects = level == 0 && world.store.entries(L2_IDX(s.chunk.corner())).is_some();
        let current = self.generation(i);
        let cube = drawn_cube(wanted_cube(d.nearest_to(chunk), d.tau()), level, d.outermost(chunk), objects);
        let desired = s.fit_at(current).map_or(cube, |f| cube.max(f));
        if self.slots[i].role == Role::Building && self.slots[i].layout_ready && self.slots[i].layout_primary != desired {
            self.release_tiers(i, |_, _| false);
        }
        let s = &mut self.slots[i];
        s.desired = desired;
        if !s.recubes() {
            s.recube = false;
        }
        let pending = !s.layout_ready || s.layout_primary != s.target() || s.layout_generation != current || (s.has(IN_FRUSTUM) && !s.decor_whole);
        self.set_pending(i, pending, now);
        let s = &mut self.slots[i];
        let settling = s.has(PENDING) && s.layout_ready;
        s.set(SETTLING, settling);
        s.set(BLEND_ACTIVE, false);
        if !s.has(PENDING) {
            self.stop_work(world, i);
        }
        if self.slots[i].fade.is_none() {
            self.decide(i, 1.0, 0.0);
        }
    }

    /// Lets go of chunk `i`'s tiers and decor but those `keep` names, and of
    /// the slots only they held; its layout too where its primary goes.
    fn release_tiers(&mut self, i: usize, keep: impl Fn(Cube, u32) -> bool) {
        let before = self.slots[i].retained();
        let s = &mut self.slots[i];
        let draw = &mut self.draw;
        s.decor.retain(|d| {
            let kept = keep(d.cube, d.generation);
            if !kept {
                d.release(draw);
            }
            kept
        });
        let (tiers, gone): (Vec<Tier>, Vec<Tier>) = s.tiers.iter().partition(|t| keep(t.cube, t.generation));
        s.tiers = tiers;
        for t in gone {
            let shared = s.tiers.iter().any(|k| k.slot.index == t.slot.index);
            let held = &mut self.meshes.holder[t.slot.index];
            if !shared && held.is_some_and(|(_, h)| h == s.id) {
                *held = None;
            }
        }
        if s.tier(s.layout_primary, s.layout_generation).is_none() {
            s.layout_ready = false;
            s.layout_secondary = None;
            s.decor_whole = false;
        }
        self.account(i, before);
    }

    /// Whether the chunk holds the tier `primary` of the current generation.
    /// When not, the builder is asked for it.
    fn ask(&mut self, world: &mut World, i: usize, primary: Cube) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let lacks = s.tier(primary, generation).is_none();
        self.builder.want(world, s.id, [lacks.then_some(primary), None]);
        !lacks
    }

    /// Shows (primary, secondary) of `generation`. What it no longer draws —
    /// an older generation's tiers and decor among it, so what is drawn
    /// changes whole — is `trim`'s to let go, after.
    fn apply_layout(&mut self, i: usize, primary: Cube, secondary: Option<Cube>, generation: u32) {
        self.counts.layouts += 1;
        let s = &mut self.slots[i];
        s.layout_primary = primary;
        s.layout_secondary = secondary.filter(|&q| q != primary);
        s.layout_generation = generation;
        s.layout_ready = true;
        s.told = None;
        let mut b: Option<Bounds> = None;
        for c in [Some(s.layout_primary), s.layout_secondary].into_iter().flatten() {
            if let Some(Some(t)) = s.tier(c, generation).map(|t| t.bounds) {
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

    /// Lets go of every tier the layout does not draw, and its decor: a slot
    /// is held only by what is on the screen or about to be.
    fn trim(&mut self, i: usize) {
        let s = &self.slots[i];
        let (p, q, g) = (s.layout_primary, s.layout_secondary, s.layout_generation);
        self.release_tiers(i, move |cube, generation| generation == g && (cube == p || Some(cube) == q));
    }

    /// Applies what `recheck` asked for, if it can: the desired tier once it
    /// is built, with its decor. A drawn piece changes cube only once the
    /// frontier has let it hold a slot for it (`Slot::recube`). A request that
    /// was refused for want of a block is not retried until the heap has
    /// released something, so a full heap costs one attempt rather than one
    /// per frame.
    fn commit(&mut self, world: &mut World, i: usize) -> bool {
        let s = &self.slots[i];
        if !s.has(PENDING) || s.role == Role::Leaving {
            return false;
        }
        if s.refused.is_some_and(|r| r == crate::host::heap_stat().releases) {
            return false;
        }
        let (p, generation) = (s.target(), self.generation(i));
        let asked = self.ask(world, i, p);
        if !asked || !self.ensure_decor(world, i, [Some(p), None], generation) {
            return false;
        }
        // A new cube of the same sources dissolves from the one shown, which
        // stays as the secondary tier until it has gone (`fade`). A change of
        // sources — a cube placed — is shown at once.
        let s = &self.slots[i];
        let was = (settings_store().lod_crossfade
            && s.role == Role::Drawn
            && s.layout_ready
            && s.layout_generation == generation
            && s.layout_primary != p
            && s.fade.is_none()
            && s.tier(s.layout_primary, generation).is_some())
        .then_some(s.layout_primary);
        self.apply_layout(i, p, was, generation);
        self.trim(i);
        let s = &mut self.slots[i];
        s.recube = s.recubes() && s.recube;
        s.decor_whole = true;
        if was.is_some() {
            s.fade = Some(Fade::Tier(self.clock_ms));
        } else if matches!(s.fade, Some(Fade::Tier(_))) {
            s.fade = None;
        }
        self.set_pending(i, false, 0.0);
        true
    }

    /// The retain budget: off-screen decor past it, or the heap pressed
    /// (`heap_pressed`), is given back farthest first. A block refused since
    /// the last pass is pressure too, whatever the numbers say: the heap has
    /// room in bytes but not in one piece, and what gives it back in pieces
    /// that join is the off-screen decor, farthest first. Decor is all a
    /// chunk holds in the heap: its runs are `.fixed.runs`'s and its tiers
    /// are on the device. What is given back is built again once its chunk
    /// is on screen (`recheck`).
    fn reclaim(&mut self, order: &[usize]) {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let refused = self.builder.stat.starved != self.refused_seen;
        self.refused_seen = self.builder.stat.starved;
        let cap = if refused { param::RECLAIMS_WHEN_REFUSED } else { param::RECLAIMS_PER_FRAME };
        let mut freed = 0;
        // Farthest first: the order puts what the view does not see last,
        // the farthest of it at the end.
        for &i in order.iter().rev() {
            if freed >= cap || !(refused || self.retained_off > budget || heap_pressed()) {
                break;
            }
            if self.slots[i].has(IN_FRUSTUM) || self.slots[i].retained() <= 0.0 {
                continue;
            }
            let before = self.slots[i].retained();
            let s = &mut self.slots[i];
            let draw = &mut self.draw;
            for d in s.decor.drain(..) {
                d.release(draw);
            }
            s.decor_whole = false;
            self.account(i, before);
            freed += 1;
        }
    }
}

/// Ids the scheduler gives the chunks it mounts.
const MOUNT_ID_BASE: u32 = 1 << 31;
/// Whether two frames look at the world the same way: what a recheck's
/// answer depends on, not the budget or the elapsed time.
fn same_view(a: &Frame, b: &Frame) -> bool {
    a.root == b.root && a.origin == b.origin && a.planes == b.planes && a.camera == b.camera
}

/// What a chunk's place in the order is decided by.
#[derive(Clone, Copy)]
struct Priority {
    holds: bool,
    order: Order,
}

/// The order work is taken in: the slots' holders, which the rest wait on;
/// then the view's own order (world/observation.rs `Order`) — what it sees,
/// nearest the eye first, then what it does not, nearest first too, since
/// that is the frontier a turn finds — the one order every layer's work is
/// taken in.
///
/// Only the distance, and no class over it. The view fills out from the
/// eye, and each piece it reaches replaces what was drawn there, coarser:
/// what is in front of the eye answers the camera first, and nothing further
/// out is worth its wait. Classes over it — a chunk showing nothing before
/// one showing an older generation, before one asking to be sharper — and a
/// wait that halved the key every four seconds sent the builds to ground at
/// the edge of the view while the middle stood coarse, which is the response
/// the view is judged by (2026-09-26).
///
/// A plain distance and not one in the chunk's own sides. That was "the
/// biggest on the screen first", and it is the same thing for every level:
/// the rule puts a chunk where its own cells are the right size, so a
/// coarse one at the horizon subtends about what a fine one under the camera
/// does, and they came out equal. The screen then filled everywhere at once
/// rather than from the middle — and everywhere at once means the middle
/// waits for all of it.
fn priority(a: &Priority, b: &Priority) -> core::cmp::Ordering {
    b.holds.cmp(&a.holds).then(a.order.cmp(&b.order))
}

/// Where a slot is from a frame: on screen, and its world center.
fn measure(frame: &Frame, s: &Slot) -> (bool, V3) {
    let [x, y, z] = frame.world_of([s.base_x() + s.bc[0], s.base_y() + s.bc[1], s.bc[2]]);
    // Where the renderer draws it: laid back on the planet about the view
    // center, the world's origin here, in the map's own axes — the view's
    // turned back by the azimuth and forward again. A far chunk culled where
    // the flat world has it would leave a hole at the horizon. The root's
    // first column is the azimuth's cosine and sine.
    let [x, y, z] = match frame.globe {
        Some(globe) => globe.place_turned([x, y, z], frame.root[1], frame.root[0]),
        None => [x, y, z],
    };
    let c = frame.camera;
    // A chunk the camera stands in is seen whatever the planes say of its
    // center: in its sphere, not over its footprint, since the chunks over
    // a column above and below the camera are not around it.
    let mut visible = (c[0] - x).hypot(c[1] - y).hypot(c[2] - z) <= s.radius;
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
        // the planet's center — the virtual globe's own cull (Cozzi & Ring),
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

/// A free slot of `pool`: one no tier holds, or one whose holder has gone.
/// Never one a drawn tier holds — a slot a piece on the screen draws from is
/// that ground's only cover — and never needed to be: the frontier starts a
/// build only where a free slot is held for it (frontier.rs).
fn claim(slots: &[Slot], index: &BTreeMap<Chunk, usize>, meshes: &Meshes, pool: Pool) -> Option<MeshSlot> {
    pool.range().find(|&at| meshes.holder[at].is_none_or(|(k, id)| slot_of(slots, index, k, id).is_none())).map(slot_at)
}

/// The slot chunk `k` is in while it is the mounting `id` names, and none
/// once that mounting is gone: what was built for a chunk unmounted and
/// mounted again is not the new one's.
fn slot_of(slots: &[Slot], index: &BTreeMap<Chunk, usize>, k: Chunk, id: u32) -> Option<usize> {
    index.get(&k).copied().filter(|&i| slots[i].id == id)
}

/// Records what a build handed over on the slot it belongs to, and passes
/// it on.
fn self_record(slots: &mut [Slot], index: &BTreeMap<Chunk, usize>, meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Tier {
        chunk,
        id,
        generation,
        cube,
        at,
        box_cnt,
        heights,
        ..
    } = e
    else {
        return;
    };
    let Some(i) = slot_of(slots, index, chunk, id) else { return };
    let s = &slots[i];
    // Only what the chunk builds now lands: a leaving piece builds nothing,
    // and a drawn one a new cube only once a slot is held for it.
    if s.tier(cube, generation).is_some() || s.role == Role::Leaving || cube != s.target() {
        return;
    }
    let level = s.level();
    // The slot this tier is rebuilt into, or a free one of its pool. A tier
    // keeps the slot it took, so a rebuild writes over its own boxes and
    // nothing else moves.
    let slot = match s.tiers.iter().find(|t| t.cube == cube).map(|t| t.slot) {
        Some(slot) => slot,
        None => match claim(slots, index, meshes, Pool::of(level, mesh_step(cube, level))) {
            Some(slot) => slot,
            None => {
                // The pool it was counted in is not the one it landed in — a
                // mesh baked coarser moves a z16 piece to the coarse pool —
                // and that one is full. It waits, thrown away and asked for
                // again, with what it would replace still drawn.
                meshes.waits += 1;
                meshes.wait_levels |= 1 << level;
                return;
            }
        },
    };
    // A mesh past its slot is never handed over (build.rs
    // `BuildEvent::Coarser`).
    xpute_core::ensure!(box_cnt <= slot.capacity, ENOTRECOVERABLE, box_cnt, slot.capacity);
    let boxes = box_cnt;
    let asked = if slot.capacity == FINE_BOXES { &mut meshes.fine_peak } else { &mut meshes.coarse_peak };
    *asked = (*asked).max(box_cnt);
    // SAFETY: the mesh's boxes, a least corner and a size in the
    // first six floats of each, in `.fixed.bake` where the bake
    // just wrote them.
    // The format is the group's, not the mesh's: a slot is a
    // stride into its group's arena (mesh_slot.rs).
    let box_words = group_box_words(slot.group);
    let words = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(at as usize), (boxes * box_words) as usize) };
    let bounds = (boxes > 0).then(|| {
        let mut b = Bounds {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        };
        let unit = (mesh_step(cube, level) << level) as f64;
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
        let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), words.len() * 4) };
        // No device to write to: asked for again once there is one.
        if !arena_write(slot.group as usize, slot.first_box * box_words * 4, bytes) {
            return;
        }
        // The write that made the arena lost nothing written before it.
        if arena_epoch() != before && before == meshes.epoch {
            meshes.epoch = arena_epoch();
        }
    }
    surface_centers(slot.index, words, box_words as usize, mesh_step(cube, level), level, heights);
    meshes.holder[slot.index] = Some((chunk, id));
    let s = &mut slots[i];
    // A tier of an older generation in the same slot shows these boxes
    // now: they were written over its own.
    for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
        t.box_cnt = boxes;
        t.bounds = bounds;
        t.heights = heights;
    }
    s.tiers.push(Tier {
        cube,
        generation,
        bounds,
        slot,
        box_cnt: boxes,
        heights,
    });
}

/// Slot `index`'s surface as its tier of `step` holds it, a word a block:
/// the lane's first (CHUNK_CELLS / step)² words. Empty under a native test,
/// which lays no memory out.
fn surface(index: usize, step: u32) -> &'static mut [u32] {
    let (off, words) = surface_of(index);
    let axis = CHUNK_CELLS / step;
    xpute_core::ensure!(axis * axis <= words, ENOTRECOVERABLE, index, step);
    // SAFETY: the slot's own lane in `.fixed.surface`, written only by the
    // tier landing in the slot and read by what stands on it, both the chunk
    // table's, on its one thread.
    range(Range::Surface).map_or(&mut [], |s| unsafe { lane(s, off, (axis * axis) as usize) })
}

/// A cell-center height query needs no CPU copy of the mesh. Quantized tops
/// use the mesh's own height scale, with zero reserved for an empty center.
/// Written into the slot's own lane, which the slot's tier sizes
/// (mesh_slot.rs `surface_of`).
fn surface_centers(index: usize, words: &[u32], stride: usize, step: u32, level: u32, heights: Heights) {
    let axis = (CHUNK_CELLS / step) as usize;
    let unit = step as f64 * GROUND.scale(level).side();
    let tops = surface(index, step);
    tops.fill(0);
    if tops.is_empty() {
        return;
    }
    for r in words.chunks_exact(stride) {
        let (min, size) = box_bounds(r, unit, heights);
        let x0 = ((min[0] / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let y0 = ((min[1] / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let x1 = (((min[0] + size[0]) / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let y1 = (((min[1] + size[1]) / unit - 0.5).ceil().max(0.0) as usize).min(axis);
        let (lo, span) = box_z(r);
        let top = lo + span + 1;
        for y in y0..y1 {
            for x in x0..x1 {
                tops[y * axis + x] = tops[y * axis + x].max(top);
            }
        }
    }
}

/// A chunk as it is drawn: its level, the unit address its first cell
/// begins at, the step its drawn tier's cells are, and that tier's surface.
/// What a layer standing things on the drawn ground reads
/// (`Scheduler::surface_at`, a decor build's surface).
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

    /// The top of the drawn surface under a point `x`, `y` units from the
    /// chunk's corner: the block's it lies in.
    pub fn top_under(&self, x: f64, y: f64) -> Option<f64> {
        let unit = self.step as f64 * GROUND.scale(self.level).side();
        self.top((x / unit) as u32, (y / unit) as u32)
    }

    /// The top of the drawn surface at the center of the `step` block at
    /// (bx, by), in units; none where that center is empty. Samples share the
    /// GPU mesh slot's lifetime, including a leaving piece's.
    pub fn top(&self, bx: u32, by: u32) -> Option<f64> {
        let axis = CHUNK_CELLS / self.step;
        let top = (*self.tops.get((by * axis + bx) as usize)?).checked_sub(1)?;
        Some(self.heights.floor + top as f64 * self.heights.step)
    }
}
