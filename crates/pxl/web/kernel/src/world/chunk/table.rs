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


use std::collections::HashMap;

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch, arena_write};
use pxl_render::look::globe::Globe;
use pxl_render::mesh::chunk_mesher::{box_bounds, Heights};
use pxl_render::mesh::paint::terrain::adapter::beside_index;
use pxl_render::mesh::voxel_scan::APRON;

use crate::sched::profiler::record::ZOOMS;
use pxl_spec::space::adapter::epsg3857::zoom_of;
use pxl_spec::space::observation::Bounds as _;
use pxl_spec::space::pyramid::{chunk_scale, GROUND};
use pxl_spec::space::observation::Frustum;
use pxl_spec::space::v3::{dot3, len3, Aabb, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::alloc;
use xpute_runtime::mem::section::span_of;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use xpute_runtime::global::Global;
use crate::layer::billboard::field::Billboards;
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::debug_lines::DebugLines;
use crate::layer::marker::hover_cursor::GhostDraw;
use crate::layer::marker::hover_cursor::{render_edits, RenderEdits};
use crate::layer::marker::location_puck::Markers;
use crate::layer::model::store::Models;
use crate::layer::object::store::EntryAt;
use crate::mem::fixed::{lane, range, Range};
use crate::param;
use crate::sched::profiler::record::MB;
use crate::shell::setting::store::settings_store;
use crate::world::chunk::build::{mesh_step, BuildEvent, Builder, World};
use pxl_spec::space::hierarchy::Scale;
use crate::world::chunk::frontier::{scale_of, pool_of, Env, Frontier, Now, Slots};
use crate::world::chunk::mesh_slot::{capacity, group_box_words, group_bytes, group_is_fine, slot_at, MeshSlot, Pool, ARENA, COVER_SLOTS, FINE_BOXES, OCCUPANCY};
use crate::world::chunk::source::{draws_band, objects_revision_of, tiers_draw};
use crate::world::paging::LEVELS;
use pxl_spec::space::hierarchy::Scale;
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;

use crate::world::observation::{Known, Order};
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::indexer::L2_IDX;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Drawn,
    /// Not drawn until every chunk of its move is ready.
    Building,
}

#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The globe's anchor in the root frame; chunks are measured about it.
    pub anchor: V3,
    pub origin: V3,
    /// Draw translations are taken relative to the eye.
    pub eye: V3,
    /// About the anchor.
    pub frustum: Frustum,
    pub camera: V3,
    pub delta_s: f64,
    /// What is left of the turn's quota, not the quota itself.
    pub remaining_ms: f64,
    pub hard_slack_ms: f64,
    pub center_z: f64,
    pub globe: Option<Globe>,
}

impl Frame {
    pub fn world_of(&self, at: V3) -> V3 {
        [0, 1, 2].map(|k| at[k] - self.origin[k] - self.anchor[k])
    }
}

#[derive(Clone, Copy)]
struct Tier {
    scale: Scale,
    generation: u32,
    bounds: Option<Aabb>,
    /// First slot of the chain the tier's boxes fill (`Meshes::next`).
    slot: MeshSlot,
    box_cnt: u32,
    heights: Heights,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkCounts {
    pub added: u32,
    pub removed: u32,
    pub layouts: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkHeld {
    /// Chunks holding a run set, and what those sets cost in the heap.
    pub run_sets: u32,
    pub run_set_mb: u32,
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
    chunk: Chunk,
    role: Role,
    /// Bounding sphere in the chunk's own space.
    bc: V3,
    radius: f64,
    order: Order,
    in_frustum: bool,
    /// Mounted and not yet rechecked; its first recheck ignores the frame budget.
    fresh: bool,
    /// Since when it has wanted a build; none while it wants none.
    pending_since: Option<f64>,
    /// Slots a tier needs, for (generation, scale), found when a bake did not
    /// fit (`BuildEvent::Reserve`); not asked for again until that many are free.
    need: Option<(u32, Scale, u32)>,
    /// A drawn piece the frontier let build another scale; a slot is held for it.
    retier: bool,
    /// The layout's tiers plus the last one landed. A source change keeps the
    /// old generation's tiers drawn until a layout of the new one is applied.
    tiers: Held<Tier>,
    /// The applied layout's primary scale and generation; none until one is, or
    /// once its tier is gone.
    layout: Option<(Scale, u32)>,
    desired: Scale,
    sources: Sources,
    decor_sources: DecorSources,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    structures: bool,
    /// A tier above a leaf bakes one color with no texture layer, so the setting
    /// is part of what the tier was built from.
    textured: bool,
    objects_revision: u32,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
    /// Level of the page the chunk and each apron neighbour are read from, so a
    /// finer page arriving rebuilds the chunk and its shared edges.
    read_from: Option<u32>,
    beside_from: [Option<u32>; 8],
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
            read_from: Terrain::pyramid().paging().drawn_from(k.page()).map(|p| p.level),
            beside_from: {
                let mut from = [None; 8];
                for ([qx, qy], p) in k.pages_beside(APRON) {
                    from[beside_index(qx, qy)] = Terrain::pyramid().paging().drawn_from(p).map(|p| p.level);
                }
                from
            },
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
        let c = b.center();
        self.bc = [c[0] - region.min[0], c[1] - region.min[1], c[2]];
        self.radius = b.radius();
    }

    fn slots_for(&self, scale: Scale, generation: u32) -> u32 {
        self.need.filter(|&(g, c, _)| g == generation && c == scale).map_or(1, |(_, _, k)| k)
    }

    fn pending(&self) -> bool {
        self.pending_since.is_some()
    }

    /// Pending with a layout up.
    fn settling(&self) -> bool {
        self.pending() && self.layout.is_some()
    }

    fn tier(&self, scale: Scale, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.scale == scale && t.generation == generation)
    }

