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
//! A chunk has one owner, its slot here: the tiers built for it and what
//! they were made from, against which `sources_step` finds a change. A chunk keeps only the tiers it draws and the one it is building:
//! every other is made again from the records if it is wanted, so a slot is
//! never held by what is not on the screen. The builder (build.rs) makes the
//! tiers; the chunk stage (stage.rs) composes the frame — the camera, the
//! anchor root's matrix, the frustum — and calls the tick.

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch, arena_write};
use pxl_render::look::globe::Globe;
use pxl_render::mesh::chunk_mesher::{box_bounds, Heights};
use pxl_render::mesh::paint::terrain::adapter::beside_index;
use pxl_render::mesh::voxel_scan::APRON;

use pxl_spec::space::observation::Bounds as _;
use pxl_spec::space::v3::V3;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::section::span_of;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use crate::global::Global;
use crate::layer::billboard::field::Billboards;
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::debug_lines::DebugLines;
use crate::layer::marker::hover_cursor::GhostDraw;
use crate::layer::marker::location_puck::Markers;
use crate::layer::model::store::Models;
use crate::layer::object::store::{render_edits, EntryAt, RenderEdits};
use crate::mem::fixed::{lane, range, Range};
use crate::param;
use crate::sched::profiler::record::MB;
use crate::shell::setting::store::settings_store;
use crate::world::chunk::build::{mesh_step, BuildEvent, Builder, Cube, World};
use crate::world::chunk::frontier::{self, cube_of, drawn_cube, pool_of, wanted_cube, Env, Frontier, Now, Slots};
use crate::world::chunk::mesh_slot::{capacity, group_box_words, group_bytes, group_is_fine, slot_at, MeshSlot, Pool, ARENA, COVER_SLOTS, FINE_BOXES, OCCUPANCY};
use crate::world::chunk::source::{draws_band, objects_revision_of, tiers_draw};
use crate::world::paging::LEVELS;
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use crate::world::observation::{Known, Order};
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::indexer::L2_IDX;

pub const IN_FRUSTUM: u16 = 1;
pub const PENDING: u16 = 4;
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
}

/// What the chunk stage composes before a tick.
#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The globe's anchor in the root frame (world/frame/view.rs
    /// `globe_anchor`): what the chunks are measured about, since the map is
    /// laid back on the planet about it.
    pub anchor: V3,
    /// The map origin, in units.
    pub origin: V3,
    /// The eye, in units: what a draw's translation is taken from, so every
    /// point the GPU holds is relative to it (pxl-render frame/draw.rs
    /// `DrawList::translation`).
    pub eye: V3,
    /// The frustum as six planes of (nx, ny, nz, constant), and the eye,
    /// about the anchor in the world's axes.
    pub planes: [f64; 24],
    pub camera: V3,
    pub delta_s: f64,
    /// What is left of the turn's quota when the pass begins, which is not
    /// the quota itself (`FrameSample::budget_ms`).
    pub remaining_ms: f64,
    pub hard_slack_ms: f64,
    /// The globe's anchor's height, in units: what the camera's height over
    /// the sea is measured up from.
    pub center_z: f64,
    /// The map center's latitude's values, which the renderer lays the map
    /// back on the planet about (pxl-render's look/globe.rs); none for a flat
    /// world.
    pub globe: Option<Globe>,
}

impl Frame {
    /// Where `at`, in the map's units, is about the anchor: against the
    /// origin, then less the anchor.
    pub fn world_of(&self, at: V3) -> V3 {
        [0, 1, 2].map(|k| at[k] - self.origin[k] - self.anchor[k])
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
    /// Where its boxes lie on the device, and how many: the first of the
    /// slots it fills, the rest chained after it (`Meshes::next`), a slot's
    /// worth each but the last.
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
pub struct ChunkHeld {
    /// Chunks holding a run set, and what those sets cost in the heap.
    pub run_sets: u32,
    pub run_set_mb: u32,
    /// Built tiers held across every chunk.
    pub tiers: u32,
}

#[derive(Clone)]
struct Slot {
    /// Its mount (`Scheduler::place`).
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
    /// How many slots a tier of this chunk fills, of the sources generation
    /// and the cube it was found for: found where a bake had more boxes than
    /// the slots free could hold (build.rs `BuildEvent::Reserve`), counted
    /// by the frontier and the quality policy from then on, and not asked
    /// for again until that many are free. A new generation finds it again.
    need: Option<(u32, Cube, u32)>,
    /// A drawn piece the frontier has let build a tier other than the one it
    /// shows: a slot is held for it (frontier.rs `Plan::recube`).
    recube: bool,
    /// The tiers built for it: those its layout draws, and the last to land
    /// of what it builds. A source changed leaves the layout's where they
    /// are: a cube's tier of the new generation joins the old one's, and the
    /// old go when a layout of the new is applied.
    tiers: Held<Tier>,
    layout_primary: Cube,
    /// The generation the layout's tiers are drawn from.
    layout_generation: u32,
    layout_ready: bool,
    /// The cube the rule wants it drawn in now.
    desired: Cube,
    /// What the builder was last told the chunk is made of: a change is a
    /// rebuild (`sources_step`).
    sources: Sources,
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
    /// page arriving builds it again from its own ground; and the same of
    /// each page beside it its apron reads (build.rs `Read`), so the edge it
    /// shares with a chunk there is made again when that chunk's ground is.
    read_from: Option<u32>,
    beside_from: [Option<u32>; 8],
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
            beside_from: {
                let mut from = [None; 8];
                for ([qx, qy], p) in k.pages_beside(APRON) {
                    from[beside_index(qx, qy)] = Terrain::pyramid().drawn_from(p).map(|p| p.level);
                }
                from
            },
        }
    }
}

impl Slot {
    /// The level it is drawn from.
    fn level(&self) -> u32 {
        self.chunk.level()
    }

    /// Its corner, in units.
    fn base(&self) -> (f64, f64) {
        let (x, y, _) = self.chunk.rect();
        (x, y)
    }