    fn retiers(&self) -> bool {
        self.layout.is_some_and(|(p, _)| p != self.desired)
    }

    /// The scale it builds now: the desired one, except a drawn piece the frontier
    /// has not let retier rebuilds its shown scale in place, so an edit still shows.
    fn target(&self) -> Scale {
        match self.layout {
            Some((p, _)) if self.role == Role::Drawn && self.retiers() && !self.retier => p,
            _ => self.desired,
        }
    }

    /// Slots held for it and not yet taken: none while it has a tier of the target scale.
    fn reserves(&self, generation: u32) -> Option<(Pool, u32)> {
        let scale = self.target();
        (!self.tiers.iter().any(|t| t.scale == scale)).then(|| (pool_of(self.chunk, scale), self.slots_for(scale, generation)))
    }
}

#[derive(Clone, Copy, Default)]
pub struct Census {
    pub chunks: u32,
    pub in_frustum: u32,
    pub settling: u32,
    /// Pending after the frame, counted directly: "start minus committed" would
    /// miss chunks whose build landed this frame.
    pub pending: u32,
    pub offscreen_pending: u32,
    pub oldest_visible_ms: f64,
    pub oldest_offscreen_ms: f64,
    pub ready_waiting: u32,
    pub slot_waits: u32,
    pub wait_levels: u32,
    pub chained: u32,
    pub fine_peak: u32,
    pub coarse_peak: u32,
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
    pub chained: u32,
    /// Bumped whenever a tier needs more slots than were held; the quality policy watches it.
    needs: u32,
    fine_peak: u32,
    coarse_peak: u32,
    waits: u32,
    wait_levels: u32,
}

pub struct Scheduler {
    pub meshes: Meshes,
    /// The boxes the last draw listed by level (`Census::level_boxes`).
    level_boxes: [u32; LEVELS as usize],
    pub builder: Builder,
    pub draw: DrawList,
    pub ghost: GhostDraw,
    pub models: Models,
    pub markers: Markers,
    pub billboards: Billboards,
    /// The sun, the moon, the light and the sky (layer/environment/state.rs).
    pub environment: EnvironmentDraw,
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
    /// found, and a column of them (`surface_at`).
    index: Index,
    /// The domain's pieces the last recheck saw (world/pyramid/domain.rs
    /// `revision`): what a chunk should show is read off it, and it is made
    /// again when the eye, the resident pages, the heights or the quality
    /// move.
    domain_seen: u32,
    frame: Option<Frame>,
    /// An input changed that no recheck has seen; the next tick rechecks every
    /// chunk. Frames are requested until then, or the world stops at the last frame.
    view_dirty: bool,
    retained_on: f64,
    retained_off: f64,
    pub census: Census,
    counts: ChunkCounts,
    /// The chunks the build task advances, in the one order, as the last
    /// tick left them.
    builds: Vec<u32>,
    landed: Vec<u32>,
    build_committed: u32,
    moves: (u32, u32),
    /// Recut pace for the profiler.
    refined: u32,
    retiered: u32,
    changes: u32,
    changes_peak: u32,
    scratch: Scratch,
}

/// Empty by default: the tick `mem::take`s it every frame, so `Default` must not allocate.
#[derive(Default)]
struct Scratch {
    order: Vec<usize>,
    keys: Vec<Priority>,
    pending: Vec<usize>,
    eligible: Vec<usize>,
    drawn: Vec<Chunk>,
    building: Vec<(Chunk, bool)>,
    retier: Vec<(Chunk, (Pool, u32))>,
    frontier: Frontier,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new()
    }
}