    /// The sphere the chunk is culled by until its mesh has measured it: the
    /// box the view measured it by (domain.rs `Domain::occupied`), where in
    /// it anything stands as far as what is known says, and the chunk whole
    /// where nothing is known. A chunk
    /// culled until it is built is never built, so it is never measured and
    /// stays culled: a sphere at a corner missed every view that missed the
    /// corner, and one at sea level missed every chunk of a mountain, whose
    /// ground stood hundreds of units over it, and left the middle of the
    /// view empty with nothing left to fetch.
    fn unmeasured(&mut self) {
        let region = self.chunk.region();
        let b = Known.contents(self.chunk.level(), region).within(region).unwrap_or(region);
        let (c, e) = (b.center(), b.extent());
        self.bc = [c[0] - region.min[0], c[1] - region.min[1], c[2]];
        self.radius = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt() * 0.5;
    }

    /// How many slots its tier of `cube` fills, of the sources as of
    /// `generation`: what is known (`need`), else one.
    fn slots_for(&self, cube: Cube, generation: u32) -> u32 {
        self.need.filter(|&(g, c, _)| g == generation && c == cube).map_or(1, |(_, _, k)| k)
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

    fn tier(&self, cube: Cube, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.cube == cube && t.generation == generation)
    }

    /// The pool its desired tier takes.
    fn desired_pool(&self) -> Pool {
        Pool::of(self.level(), mesh_step(self.desired, self.level()))
    }

    /// The slots of its pool its desired tier fills, of the sources as of
    /// `generation`.
    fn desired_slots(&self, generation: u32) -> (Pool, u32) {
        (self.desired_pool(), self.slots_for(self.desired, generation))
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

    /// The pool slots are held in for it and not yet taken, and how many:
    /// what it builds has not landed, and it has no tier of that cube to
    /// build into.
    fn reserves(&self, generation: u32) -> Option<(Pool, u32)> {
        let cube = self.target();
        (!self.tiers.iter().any(|t| t.cube == cube)).then(|| (Pool::of(self.level(), mesh_step(cube, self.level())), self.slots_for(cube, generation)))
    }
}

/// The census the profiler reads.
#[derive(Clone, Copy, Default)]
pub struct Census {
    /// Chunks the table holds, which the arena bounds.
    pub chunks: u32,
    pub in_frustum: u32,
    pub settling: u32,
    /// Chunks still pending once the frame is done. Not what the frame began
    /// with less what it committed, which would also count the chunks a build
    /// landed on this frame, pending or not when it began.
    pub pending: u32,
    pub offscreen_pending: u32,
    pub oldest_visible_ms: f64,
    pub oldest_offscreen_ms: f64,
    /// Chunks built for a move and ready, waiting on the rest of it.
    pub ready_waiting: u32,
    /// Meshes thrown away since the last census because their pool had no
    /// free slot, and the levels of the chunks they were for, as bits.
    pub slot_waits: u32,
    pub wait_levels: u32,
    /// Tiers landed across more than one slot since the last census: they
    /// had more boxes than one slot holds (mesh_slot.rs `capacity`).
    pub chained: u32,
    /// The largest mesh handed over since the last census, in boxes, of a
    /// fine slot and of a coarse one.
    pub fine_peak: u32,
    pub coarse_peak: u32,
    /// Moves of the frontier the arena had no room for this frame, and moves
    /// under way and not all ready (frontier.rs `Plan`).
    pub refused: u32,
    pub waiting: u32,
    /// The boxes the last draw listed, by the level of the chunk they are of.
    pub level_boxes: [u32; LEVELS as usize],
    /// Chunks wanting a layout that the commit pass did not reach: whether
    /// the frame's cap is what holds the screen back.
    pub commit_left: u32,
}

/// A slot's or a mount's number in a lane of `.fixed.mounts`, or none: one
/// past the number, so a lane as it is laid, zeroed, reads as none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
struct Nth(u32);

impl Nth {
    const NONE: Nth = Nth(0);

    fn of(n: usize) -> Nth {
        Nth(n as u32 + 1)
    }

    fn get(self) -> Option<usize> {
        self.0.checked_sub(1).map(|n| n as usize)
    }

    fn is_none(self) -> bool {
        self == Nth::NONE
    }

    fn take(&mut self) -> Option<usize> {
        core::mem::take(self).get()
    }
}

/// The mounts free, the last given back first.
struct Free {
    lane: &'static mut [u32],
    len: usize,
}

impl Free {
    fn pop(&mut self) -> Option<u32> {
        self.len = self.len.checked_sub(1)?;
        Some(self.lane[self.len])
    }

    fn push(&mut self, id: u32) {
        self.lane[self.len] = id;
        self.len += 1;
    }
}

/// The chunks mounted in chunk order, each with its mount: a sorted run of
/// a lane as long as there are mounts.
struct Index {
    lane: &'static mut [(Chunk, u32)],
    len: usize,
}

impl Index {
    fn insert(&mut self, n: usize, entry: (Chunk, u32)) {
        self.lane.copy_within(n..self.len, n + 1);
        self.lane[n] = entry;
        self.len += 1;
    }

    fn remove(&mut self, n: usize) -> (Chunk, u32) {
        let entry = self.lane[n];
        self.lane.copy_within(n + 1..self.len, n);
        self.len -= 1;
        entry
    }
}

impl core::ops::Deref for Index {
    type Target = [(Chunk, u32)];

    fn deref(&self) -> &[(Chunk, u32)] {
        &self.lane[..self.len]
    }
}

/// `.fixed.mounts`, lane by lane: a slot's holder and its next, a mount's
/// place, the mounts free, the index.
const MOUNT_LANES: [u32; 5] = [
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<u32>()) as u32,
    (OCCUPANCY * size_of::<(Chunk, u32)>()) as u32,
];
const MOUNT_ALIGN: u32 = 8;

/// What `.fixed.mounts` spans.
pub const MOUNTS_BYTES: u32 = span_of(&MOUNT_LANES, MOUNT_ALIGN);

/// Lane `k` of `.fixed.mounts`, its `OCCUPANCY` entries.
///
/// # Safety
/// Each lane is the table's alone, taken once as it is made; a zeroed entry
/// is a valid one for the plain numbers each holds.
unsafe fn mount_lane<T>(k: usize) -> &'static mut [T] {
    unsafe { lane(range(Range::Mounts).nth(&MOUNT_LANES, MOUNT_ALIGN, k), 0, OCCUPANCY) }
}