static BUILDS: Global<Notify> = Global::new();

pub fn builds() -> &'static mut Notify {
    BUILDS.get(Notify::new)
}

/// Runs `step` over `items`: the first always, then until `cap` did work or
/// the budget is spent, never past `hard`. Returns (ran, visited).
fn under_budget<T: Copy>(items: &[T], cap: u32, deadline: f64, hard: f64, now: &impl Fn() -> f64, step: impl FnMut(T) -> bool) -> PassStats {
    let at = now();
    let opts = BudgetOpts {
        budget_ms: deadline - at,
        hard_budget_ms: Some(hard - at),
        max_steps: Some(cap),
    };
    run_under_budget(items.iter().copied(), step, opts)
}

struct Pieces<'a> {
    d: &'a Domain,
    table: &'a Scheduler,
}

impl Env for Pieces<'_> {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let scale = scale_of(c, self.d.outermost(c), self.d.order_of(c).near, self.d.tau(), false);
        (pool_of(c, scale), self.table.slots_of(c, scale))
    }

    fn buildable(&self, c: Chunk) -> bool {
        Terrain::pyramid().paging().drawn_from(c.page()).is_some()
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
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            counts: ChunkCounts::default(),
            scratch: Scratch::default(),
            builds: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            moves: (0, 0),
            refined: 0,
            retiered: 0,
            changes: 0,
            changes_peak: 0,
        }
    }

    pub fn slots_of(&self, c: Chunk, scale: Scale) -> u32 {
        self.find(&c).map_or(1, |i| self.slots[i].slots_for(scale, self.generation(i)))
    }

    /// Refinements and changes of cube since load, and the most changes to
    /// what is drawn between two of the frontier's steps.
    pub fn pace(&self) -> [u32; 3] {
        [self.refined, self.retiered, self.changes_peak]
    }

    /// Each mounted chunk, whether it is drawn, and whether it is in the frustum.
    pub fn mounted(&self, out: &mut Vec<(Chunk, bool, bool)>) {
        out.clear();
        out.extend(self.slots.iter().map(|s| (s.chunk, s.role == Role::Drawn, s.in_frustum)));
    }

    /// The finest drawn chunk holding `p`, as zooms: its cells', its tier's,
    /// and those of the page it is read from.
    pub fn drawn_at(&self, p: &Pvf3) -> Option<(i32, Option<i32>, Option<i32>)> {
        let s = (0..LEVELS).find_map(|level| self.slots.iter().find(|s| s.role == Role::Drawn && s.chunk == Chunk::of(level, p)))?;
        Some((
            zoom_of(GROUND.scale(s.level())),
            s.layout.map(|(scale, _)| zoom_of(scale)),
            s.sources.read_from.map(|l| zoom_of(GROUND.scale(l))),
        ))
    }

    pub fn drawn_revision(&self) -> u32 {
        self.drawn_revision
    }

    /// Which telling of the slots tiers need this is (`slots_of`).
    pub fn needs_revision(&self) -> u32 {
        self.meshes.needs
    }

    fn add(&mut self, world: &mut World, k: Chunk) {
        // The frontier starts no move without room, so running out of mounts is a bug.
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
            in_frustum: false,
            fresh: true,
            pending_since: None,
            need: None,
            retier: false,
            tiers: Held::EMPTY,
            layout: None,
            desired: chunk_scale(level),
            sources,
            decor_sources: DecorSources::of(k, edits),
        };
        slot.unmeasured();
        if let Some(frame) = self.frame {
            slot.in_frustum = visible(&frame, &slot);
        }
        if let Some(d) = domain::current() {
            slot.order = d.order_of(k);
        }
        let n = self.index.partition_point(|&(c, _)| c < k);
        self.index.insert(n, (k, id));
        self.place[id as usize] = Nth::of(self.slots.len());
        self.slots.push(slot);
        let i = self.slots.len() - 1;
        self.recheck(world, i, xpute_runtime::clock::now());
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
        let before = self.slots[i].retained();
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on -= before;
        } else {
            self.retained_off -= before;
        }
        for t in self.slots[i].tiers.iter() {
            let_go(&mut self.meshes, t.slot.index, id);
        }
        for d in self.slots[i].decor.iter() {
            d.release(&mut self.draw);
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

    fn find(&self, k: &Chunk) -> Option<usize> {
        let n = self.index.binary_search_by_key(k, |&(c, _)| c).ok()?;
        self.place[self.index[n].1 as usize].get()
    }

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
        if s.layout.is_none() {
            s.unmeasured();
        }
        s.fresh = true;
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
    /// or a block was refused.
    pub fn build_step(&mut self, world: &mut World, budget_ms: f64) -> bool {
        let (slots, place, meshes, landed) = (&mut self.slots, &*self.place, &mut self.meshes, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick(world, &self.builds, budget_ms, &xpute_runtime::clock::now, &mut |e| match e {
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

    /// What the chunks hold, with the run sets `.fixed.runs` has lent as the
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
            out.decor_steps += s.decor.len() as u32;
            out.decor_instances += s.decor.iter().flat_map(|d| d.groups()).map(|(_, n)| n).sum::<u32>();
        }
        out
    }

    /// Free slots per pool after what mounted chunks will take.
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

    fn materialize(&mut self, world: &mut World, sc: &mut Scratch) {
        let Some(d) = domain::current() else { return };
        sc.drawn.clear();
        sc.building.clear();
        sc.retier.clear();
        let mut recubing = 0;
        for (i, s) in self.slots.iter().enumerate() {
            match s.role {
                Role::Drawn => {
                    sc.drawn.push(s.chunk);
                    if s.retier {
                        recubing += 1;
                    } else if s.retiers() {
                        sc.retier.push((s.chunk, (pool_of(s.chunk, s.desired), s.slots_for(s.desired, self.generation(i)))));
                    }
                }
                Role::Building => sc.building.push((s.chunk, s.layout.is_some())),
            }
        }
        sc.drawn.sort_unstable();
        sc.building.sort_unstable_by_key(|&(c, _)| c);
        let now = Now {
            ms: xpute_runtime::clock::now(),
            goal: d.pieces(),
            drawn: &sc.drawn,
            building: &sc.building,
            recubing,
            retier: &sc.retier,
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
        let now = xpute_runtime::clock::now();
        for &k in &plan.start {
            xpute_core::ensure!(self.find(&k).is_none(), ENOTRECOVERABLE);
            self.add(world, k);
            self.counts.added += 1;
        }
        for &k in &plan.retier {
            if let Some(i) = self.find(&k) {
                self.slots[i].retier = true;
                self.recheck(world, i, now);
            }
        }
        // Every cover slot held belongs to a current root, and no root holds two.
        let roots = d.roots();
        let mut owned = [false; COVER_SLOTS as usize];
        for at in Pool::Cover.range() {
            let Some(id) = self.meshes.holder[at].get() else { continue };
            let chunk = self.place[id].get().map(|i| self.slots[i].chunk);
            let root = chunk.and_then(|c| roots.binary_search(&c).ok());
            xpute_core::ensure!(root.is_some_and(|r| !core::mem::replace(&mut owned[r], true)), ENOTRECOVERABLE, at);
        }
    }

    /// The arena was recreated with the device: forget every tier. A layout kept
    /// ready over lost tiers would draw nothing until its scale changed.
    fn lose_meshes(&mut self) {
        let epoch = arena_epoch();
        if epoch == self.meshes.epoch {
            return;
        }
        self.meshes.epoch = epoch;
        self.meshes.holder.fill(Nth::NONE);
        for s in self.slots.iter_mut() {
            s.tiers.clear();
            s.layout = None;
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
        sc.eligible.extend((0..self.slots.len()).filter(|&i| full_recheck || (self.slots[i].fresh || self.slots[i].settling())));
        for k in 0..sc.eligible.len() {
            let i = sc.eligible[k];
            self.recheck(world, i, t0);
        }
        if full_recheck {
            self.view_dirty = false;
        }

        self.materialize(world, &mut sc);
        self.order(&mut sc.order, &mut sc.keys);

        sc.pending.clear();
        sc.pending.extend(sc.order.iter().copied().filter(|&i| self.slots[i].pending()));
        let pending_total = sc.pending.len() as u32;

        let commits = under_budget(&sc.pending, param::COMMITS_PER_FRAME, deadline, hard, now, |i| self.commit(world, i));
        let commit_left = sc.pending.len() as u32 - commits.visited;
        let mut committed = commits.ran;

        let slots = &self.slots;
        self.builds.clear();
        self.builds.extend(sc.pending.iter().filter(|&&i| slots[i].pending()).map(|&i| slots[i].id));
        if !self.builds.is_empty() {
            builds().notify();
        }
        committed += core::mem::take(&mut self.build_committed);

        self.reclaim(&sc.order);
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
            if s.in_frustum {
                c.in_frustum += 1;
            }
            if s.settling() {
                c.settling += 1;
            }
            if s.pending() {
                c.pending += 1;
            }
            if s.role == Role::Building && s.layout.is_some() {
                c.ready_waiting += 1;
            }
            if let Some(since) = s.pending_since {
                let waited = t0 - since;
                if s.in_frustum {
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
        c.retained_on = self.retained_on;
        c.retained_off = self.retained_off;
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
            let Some((scale, generation)) = s.layout else { continue };
            if s.role == Role::Building || !s.in_frustum {
                continue;
            }
            let (x0, y0) = s.base();
            let translation = DrawList::translation([x0, y0, 0.0], frame.eye);
            let Some(t) = s.tier(scale, generation) else { continue };
            // Slots are recycled between chunks, so a chunk must draw only from a slot
            // it holds; otherwise it can draw another chunk's ground.
            xpute_core::ensure!(holder[t.slot.index] == Nth::of(s.id as usize), ENOTRECOVERABLE);
            if t.box_cnt == 0 {
                continue;
            }
            // Mesh heights are steps from the tier's floor, so the floor goes in the translation.
            let at = [translation[0], translation[1], translation[2] + t.heights.floor as f32];
            let (unit, z_step) = ((mesh_step(scale, s.level()) * GROUND.finest_in(s.level())) as f32, t.heights.step as f32);
            let (mut left, mut slot) = (t.box_cnt, Some(t.slot.index));
            while let (Some(at_slot), true) = (slot, left > 0) {
                let m = slot_at(at_slot);
                let n = left.min(m.capacity);
                self.draw.draw_arena(
                    m.group,
                    m.first_box,
                    n,
                    at,
                    unit,
                    z_step,
                    group_is_fine(m.group),
                );
                left -= n;
                slot = self.meshes.next[at_slot].get();
            }
            self.level_boxes[s.level() as usize] += t.box_cnt;
        }
    }

    /// Piece `s` as it is drawn now, as the block of the pyramid's cells it
    /// draws; a chunk that is not a drawn piece, or has no layout, draws
    /// nothing a layer could stand on.
    fn drawn_of<'a>(&'a self, s: &'a Slot) -> Option<Drawn<'a>> {
        if s.role != Role::Drawn || !s.layout_ready {
            return None;
        }
        let cube = s.layout_primary;
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
    /// screen — a coarser piece while what replaces it builds — and none
    /// where nothing drawn stands there.
    ///
    /// A column is drawn by every chunk over it that holds something, each
    /// cut to its own heights (world/pyramid/chunk.rs), so a chunk under the
    /// surface tops out where it is cut. The highest one drawn there is the
    /// one whose top is the surface's.
    pub fn surface_at(&self, at: &Pvf3) -> Option<(u32, f64)> {
        (0..=TOP_LEVEL).find_map(|level| {
            let column = Chunk::column_of(level, at);
            let (from, to) = (self.index.partition_point(|(c, _)| c < column.start()), self.index.partition_point(|(c, _)| c <= column.end()));
            self.index[from..to].iter().rev().find_map(|&(_, id)| {
                let d = self.drawn_of(&self.slots[self.place[id as usize].get()?])?;
                // The chunk lies in one page at every level over its own, so
                // the point's cell and the chunk's first are of one page.
                let [cx, cy] = GROUND.cell_of(d.cell_level(), at).in_page();
                let [fx, fy] = GROUND.cell_of(d.cell_level(), &Pvf3 { i: d.base, ..Default::default() }).in_page();
                Some((d.cell_level(), d.top(cx - fx, cy - fy)?))
            })
        })
    }

    /// Where every chunk is and whether it is on screen, measured against the
    /// frame.
    fn refresh(&mut self, frame: &Frame) {
        let domain = domain::current();
        for i in 0..self.slots.len() {
            let visible = visible(frame, &self.slots[i]);
            let order = domain.map_or(Order::LAST, |d| d.order_of(self.slots[i].chunk));
            let s = &mut self.slots[i];
            s.order = order;
            let was = s.in_frustum;
            if visible != was {
                s.in_frustum = visible;
                self.view_dirty = true;
            }
        }
    }

    fn order(&self, idx: &mut Vec<usize>, keys: &mut Vec<Priority>) {
        let holders = self.builder.holders();
        keys.clear();
        keys.extend(self.slots.iter().map(|s| Priority {
            holds: holders.contains(&Some(s.id)),
            order: s.order,
        }));
        idx.clear();
        idx.extend(0..self.slots.len());
        // Ties broken by index, which matches a stable sort without its merge buffer.
        idx.sort_unstable_by(|&a, &b| priority(&keys[a], &keys[b]).then(a.cmp(&b)));
    }

    fn set_pending(&mut self, i: usize, pending: bool, now: f64) {
        let s = &mut self.slots[i];
        s.pending_since = pending.then(|| s.pending_since.unwrap_or(now));
    }

    fn stop_work(&mut self, world: &mut World, i: usize) {
        self.builder.want(world, self.slots[i].id, [None, None]);
        settle(&mut self.meshes, &self.builder);
    }

    /// Sets the desired scale and whether the chunk is pending. A drawn piece
    /// wanting another scale waits for the frontier (`materialize`); a building
    /// chunk drops a tier no longer wanted.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64) {
        self.slots[i].fresh = false;
        let Some(d) = domain::current() else { return };
        let s = &self.slots[i];
        let (level, chunk) = (s.level(), s.chunk);
        let objects = level == 0 && world.store.entries(L2_IDX(chunk.corner())).is_some();
        let current = self.generation(i);
        let desired = scale_of(chunk, d.outermost(chunk), d.order_of(chunk).near, d.tau(), objects);
        if self.slots[i].role == Role::Building && self.slots[i].layout.is_some_and(|(p, _)| p != desired) {
            self.release_tiers(i, |_, _| false);
        }
        let s = &mut self.slots[i];
        s.desired = desired;
        if !s.retiers() {
            s.retier = false;
        }
        let pending = s.layout != Some((s.target(), current));
        self.set_pending(i, pending, now);
        let s = &mut self.slots[i];
        if !s.pending() {
            self.stop_work(world, i);
        }
    }

    fn release_tiers(&mut self, i: usize, keep: impl Fn(Scale, u32) -> bool) {
        let s = &mut self.slots[i];
        prune(s, &mut self.meshes, |t| keep(t.scale, t.generation), None);
        if s.layout.is_some_and(|(p, g)| s.tier(p, g).is_none()) {
            s.layout = None;
        }
        self.account(i, before);
    }

    /// Whether the chunk holds `primary` of the current generation; asks the
    /// builder otherwise, unless the tier is known not to fit the free slots.
    fn ask(&mut self, world: &mut World, i: usize, primary: Scale) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let lacks = s.tier(primary, generation).is_none();
        let room = !lacks || s.slots_for(primary, generation) <= claimable(&self.meshes, pool_of(s.chunk, primary));
        self.builder.want(world, s.id, [(lacks && room).then_some(primary), None]);
        settle(&mut self.meshes, &self.builder);
        if !room {
            self.slots[i].fresh = true;
        }
        !lacks
    }

    fn apply_layout(&mut self, i: usize, primary: Scale, generation: u32) {
        self.counts.layouts += 1;
        let s = &mut self.slots[i];
        s.layout = Some((primary, generation));
        match s.tier(primary, generation).and_then(|t| t.bounds) {
            Some(b) => (s.bc, s.radius) = (b.center(), b.radius()),
            None => s.unmeasured(),
        }
    }

    /// Lets go of every tier the layout does not draw: a slot
    /// is held only by what is on the screen or about to be.
    fn trim(&mut self, i: usize) {
        let layout = self.slots[i].layout;
        self.release_tiers(i, move |scale, generation| layout == Some((scale, generation)));
    }

    /// Applies what `recheck` asked for, if it can: the desired tier once it
    /// is built. A drawn piece changes cube only once the frontier has let it
    /// hold a slot for it (`Slot::recube`).
    fn commit(&mut self, world: &mut World, i: usize) -> bool {
        let s = &self.slots[i];
        if !s.pending() {
            return false;
        }
        let (p, generation) = (s.target(), self.generation(i));
        let asked = self.ask(world, i, p);
        if !asked || !self.ensure_decor(world, i, p, generation) {
            return false;
        }
        let s = &self.slots[i];
        let retiered = s.role == Role::Drawn && s.layout.is_some_and(|(q, g)| g == generation && q != p);
        self.apply_layout(i, p, generation);
        self.trim(i);
        if retiered {
            self.retiered = self.retiered.wrapping_add(1);
            self.changes += 1;
        }
        let s = &mut self.slots[i];
        s.retier = s.retiers() && s.retier;
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
            for d in s.decor.take().iter() {
                d.release(draw);
            }
            s.decor_whole = false;
            self.account(i, before);
            freed += 1;
        }
    }
}

fn same_view(a: &Frame, b: &Frame) -> bool {
    a.anchor == b.anchor && a.origin == b.origin && a.frustum == b.frustum && a.camera == b.camera
}

#[derive(Clone, Copy)]
struct Priority {
    holds: bool,
    order: Order,
}

/// Slot holders first, then distance from the eye (`Order`). Any class over
/// the distance, or sizing by screen extent, sent builds to the edge of the
/// view while the middle stayed coarse (2026-09-26).
fn priority(a: &Priority, b: &Priority) -> core::cmp::Ordering {
    b.holds.cmp(&a.holds).then(a.order.cmp(&b.order))
}

fn visible(frame: &Frame, s: &Slot) -> bool {
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
    // A chunk whose sphere contains the camera is always visible.
    let mut visible = len3(&[c[0] - x, c[1] - y, c[2] - z]) <= s.radius;
    if !visible {
        visible = frame.frustum.sees([x, y, z], s.radius);
        // Horizon cull (Cozzi & Ring): a point is above the eye's horizon while
        // E·P >= r², from the planet's center, widened by the chunk's radius.
        if visible {
            if let Some(globe) = frame.globe {
                let r = globe.radius_u();
                let (e, p) = ([c[0], c[1], c[2] + r], [x, y, z + r]);
                visible = dot3(&e, &p) + s.radius * len3(&e) >= r * r
            }
        }
    }
    visible
}

fn free_at(meshes: &Meshes, at: usize) -> bool {
    meshes.holder[at].is_none()
}

fn claimable(meshes: &Meshes, pool: Pool) -> u32 {
    pool.range().filter(|&at| free_at(meshes, at)).count() as u32
}

/// `k` free slots of `pool`, all or none, chained in order. Taking some and
/// waiting for the rest would let two tiers deadlock on each other's slots.
fn claim(meshes: &mut Meshes, pool: Pool, k: u32, holder: u32) -> Option<usize> {
    if claimable(meshes, pool) < k {
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

fn let_go(meshes: &mut Meshes, at: usize, id: u32) {
    let mut slot = Some(at);
    while let Some(at) = slot {
        slot = meshes.next[at].take();
        if meshes.holder[at] == Nth::of(id as usize) {
            meshes.holder[at] = Nth::NONE;
        }
    }
}

/// Drops the tiers `keep` rejects, and their slots unless a kept or landing tier shares them.
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

    /// Everything held, leaving nothing.
    fn take(&mut self) -> Held<T> {
        core::mem::replace(self, Held::EMPTY)
    }
}

fn chained(meshes: &Meshes, at: usize, n: u32) -> Option<usize> {
    (0..n).try_fold(at, |at, _| meshes.next[at].get())
}

struct Landing {
    id: u32,
    generation: u32,
    scale: Scale,
    first: usize,
    slots: u32,
    /// Slots of the tier it is rebuilt over, which it writes over.
    over: u32,
    bounds: Option<Aabb>,
}

/// Holds every slot a tier of `boxes` boxes fills, at once, or says there is
/// not the room (build.rs `BuildEvent::Reserve`). A tier rebuilt over one of
/// the same cube writes over its slots and takes only what it needs past
/// them. Where the room is not there, how many it needs is kept: the
/// frontier and the quality policy count it so from then on, and it is not
/// asked for again until the slots are free (`Scheduler::ask`).
fn reserve(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) -> bool {
    let BuildEvent::Reserve { id, generation, scale, boxes } = e else { return false };
    // One bake is open at a time: a leftover landing is from one that closed
    // without landing.
    if let Some(l) = meshes.landing.take() {
        release(meshes, l);
    }
    let Some(i) = place[id as usize].get() else { return false };
    let s = &slots[i];
    if scale != s.target() {
        return false;
    }
    let level = s.level();
    let pool = pool_of(s.chunk, scale);
    let k = boxes.div_ceil(capacity(level, mesh_step(scale, level))).max(1);
    let over = s.tiers.iter().find(|t| t.scale == scale).map(|t| t.slot.index);
    let taken = match over {
        Some(first) => Some((first, (0..).take_while(|&n| chained(meshes, first, n).is_some()).count() as u32)),
        None => claim(meshes, pool, k, id).map(|first| (first, 0)),
    };
    let Some((first, held)) = taken else {
        return refuse(slots, i, meshes, generation, scale, k, level);
    };
    if held > 0 && k > held {
        let last = chained(meshes, first, held - 1).unwrap_or(first);
        let more = claim(meshes, pool, k - held, id);
        match more {
            Some(more) => meshes.next[last] = Nth::of(more),
            None => return refuse(slots, i, meshes, generation, scale, k, level),
        }
    }
    meshes.landing = Some(Landing {
        id,
        generation,
        scale,
        first,
        slots: k,
        over: held,
        bounds: None,
    });
    true
}

/// Releases the landing once its bake is no longer open. Called after every
/// call that can close a bake.
fn settle(meshes: &mut Meshes, builder: &Builder) {
    let open = builder.baking();
    if let Some(l) = meshes.landing.take_if(|l| open != Some((l.id, l.scale, l.generation))) {
        release(meshes, l);
    }
}

fn release(meshes: &mut Meshes, l: Landing) {
    if l.over == 0 {
        let_go(meshes, l.first, l.id);
    } else if let Some(last) = chained(meshes, l.first, l.over - 1) {
        if let Some(more) = meshes.next[last].take() {
            let_go(meshes, more, l.id);
        }
    }
}

fn refuse(slots: &mut [Slot], i: usize, meshes: &mut Meshes, generation: u32, scale: Scale, k: u32, level: u32) -> bool {
    slots[i].need = Some((generation, scale, k));
    slots[i].fresh = true;
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
        scale,
        at,
        first,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(l) = meshes.landing.as_ref().filter(|l| (l.id, l.generation, l.scale) == (id, generation, scale)) else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let level = slots[i].level();
    let step = mesh_step(scale, level);
    let head = slot_at(l.first);
    let n = first / head.capacity;
    let Some(slot) = chained(meshes, l.first, n).map(slot_at) else { return };
    xpute_core::ensure!(first % head.capacity == 0 && box_cnt <= slot.capacity, ENOTRECOVERABLE, first, box_cnt);
    let box_words = group_box_words(slot.group);
    let asked = if slot.capacity == FINE_BOXES { &mut meshes.fine_peak } else { &mut meshes.coarse_peak };
    *asked = (*asked).max(first + box_cnt);
    // SAFETY: the block's boxes in `.fixed.bake`, just written by the bake, in
    // the slot group's format.
    let words = unsafe { core::slice::from_raw_parts(ptr_at::<u32>(at as usize), (box_cnt * box_words) as usize) };
    let unit = (step * GROUND.finest_in(level)) as f64;
    let mut bounds = meshes.landing.as_ref().and_then(|l| l.bounds);
    for r in words.chunks_exact(box_words as usize) {
        let (min, size) = box_bounds(r, unit, heights);
        let b = Aabb { min, max: std::array::from_fn(|a| min[a] + size[a]) };
        bounds = Some(bounds.map_or(b, |a| a.join(b)));
    }
    if box_cnt > 0 {
        let before = arena_epoch();
        // SAFETY: the same words, as bytes.
        let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), words.len() * 4) };
        // No device: the tier is asked for again once there is one.
        if !arena_write(slot.group as usize, slot.first_box * box_words * 4, bytes) {
            meshes.landing = None;
            return;
        }
        // The write that recreated the arena lost nothing written before it.
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
        scale,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let s = &slots[i];
    if s.tier(scale, generation).is_some() || scale != s.target() {
        return;
    }
    let level = s.level();
    let landing = match meshes.landing.take() {
        Some(l) if (l.id, l.generation, l.scale) == (id, generation, scale) => l,
        other => {
            meshes.landing = other;
            // An empty mesh reserved nothing; it takes one slot here.
            if box_cnt > 0 {
                return;
            }
            let over = s.tiers.iter().find(|t| t.scale == scale).map(|t| t.slot.index);
            let taken = match over {
                Some(first) => Some(first),
                None => claim(meshes, pool_of(s.chunk, scale), 1, id),
            };
            let Some(first) = taken else {
                refuse(slots, i, meshes, generation, scale, 1, level);
                return;
            };
            Landing {
                id,
                generation,
                scale,
                first,
                slots: 1,
                over: 0,
                bounds: None,
            }
        }
    };
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
    let layout = s.layout;
    prune(s, meshes, |t| layout == Some((t.scale, t.generation)), Some(slot.index));
    // An older-generation tier on the same slots now shows these boxes.
    for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
        t.box_cnt = box_cnt;
        t.bounds = landing.bounds;
        t.heights = heights;
    }
    s.tiers.push(Tier {
        scale,
        generation,
        bounds: landing.bounds,
        slot,
        box_cnt,
        heights,
    });
}

/// Slot `index`'s surface as its tier of `step` holds it, a word a block:
/// the lane's first (CHUNK_CELLS / step)² words.
fn surface(index: usize, step: u32) -> &'static mut [u32] {
    let (off, words) = surface_of(index);
    let axis = CHUNK_CELLS / step;
    xpute_core::ensure!(axis * axis <= words, ENOTRECOVERABLE, index, step);
    // SAFETY: the slot's own lane in `.fixed.surface`, written only by the
    // tier landing in the slot and read by what stands on it, both the chunk
    // table's, on its one thread.
    unsafe { lane(range(Range::Surface), off, (axis * axis) as usize) }
}

/// A cell-center height query needs no CPU copy of the mesh. Quantized tops
/// use the mesh's own height scale, with zero reserved for an empty center.
/// Written into the lane of the tier's first slot, which the slot's tier
/// sizes (mesh_slot.rs `surface_of`), a block of boxes at a time: cleared
/// with the first.
fn surface_centers(index: usize, first: bool, words: &[u32], stride: usize, step: u32, level: u32, heights: Heights) {
    let axis = (CHUNK_CELLS / step) as usize;
    let unit = step as f64 * GROUND.scale(level).side();
    let tops = surface(index, step);
    if first {
        tops.fill(0);
    }
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