/// Who holds each mesh slot (mesh_slot.rs), by mount, none for a free one;
/// the slot each is chained to; the tier landing now; and the arena epoch
/// the tiers were written under.
pub struct Meshes {
    holder: &'static mut [Nth],
    /// The slot after each in the tier that fills it, by index: a tier past
    /// one slot's boxes goes on in any free slot of its pool, contiguous or
    /// not. Fixed at the slots' count, so it holds no more than they do.
    next: &'static mut [Nth],
    /// The tier the one bake open is landing: every slot it fills was held
    /// for it at once when its size was known (`reserve`), and its blocks
    /// are written as they come (`part`).
    landing: Option<Landing>,
    epoch: u32,
    /// Tiers landed across more than one slot, until a census takes them.
    pub chained: u32,
    /// Which telling of the slots tiers need this is: it moves whenever a
    /// chunk finds a tier fills more than was held for it, which the quality
    /// policy counts.
    needs: u32,
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
    level_boxes: [u32; LEVELS as usize],
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
    /// The sun, the moon, the light and the sky (layer/environment/state.rs).
    pub environment: EnvironmentDraw,
    /// The debug gizmos' segments (marker/debug_lines.rs).
    pub debug: DebugLines,
    slots: Vec<Slot>,
    /// Where each mount's slot is in `slots`, by mount. A mount is the
    /// number a chunk holds while it is mounted (`Slot::id`), and every part
    /// of it names it by that: its build, the mesh slots it holds, the tier
    /// it is landing. Nothing that names a mount outlives it (`remove`), so
    /// a number given back is given again at once, and nothing can take the
    /// chunk mounted under it for the one before.
    place: &'static mut [Nth],
    /// The mounts free, the last given back first.
    mounts: Free,
    /// The mounted chunks in chunk order, with their mounts: how a chunk is
    /// found.
    index: Index,
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
    pub census: Census,
    counts: ChunkCounts,
    /// The chunks the build task advances, in the one order, as the last
    /// tick left them.
    builds: Vec<u32>,
    landed: Vec<u32>,
    /// Chunks the build task committed since the last census.
    build_committed: u32,
    /// What the frontier's last step left: moves refused for room, and
    /// moves under way or held — each asks for the next turn.
    moves: (u32, u32),
    /// What a recut's pace is read by, for the profiler: refinements made
    /// and drawn pieces changed to another cube since load, the changes to
    /// what is drawn since the frontier's last step — moves of every kind and
    /// changes of cube — and the most of those between two steps.
    refined: u32,
    recubed: u32,
    changes: u32,
    changes_peak: u32,
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
    /// What the frontier reads of the table (`Now`), and its step.
    drawn: Vec<Chunk>,
    building: Vec<(Chunk, bool)>,
    recube: Vec<(Chunk, (Pool, u32))>,
    frontier: Frontier,
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
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let cube = cube_of(c, self.d.outermost(c), self.d.nearest_to(c), self.d.tau());
        (pool_of(c, cube), self.table.slots_of(c, cube))
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
        // What is sized by the mesh slots — who holds each and what each is
        // chained to, where each mount's slot is, the mounts free and the
        // chunk index — lies in `.fixed.mounts` (mem/fixed.rs), taken once
        // here, so a view moving and chunks coming and going allocate nothing
        // for it. The builder's state by mount is made once too, in the heap:
        // it holds what a build is in the middle of, not plain numbers.
        // SAFETY: the five lanes, each taken once here and the table's alone.
        let (holder, next, place, free, index) = unsafe { (mount_lane(0), mount_lane(1), mount_lane(2), mount_lane::<u32>(3), mount_lane(4)) };
        for (k, id) in free.iter_mut().enumerate() {
            *id = (OCCUPANCY - 1 - k) as u32;
        }
        Scheduler {
            level_boxes: [0; LEVELS as usize],
            meshes: Meshes {
                holder,
                next,
                landing: None,
                epoch: arena_epoch(),
                chained: 0,
                needs: 0,
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
            environment: EnvironmentDraw::default(),
            debug: DebugLines::default(),
            slots: Vec::new(),
            place,
            mounts: Free { lane: free, len: OCCUPANCY },
            index: Index { lane: index, len: 0 },
            domain_seen: u32::MAX,
            frame: None,
            view_dirty: true,
            census: Census::default(),
            counts: ChunkCounts::default(),
            scratch: Scratch::default(),
            builds: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            moves: (0, 0),
            refined: 0,
            recubed: 0,
            changes: 0,
            changes_peak: 0,
        }
    }

    /// How many slots chunk `c`'s tier of `cube` fills, as far as is known
    /// of its current sources: what the frontier and the quality policy
    /// count it by (frontier.rs `need`).
    pub fn slots_of(&self, c: Chunk, cube: Cube) -> u32 {
        self.find(&c).map_or(1, |i| self.slots[i].slots_for(cube, self.generation(i)))
    }

    /// Refinements and changes of cube since load, and the most changes to
    /// what is drawn between two of the frontier's steps.
    pub fn pace(&self) -> [u32; 3] {
        [self.refined, self.recubed, self.changes_peak]
    }

    /// Which telling of the slots tiers need this is (`slots_of`).
    pub fn needs_revision(&self) -> u32 {
        self.meshes.needs
    }

    // ---- commands ----

    /// Chunk `k`, mounted for a move of the frontier, its tier the one the
    /// rule wants for it now.
    fn add(&mut self, world: &mut World, k: Chunk) {
        // Every chunk mounted holds a slot of the arena or has one held for
        // it (`Slot::reserves`), and the frontier starts none it has no room
        // for: more than the arena's slots is a bug in that accounting.
        let id = self.mounts.pop().or_bug(Errno::ENOSPC);
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
            need: None,
            recube: false,
            tiers: Held::EMPTY,
            layout_primary: frontier::coarsest(level),
            layout_generation: 0,
            layout_ready: false,
            desired: frontier::coarsest(level),
            sources,
        };
        slot.unmeasured();
        if let Some(frame) = self.frame {
            slot.set(IN_FRUSTUM, measure(&frame, &slot).0);
        }
        if let Some(d) = domain::current() {
            slot.order = d.order_of(k);
        }
        let n = self.index.partition_point(|&(c, _)| c < k);
        self.index.insert(n, (k, id));
        self.place[id as usize] = Nth::of(self.slots.len());
        self.slots.push(slot);
        let i = self.slots.len() - 1;
        self.recheck(world, i, crate::host::now());
        self.view_dirty = true;
    }

    fn remove(&mut self, world: &mut World, k: Chunk) {
        let Ok(n) = self.index.binary_search_by_key(&k, |&(c, _)| c) else { return };
        let (_, id) = self.index.remove(n);
        let i = self.place[id as usize].get().or_bug(Errno::ENOTRECOVERABLE);
        self.view_dirty = true;
        self.counts.removed += 1;
        self.builder.remove(world, id);
        settle(&mut self.meshes, &self.builder);
        for t in self.slots[i].tiers.iter() {
            let_go(&mut self.meshes, t.slot.index, id);
        }
        self.slots.swap_remove(i);
        if i < self.slots.len() {
            self.place[self.slots[i].id as usize] = Nth::of(i);
        }
        self.place[id as usize] = Nth::NONE;
        // The mount is given again at once: nothing may still name it.
        debug_assert!(
            self.builder.generation_of(id).is_none() && self.meshes.landing.as_ref().is_none_or(|l| l.id != id) && self.meshes.holder.iter().all(|&h| h != Nth::of(id as usize)),
            "mount {id} outlived by what names it"
        );
        self.mounts.push(id);
    }

    /// Where chunk `k`'s slot is, if it is mounted.
    fn find(&self, k: &Chunk) -> Option<usize> {
        let n = self.index.binary_search_by_key(k, |&(c, _)| c).ok()?;
        self.place[self.index[n].1 as usize].get()
    }

    /// The builder's generation for slot `i`: the telling of its sources a
    /// tier must be built from to count.
    fn generation(&self, i: usize) -> u32 {
        self.builder.generation_of(self.slots[i].id).unwrap_or(0)
    }

    /// The chunk's sources changed. What it shows stays up: the tiers it
    /// holds are an older generation now, drawn until a layout of
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
    /// sources moved is told again. The terrain page is not a source
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
        }
    }

    /// The build task's step: the chunks the last tick ordered, holders of the
    /// scan and the bake slot first, advanced for what is left of the turn's
    /// quota and at least one step, and the chunks a tier landed on
    /// committed. Whether any did work; none did when all are built, blocked,
    /// or a block was refused.
    pub fn build_step(&mut self, world: &mut World, budget_ms: f64) -> bool {
        let (slots, place, meshes, landed) = (&mut self.slots, &*self.place, &mut self.meshes, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick(world, &self.builds, budget_ms, &crate::host::now, &mut |e| match e {
            BuildEvent::Reserve { .. } => reserve(slots, place, meshes, e),
            BuildEvent::Part { .. } => {
                part(slots, place, meshes, e);
                true
            }
            BuildEvent::Tier { id, .. } => {
                landed.push(id);
                land(slots, place, meshes, e);
                true
            }
        });
        settle(&mut self.meshes, &self.builder);
        let landed = core::mem::take(&mut self.landed);
        for &id in &landed {
            if let Some(i) = self.place[id as usize].get() {
                if self.commit(world, i) {
                    self.build_committed += 1;
                }
            }
        }
        self.landed = landed;
        worked
    }

    /// The counts since the last frame sample, taken.
    pub fn take_counts(&mut self) -> ChunkCounts {
        core::mem::take(&mut self.counts)
    }

    /// What the chunks hold, with the run sets held for them as the
    /// builder counts them (`Builder::run_sets`).
    pub fn held(&self) -> ChunkHeld {
        let (run_sets, run_bytes) = self.builder.run_sets();
        let mut out = ChunkHeld {
            run_sets,
            run_set_mb: round(run_bytes as f64 / MB) as u32,
            ..ChunkHeld::default()
        };
        for s in &self.slots {
            out.tiers += s.tiers.len() as u32;
        }
        out
    }

    /// Slots each pool has that no tier holds and no chunk mounted will
    /// take: what the frontier may start moves in.
    fn free(&self) -> Slots {
        let mut free = ARENA;
        for (at, h) in self.meshes.holder.iter().enumerate() {
            if !h.is_none() {
                let p = Pool::at(at) as usize;
                free[p] = free[p].saturating_sub(1);
            }
        }
        for (i, s) in self.slots.iter().enumerate() {
            if let Some((p, k)) = s.reserves(self.generation(i)) {
                free[p as usize] = free[p as usize].saturating_sub(k);
            }
        }
        free
    }

    /// The frontier's step (world/chunk/frontier.rs): the drawn pieces moved
    /// toward the view's pieces at the effective quality, as far as what is
    /// ready and the arena's room let them.
    fn materialize(&mut self, world: &mut World, sc: &mut Scratch) {
        let Some(d) = domain::current() else { return };
        sc.drawn.clear();
        sc.building.clear();
        sc.recube.clear();
        let mut recubing = 0;
        for (i, s) in self.slots.iter().enumerate() {
            match s.role {
                Role::Drawn => {
                    sc.drawn.push(s.chunk);
                    if s.recube {
                        recubing += 1;
                    } else if s.recubes() {
                        sc.recube.push((s.chunk, s.desired_slots(self.generation(i))));
                    }
                }
                Role::Building => sc.building.push((s.chunk, s.layout_ready)),
            }
        }
        sc.drawn.sort_unstable();
        sc.building.sort_unstable_by_key(|&(c, _)| c);
        let now = Now {
            ms: crate::host::now(),
            goal: d.pieces(),
            drawn: &sc.drawn,
            building: &sc.building,
            recubing,
            recube: &sc.recube,
            free: self.free(),
        };
        let plan = sc.frontier.plan(&now, &Pieces { d, table: self });
        self.moves = (plan.refused, plan.waiting + plan.held);
        self.refined = self.refined.wrapping_add(plan.refined);
        self.changes_peak = self.changes_peak.max(self.changes);
        self.changes = plan.swaps().count() as u32;

        for &k in plan.cancel.iter().chain(&plan.drop) {
            self.remove(world, k);
        }
        for (leave, enter) in plan.swaps() {
            for &k in leave {
                self.remove(world, k);
            }
            for &k in enter {
                if let Some(i) = self.find(&k) {
                    self.slots[i].role = Role::Drawn;
                }
            }
            self.view_dirty = true;
        }
        let now = crate::host::now();
        for &k in &plan.start {
            // What a move starts is neither drawn nor building: what was is
            // let go or cancelled above, or entered no move.
            xpute_core::ensure!(self.find(&k).is_none(), ENOTRECOVERABLE);
            self.add(world, k);
            self.counts.added += 1;
        }
        for &k in &plan.recube {
            if let Some(i) = self.find(&k) {
                self.slots[i].recube = true;
                self.recheck(world, i, now);
            }
        }
        // The cover's pool holds what it is sized to hold (mesh_slot.rs
        // `COVER_SLOTS`): every slot of it held is a current root's, and no
        // root holds two.
        let roots = d.roots();
        let mut owned = [false; COVER_SLOTS as usize];
        for at in Pool::Cover.range() {
            let Some(id) = self.meshes.holder[at].get() else { continue };
            let chunk = self.place[id].get().map(|i| self.slots[i].chunk);
            let root = chunk.and_then(|c| roots.binary_search(&c).ok());
            xpute_core::ensure!(root.is_some_and(|r| !core::mem::replace(&mut owned[r], true)), ENOTRECOVERABLE, at);
        }
    }

    /// The device's meshes are gone — the arena was made anew, the device with
    /// it — so every tier is forgotten and built again from the records.
    ///
    /// A layout is what a chunk shows, and it showed those tiers: kept ready
    /// over tiers that are gone, the recheck finds nothing to ask for and the
    /// chunk draws nothing until its cube happens to change.
    /// With its layout gone it is a hole, which the recheck asks for first.
    fn lose_meshes(&mut self) {
        let epoch = arena_epoch();
        if epoch == self.meshes.epoch {
            return;
        }
        self.meshes.epoch = epoch;
        self.meshes.holder.fill(Nth::NONE);
        for s in self.slots.iter_mut() {
            s.tiers.clear();
            s.layout_ready = false;
        }
        self.view_dirty = true;
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, move the frontier, commit
    /// under the frame's budget, and set what the build task takes.
    /// Whether the canvas has something new to draw.
    pub fn tick(&mut self, world: &mut World, frame: Frame, now: &impl Fn() -> f64) -> bool {
        let t0 = now();
        let deadline = t0 + frame.remaining_ms;
        let hard = deadline + frame.hard_slack_ms;
        if !self.frame.is_some_and(|prev| same_view(&prev, &frame)) {
            self.view_dirty = true;
        }
        self.frame = Some(frame);
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

        let mut sc = core::mem::take(&mut self.scratch);
        self.lose_meshes();
        self.refresh(&frame);

        // What each chunk should be showing, for every chunk that could have
        // changed: freshly mounted or settling — and all of them
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
        // them at the worst, and only when an input moved. A recheck reads
        // and writes its own chunk alone, so the order it runs in is no part
        // of what it decides, and the chunks go in the table's order.
        let full_recheck = self.view_dirty;
        sc.eligible.clear();
        sc.eligible.extend((0..self.slots.len()).filter(|&i| full_recheck || self.slots[i].flags & (FRESH | SETTLING) != 0));
        for k in 0..sc.eligible.len() {
            let i = sc.eligible[k];
            self.recheck(world, i, t0);
        }
        if full_recheck {
            self.view_dirty = false;
        }

        self.materialize(world, &mut sc);
        // Mounting and letting go move chunks in the table.
        self.order(&mut sc.order, &mut sc.keys);

        // The pending chunks, in the one order, on screen or not: a piece
        // off the screen is the frontier's as much as one on it, and is what
        // a turn finds drawn.
        sc.pending.clear();
        sc.pending.extend(sc.order.iter().copied().filter(|&i| self.slots[i].has(PENDING)));
        let pending_total = sc.pending.len() as u32;

        let commits = under_budget(&sc.pending, param::COMMITS_PER_FRAME, deadline, hard, now, |i| self.commit(world, i));
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

        self.scratch = sc;
        let animating = self.models.tick(&mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
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
            if s.has(PENDING) {
                c.pending += 1;
            }
            if s.role == Role::Building && s.layout_ready {
                c.ready_waiting += 1;
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
        c.chained = core::mem::take(&mut self.meshes.chained);
        c.fine_peak = core::mem::take(&mut self.meshes.fine_peak);
        c.coarse_peak = core::mem::take(&mut self.meshes.coarse_peak);
        let moving = c.waiting > 0 || c.ready_waiting > 0;
        self.census = c;
        committed > 0 || pending_total > committed || moving || self.view_dirty || animating
    }

    /// This frame's draws: every piece drawn that is on screen with a
    /// layout, its tier placed where the chunk is in the frame the camera is
    /// written in. A chunk built for a move is not drawn until the move is
    /// made: the piece it replaces is what covers its ground until then.
    fn draw_frame(&mut self, frame: &Frame) {
        self.draw.begin_frame();
        self.level_boxes = [0; LEVELS as usize];
        self.ghost.draw(&mut self.draw, frame.eye);
        self.models.draw(&mut self.draw, frame.eye);
        self.markers.draw(&mut self.draw, frame.eye);
        self.debug.draw(&mut self.draw, frame.eye);
        let holder = &self.meshes.holder;
        for s in &self.slots {
            if s.role == Role::Building || !s.has(IN_FRUSTUM) || !s.layout_ready {
                continue;
            }
            let (x0, y0) = s.base();
            let translation = DrawList::translation([x0, y0, 0.0], frame.eye);
            let cube = s.layout_primary;
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
            xpute_core::ensure!(holder[t.slot.index] == Nth::of(s.id as usize), ENOTRECOVERABLE);
            if t.box_cnt == 0 {
                continue;
            }
            // A mesh's heights are steps from its own floor, so the
            // floor rides in the translation with the chunk's corner.
            let at = [translation[0], translation[1], translation[2] + t.heights.floor as f32];
            let (unit, z_step) = ((mesh_step(cube, s.level()) << s.level()) as f32, t.heights.step as f32);
            // A slot's worth a draw, down the tier's chain.
            let (mut left, mut slot) = (t.box_cnt, Some(t.slot.index));
            while let (Some(at_slot), true) = (slot, left > 0) {
                let m = slot_at(at_slot);
                let n = left.min(m.capacity);
                self.draw.draw_arena(m.group, m.first_box, n, at, unit, z_step, group_is_fine(m.group));
                left -= n;
                slot = self.meshes.next[at_slot].get();
            }
            self.level_boxes[s.level() as usize] += t.box_cnt;
        }
    }

    /// Where every chunk is and whether it is on screen, measured against the
    /// frame.
    fn refresh(&mut self, frame: &Frame) {
        let domain = domain::current();
        for i in 0..self.slots.len() {
            let visible = measure(frame, &self.slots[i]).0;
            let order = domain.map_or(Order::LAST, |d| d.order_of(self.slots[i].chunk));
            let s = &mut self.slots[i];
            s.order = order;
            let was = s.has(IN_FRUSTUM);
            if visible != was {
                s.set(IN_FRUSTUM, visible);
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
        settle(&mut self.meshes, &self.builder);
    }

    /// The tier this chunk should be showing, and whether that makes it
    /// pending: the cube the rule wants at its nearest distance at the
    /// effective quality (frontier.rs `wanted_cube`), whatever the slots its
    /// mesh fills. Pending while what it shows is not what it builds
    /// (`Slot::target`), of its sources as they are now: a drawn piece wanting another cube is the frontier's to let
    /// (`materialize`), and is not pending until it is. A chunk built for a
    /// move drops a tier the rule no longer wants rather than hold two, as
    /// nothing of it is drawn yet. SETTLING is pending with a layout already
    /// up, so a frame can tell a chunk that is changing tier from one that
    /// has nothing to show yet.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64) {
        self.slots[i].set(FRESH, false);
        let Some(d) = domain::current() else { return };
        let s = &self.slots[i];
        let (level, chunk) = (s.level(), s.chunk);
        let objects = level == 0 && world.store.entries(L2_IDX(chunk.corner())).is_some();
        let current = self.generation(i);
        let desired = drawn_cube(wanted_cube(d.nearest_to(chunk), d.tau()), level, d.outermost(chunk), objects);
        if self.slots[i].role == Role::Building && self.slots[i].layout_ready && self.slots[i].layout_primary != desired {
            self.release_tiers(i, |_, _| false);
        }
        let s = &mut self.slots[i];
        s.desired = desired;
        if !s.recubes() {
            s.recube = false;
        }
        let pending = !s.layout_ready || s.layout_primary != s.target() || s.layout_generation != current;
        self.set_pending(i, pending, now);
        let s = &mut self.slots[i];
        let settling = s.has(PENDING) && s.layout_ready;
        s.set(SETTLING, settling);
        if !s.has(PENDING) {
            self.stop_work(world, i);
        }
    }

    /// Lets go of chunk `i`'s tiers but those `keep` names, and of
    /// the slots only they held; its layout too where its primary goes.
    fn release_tiers(&mut self, i: usize, keep: impl Fn(Cube, u32) -> bool) {
        let s = &mut self.slots[i];
        prune(s, &mut self.meshes, |t| keep(t.cube, t.generation), None);
        if s.tier(s.layout_primary, s.layout_generation).is_none() {
            s.layout_ready = false;
        }
    }

    /// Whether the chunk holds the tier `primary` of the current generation.
    /// When not, the builder is asked for it.
    ///
    /// A tier known to fill more slots than its pool has free is not asked
    /// for until it does: its bake would only be thrown away again
    /// (`reserve`). It is looked at again while it waits.
    fn ask(&mut self, world: &mut World, i: usize, primary: Cube) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let lacks = s.tier(primary, generation).is_none();
        let room = !lacks || s.slots_for(primary, generation) <= claimable(&self.meshes, Pool::of(s.level(), mesh_step(primary, s.level())));
        self.builder.want(world, s.id, [(lacks && room).then_some(primary), None]);
        settle(&mut self.meshes, &self.builder);
        if !room {
            self.slots[i].set(FRESH, true);
        }
        !lacks
    }

    /// Shows `primary` of `generation`. What it no longer draws —
    /// an older generation's tiers among it, so what is drawn
    /// changes whole — is `trim`'s to let go, after.
    fn apply_layout(&mut self, i: usize, primary: Cube, generation: u32) {
        self.counts.layouts += 1;
        let s = &mut self.slots[i];
        s.layout_primary = primary;
        s.layout_generation = generation;
        s.layout_ready = true;
        match s.tier(primary, generation).and_then(|t| t.bounds) {
            Some(b) => {
                let d = [(b.max[0] - b.min[0]) as f64, (b.max[1] - b.min[1]) as f64, (b.max[2] - b.min[2]) as f64];
                s.bc = [(b.min[0] + b.max[0]) as f64 * 0.5, (b.min[1] + b.max[1]) as f64 * 0.5, (b.min[2] + b.max[2]) as f64 * 0.5];
                s.radius = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() * 0.5;
            }
            None => s.unmeasured(),
        }
    }

    /// Lets go of every tier the layout does not draw: a slot
    /// is held only by what is on the screen or about to be.
    fn trim(&mut self, i: usize) {
        let s = &self.slots[i];
        let (p, g) = (s.layout_primary, s.layout_generation);
        self.release_tiers(i, move |cube, generation| generation == g && cube == p);
    }

    /// Applies what `recheck` asked for, if it can: the desired tier once it
    /// is built. A drawn piece changes cube only once the frontier has let it
    /// hold a slot for it (`Slot::recube`).
    fn commit(&mut self, world: &mut World, i: usize) -> bool {
        let s = &self.slots[i];
        if !s.has(PENDING) {
            return false;
        }
        let (p, generation) = (s.target(), self.generation(i));
        let asked = self.ask(world, i, p);
        if !asked {
            return false;
        }
        let s = &self.slots[i];
        let recubed = s.role == Role::Drawn && s.layout_ready && s.layout_generation == generation && s.layout_primary != p;
        self.apply_layout(i, p, generation);
        self.trim(i);
        if recubed {
            self.recubed = self.recubed.wrapping_add(1);
            self.changes += 1;
        }
        let s = &mut self.slots[i];
        s.recube = s.recubes() && s.recube;
        self.set_pending(i, false, 0.0);
        true
    }
}

/// Whether two frames look at the world the same way: what a recheck's
/// answer depends on, not the budget or the elapsed time.
fn same_view(a: &Frame, b: &Frame) -> bool {
    a.anchor == b.anchor && a.origin == b.origin && a.planes == b.planes && a.camera == b.camera
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
    let (x0, y0) = s.base();
    let [x, y, z] = frame.world_of([x0 + s.bc[0], y0 + s.bc[1], s.bc[2]]);
    // Where the renderer draws it: laid back on the planet about the
    // anchor, the origin here. A far chunk culled where the flat world has
    // it would leave a hole at the horizon.
    let [x, y, z] = match frame.globe {
        Some(globe) => globe.place([x, y, z]),
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

/// Whether slot `at` is free: no tier holds it. Never one a drawn tier
/// holds — a slot a piece on the screen draws from is that ground's only
/// cover.
fn free_at(meshes: &Meshes, at: usize) -> bool {
    meshes.holder[at].is_none()
}

/// How many slots of `pool` are free.
fn claimable(meshes: &Meshes, pool: Pool) -> u32 {
    pool.range().filter(|&at| free_at(meshes, at)).count() as u32
}

/// `k` free slots of `pool`, all of them or none: held for mount `holder`
/// and chained in the order they are written, the first
/// returned. Taking some and waiting for the rest would let two tiers each
/// hold part of what the other needs.
fn claim(meshes: &mut Meshes, pool: Pool, k: u32, holder: u32) -> Option<usize> {
    if (pool.range().filter(|&at| free_at(meshes, at)).take(k as usize).count() as u32) < k {
        return None;
    }
    let (mut first, mut last): (Option<usize>, Option<usize>) = (None, None);
    let mut left = k;
    for at in pool.range() {
        if left == 0 {
            break;
        }
        if !free_at(meshes, at) {
            continue;
        }
        meshes.holder[at] = Nth::of(holder as usize);
        meshes.next[at] = Nth::NONE;
        match last {
            Some(p) => meshes.next[p] = Nth::of(at),
            None => first = Some(at),
        }
        last = Some(at);
        left -= 1;
    }
    first
}

/// Lets go of the chain from slot `at`, where mount `id` holds it.
fn let_go(meshes: &mut Meshes, at: usize, id: u32) {
    let mut slot = Some(at);
    while let Some(at) = slot {
        slot = meshes.next[at].take();
        if meshes.holder[at] == Nth::of(id as usize) {
            meshes.holder[at] = Nth::NONE;
        }
    }
}

/// Lets go of the tiers of `s` that `keep` does not name, and of each one's
/// slots unless a tier kept, or a tier landing at `landing`, draws from
/// them.
fn prune(s: &mut Slot, meshes: &mut Meshes, keep: impl Fn(&Tier) -> bool, landing: Option<usize>) {
    for t in s.tiers.iter().filter(|t| !keep(t)) {
        if landing != Some(t.slot.index) && !s.tiers.iter().any(|k| keep(k) && k.slot.index == t.slot.index) {
            let_go(meshes, t.slot.index, s.id);
        }
    }
    s.tiers.retain(keep);
}

/// The most a slot holds of its tiers: the one its layout draws, and one
/// more — the tier it has just built (`land`), which the commit applying it
/// then lets the other go for.
const HELD: usize = 2;

/// A slot's few of something, in the order they came. One more than `HELD`
/// is a slot holding what nothing draws or builds, which is a bug.
#[derive(Clone, Copy)]
struct Held<T: Copy> {
    items: [Option<T>; HELD],
    len: usize,
}

impl<T: Copy> Held<T> {
    const EMPTY: Held<T> = Held { items: [None; HELD], len: 0 };

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.items[..self.len].iter().flatten()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items[..self.len].iter_mut().flatten()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn push(&mut self, t: T) {
        xpute_core::ensure!(self.len < HELD, ENOTRECOVERABLE, self.len);
        self.items[self.len] = Some(t);
        self.len += 1;
    }

    fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let mut n = 0;
        for k in 0..self.len {
            if let Some(t) = self.items[k].take().filter(|t| keep(t)) {
                self.items[n] = Some(t);
                n += 1;
            }
        }
        self.len = n;
    }

    fn clear(&mut self) {
        self.retain(|_| false);
    }
}

/// The slot `n` down the chain from `at`.
fn chained(meshes: &Meshes, at: usize, n: u32) -> Option<usize> {
    (0..n).try_fold(at, |at, _| meshes.next[at].get())
}

/// A tier the open bake is landing: whose it is, the first of the slots it
/// fills, how many, and what is known of it as its blocks come.
struct Landing {
    id: u32,
    generation: u32,
    cube: Cube,
    first: usize,
    slots: u32,
    /// The slots of the tier it is rebuilt over, where it is, which it
    /// writes over rather than holding more.
    over: u32,
    bounds: Option<Bounds>,
}

/// Holds every slot a tier of `boxes` boxes fills, at once, or says there is
/// not the room (build.rs `BuildEvent::Reserve`). A tier rebuilt over one of
/// the same cube writes over its slots and takes only what it needs past
/// them. Where the room is not there, how many it needs is kept: the
/// frontier and the quality policy count it so from then on, and it is not
/// asked for again until the slots are free (`Scheduler::ask`).
fn reserve(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) -> bool {
    let BuildEvent::Reserve { id, generation, cube, boxes } = e else { return false };
    // One bake is open at a time: a landing still here is of one that closed
    // within this tick without landing, and is let go as `settle` would.
    if let Some(l) = meshes.landing.take() {
        release(meshes, l);
    }
    let Some(i) = place[id as usize].get() else { return false };
    let s = &slots[i];
    if cube != s.target() {
        return false;
    }
    let level = s.level();
    let pool = Pool::of(level, mesh_step(cube, level));
    let k = boxes.div_ceil(capacity(level, mesh_step(cube, level))).max(1);
    let over = s.tiers.iter().find(|t| t.cube == cube).map(|t| t.slot.index);
    let taken = match over {
        Some(first) => Some((first, (0..).take_while(|&n| chained(meshes, first, n).is_some()).count() as u32)),
        None => claim(meshes, pool, k, id).map(|first| (first, 0)),
    };
    let Some((first, held)) = taken else {
        return refuse(slots, i, meshes, generation, cube, k, level);
    };
    if held > 0 && k > held {
        let last = chained(meshes, first, held - 1).unwrap_or(first);
        let more = claim(meshes, pool, k - held, id);
        match more {
            Some(more) => meshes.next[last] = Nth::of(more),
            None => return refuse(slots, i, meshes, generation, cube, k, level),
        }
    }
    meshes.landing = Some(Landing {
        id,
        generation,
        cube,
        first,
        slots: k,
        over: held,
        bounds: None,
    });
    true
}

/// Lets go of what the landing tier holds as soon as the bake it is from is
/// no longer open — thrown away, its chunk let go or asked for another cube —
/// so the slots a bake held are the bake's until it lands and free the
/// moment it does not: the slots it took, and past a tier it was rebuilt
/// over only those taken past that tier's own. Asked after every call that
/// can close a bake.
fn settle(meshes: &mut Meshes, builder: &Builder) {
    let open = builder.baking();
    if let Some(l) = meshes.landing.take_if(|l| open != Some((l.id, l.cube, l.generation))) {
        release(meshes, l);
    }
}

/// Lets go of what a tier that did not land held: the slots it took, and
/// past a tier it was rebuilt over only those taken past that tier's own.
fn release(meshes: &mut Meshes, l: Landing) {
    if l.over == 0 {
        let_go(meshes, l.first, l.id);
    } else if let Some(last) = chained(meshes, l.first, l.over - 1) {
        if let Some(more) = meshes.next[last].take() {
            let_go(meshes, more, l.id);
        }
    }
}

/// A tier found to fill `k` slots where they were not free: kept, and
/// counted from then on.
fn refuse(slots: &mut [Slot], i: usize, meshes: &mut Meshes, generation: u32, cube: Cube, k: u32, level: u32) -> bool {
    slots[i].need = Some((generation, cube, k));
    slots[i].set(FRESH, true);
    meshes.needs = meshes.needs.wrapping_add(1);
    meshes.waits += 1;
    meshes.wait_levels |= 1 << level;
    false
}

/// Writes a block of the landing tier's boxes to the slot it fills
/// (build.rs `BuildEvent::Part`), and gathers what the tier's bounds and
/// surface are.
fn part(slots: &[Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Part {
        id,
        generation,
        cube,
        at,
        first,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(l) = meshes.landing.as_ref().filter(|l| (l.id, l.generation, l.cube) == (id, generation, cube)) else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let level = slots[i].level();
    let step = mesh_step(cube, level);
    let head = slot_at(l.first);
    let n = first / head.capacity;
    let Some(slot) = chained(meshes, l.first, n).map(slot_at) else { return };
    xpute_core::ensure!(first % head.capacity == 0 && box_cnt <= slot.capacity, ENOTRECOVERABLE, first, box_cnt);
    let box_words = group_box_words(slot.group);
    let asked = if slot.capacity == FINE_BOXES { &mut meshes.fine_peak } else { &mut meshes.coarse_peak };
    *asked = (*asked).max(first + box_cnt);
    // SAFETY: the block's boxes, a least corner and a size in the first six
    // floats of each, in `.fixed.bake` where the bake just wrote them. The
    // format is the group's, not the mesh's: a slot is a stride into its
    // group's arena (mesh_slot.rs).
    let words = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(at as usize), (box_cnt * box_words) as usize) };
    let unit = (step << level) as f64;
    let mut bounds = meshes.landing.as_ref().and_then(|l| l.bounds);
    for r in words.chunks_exact(box_words as usize) {
        let (min, size) = box_bounds(r, unit, heights);
        let b = bounds.get_or_insert(Bounds {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        });
        for a in 0..3 {
            let (lo, size) = (min[a] as f32, size[a] as f32);
            b.min[a] = b.min[a].min(lo);
            b.max[a] = b.max[a].max(lo + size);
        }
    }
    if box_cnt > 0 {
        let before = arena_epoch();
        // SAFETY: the same words, as bytes.
        let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), words.len() * 4) };
        // No device to write to: the tier is asked for again once there is
        // one (`land` finds its landing gone).
        if !arena_write(slot.group as usize, slot.first_box * box_words * 4, bytes) {
            meshes.landing = None;
            return;
        }
        // The write that made the arena lost nothing written before it.
        if arena_epoch() != before && before == meshes.epoch {
            meshes.epoch = arena_epoch();
        }
    }
    if let Some(l) = meshes.landing.as_mut() {
        l.bounds = bounds;
    }
}

/// Lands the tier every block of which is written (build.rs
/// `BuildEvent::Tier`): recorded on its chunk, over the slots held for it,
/// and what the tier it was rebuilt over held past what it fills let go.
fn land(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Tier {
        id,
        generation,
        cube,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let s = &slots[i];
    // Only what the chunk builds now lands: a drawn piece a new cube only
    // once a slot is held for it.
    if s.tier(cube, generation).is_some() || cube != s.target() {
        return;
    }
    let level = s.level();
    let landing = match meshes.landing.take() {
        Some(l) if (l.id, l.generation, l.cube) == (id, generation, cube) => l,
        other => {
            meshes.landing = other;
            // A mesh of nothing asked for no slot: it takes one, over the
            // tier of its cube where there is one.
            if box_cnt > 0 {
                return;
            }
            let over = s.tiers.iter().find(|t| t.cube == cube).map(|t| t.slot.index);
            let taken = match over {
                Some(first) => Some(first),
                None => claim(meshes, Pool::of(level, mesh_step(cube, level)), 1, id),
            };
            let Some(first) = taken else {
                refuse(slots, i, meshes, generation, cube, 1, level);
                return;
            };
            Landing {
                id,
                generation,
                cube,
                first,
                slots: 1,
                over: 0,
                bounds: None,
            }
        }
    };
    // What the tier it is rebuilt over held past what this one fills.
    if landing.over > landing.slots {
        if let Some(last) = chained(meshes, landing.first, landing.slots - 1) {
            if let Some(tail) = meshes.next[last].take() {
                let_go(meshes, tail, id);
            }
        }
    }
    if landing.slots > 1 {
        meshes.chained += 1;
    }
    let slot = slot_at(landing.first);
    let s = &mut slots[i];
    // A slot holds what its layout draws and what it has just built, and
    // nothing else (`HELD`): a tier that is neither — this cube's of an older
    // generation, built over, or a cube it no longer builds — goes now, and
    // its slots with it unless what stays draws from them.
    let (p, g) = (s.layout_primary, s.layout_generation);
    prune(s, meshes, |t| t.generation == g && t.cube == p, Some(slot.index));
    // A tier of an older generation over the same slots shows these boxes
    // now: they were written over its own.
    for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
        t.box_cnt = box_cnt;
        t.bounds = landing.bounds;
        t.heights = heights;
    }
    s.tiers.push(Tier {
        cube,
        generation,
        bounds: landing.bounds,
        slot,
        box_cnt,
        heights,
    });
}
