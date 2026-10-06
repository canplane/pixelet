// pxl-kernel/world/chunk/table.rs

//! The chunks' table: which chunks are mounted, the role each has in the
//! frontier (frontier.rs), which tier each shows, and what to build next.
//! Every mounted chunk holds a mesh slot or has one held for it, so the table
//! is never larger than the arena.

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch, arena_write};
use pxl_render::look::globe::Globe;
use pxl_render::mesh::chunk_mesher::{box_bounds, Heights};
use pxl_render::mesh::paint::terrain::adapter::beside_index;
use pxl_render::mesh::run::APRON;

use crate::sched::profiler::record::{MB, ZOOMS};
use pxl_spec::space::observation::Bounds as _;
use pxl_spec::space::observation::Frustum;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::GROUND;
use pxl_spec::space::v3::{dot3, len3, Aabb, V3};
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::section::span_of;
use xpute_runtime::sched::edge::Notify;
use xpute_runtime::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use crate::layer::billboard::field::Billboards;
use crate::layer::environment::state::EnvironmentDraw;
use crate::layer::marker::coverage_gizmo::CoverageGizmo;
use crate::layer::marker::debug_lines::DebugLines;
use crate::layer::marker::hover_cursor::{render_edits, GhostDraw, RenderEdits};
use crate::layer::marker::location_puck::Markers;
use crate::layer::model::store::Models;
use crate::layer::object::store::EntryAt;
use crate::mem::fixed::{lane, range, Range};
use crate::param;
use crate::shell::setting::store::settings_store;
use crate::world::chunk::build::{draws_as_held, BuildEvent, Builder, World};
use crate::world::chunk::frontier::{pool_of, zoom_of, Env, Frontier, Now, Slots};
use crate::world::chunk::mesh_slot::{capacity, group_box_words, group_bytes, group_is_fine, slot_at, MeshSlot, Pool, ARENA, COVER_SLOTS, FINE_BOXES, OCCUPANCY};
use crate::world::chunk::source::{draws, objects_revision_of};
use pxl_spec::space::zoom::{units, Zoom};
use xpute_core::math::scalar::round;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;

use crate::world::observation::{Known, Order};
use crate::world::pyramid::chunk::{Chunk, FINEST_CHUNK, TOP_CHUNK};
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::read::TileRead;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Drawn,
    /// Not drawn until every chunk of its move is ready.
    Building,
}

#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The sea under the eye, in the world's doubles; chunks are measured about it.
    pub anchor: V3,
    /// Draw translations are taken relative to the eye.
    pub eye: V3,
    /// About the anchor.
    pub frustum: Frustum,
    pub camera: V3,
    pub delta_s: f64,
    /// What is left of the turn's quota, not the quota itself.
    pub remaining_ms: f64,
    pub hard_slack_ms: f64,
    pub globe: Option<Globe>,
}

impl Frame {
    /// A point about the anchor, its X on the anchor's lap.
    pub fn world_of(&self, at: V3) -> V3 {
        let x = pxl_spec::space::zoom::nearest(at[0], self.anchor[0]);
        [x - self.anchor[0], at[1] - self.anchor[1], at[2] - self.anchor[2]]
    }
}

#[derive(Clone, Copy)]
struct Tier {
    cell_zoom: Zoom,
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
    pub run_sets: u32,
    pub run_set_mb: u32,
    pub tiers: u32,
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
    /// Slots a tier needs, for (generation, cell zoom), found when a bake did not
    /// fit (`BuildEvent::Reserve`); not asked for again until that many are free.
    need: Option<(u32, Zoom, u32)>,
    /// A drawn piece the frontier let build another cell zoom; a slot is held for it.
    retier: bool,
    /// The layout's tiers plus the last one landed. A source change keeps the
    /// old generation's tiers drawn until a layout of the new one is applied.
    tiers: Held<Tier>,
    /// The applied layout's primary cell zoom and generation; none until one is, or
    /// once its tier is gone.
    layout: Option<(Zoom, u32)>,
    desired: Zoom,
    sources: Sources,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Sources {
    /// A tier above a leaf bakes one color with no texture layer, so the setting
    /// is part of what the tier was built from.
    textured: bool,
    objects_revision: u32,
    hide: Option<EntryAt>,
    carve: Option<EntryAt>,
    /// Zoom of the tile the chunk and each apron neighbour are read from, so a
    /// finer tile arriving rebuilds the chunk and its shared edges.
    read_from: Option<Zoom>,
    beside_from: [Option<Zoom>; 8],
}

impl Sources {
    fn of(k: Chunk, edits: RenderEdits) -> Sources {
        let settings = settings_store();
        let drawn = |at: &EntryAt| draws(k, *at) && draws_as_held(k, at.zoom);
        Sources {
            textured: settings.textures,
            objects_revision: objects_revision_of(k),
            hide: edits.hide.filter(|at| edits.hide_in.0 && drawn(at)),
            carve: edits.carve.filter(drawn),
            read_from: Terrain::pyramid().residency().drawn_from(TileRead::of(GROUND, k).tile()).map(|p| p.zoom),
            beside_from: {
                let mut from = [None; 8];
                for ([qx, qy], p) in TileRead::of(GROUND, k).tiles_beside(APRON) {
                    from[beside_index(qx, qy)] = Terrain::pyramid().residency().drawn_from(p).map(|p| p.zoom);
                }
                from
            },
        }
    }
}

impl Slot {
    fn base(&self) -> (f64, f64) {
        let (x, y, _) = self.chunk.rect();
        (x, y)
    }

    /// Culling sphere before the mesh measures it: the known contents, or the
    /// whole chunk. A sphere at a corner or at sea level culled chunks that were
    /// then never built, leaving holes with nothing left to fetch.
    fn unmeasured(&mut self) {
        let region = self.chunk.region();
        let b = Known.contents(self.chunk.cell_zoom(), region).within(region).unwrap_or(region);
        let c = b.center();
        self.bc = [c[0] - region.min[0], c[1] - region.min[1], c[2]];
        self.radius = b.radius();
    }

    fn slots_for(&self, cell_zoom: Zoom, generation: u32) -> u32 {
        self.need.filter(|&(g, c, _)| g == generation && c == cell_zoom).map_or(1, |(_, _, k)| k)
    }

    fn pending(&self) -> bool {
        self.pending_since.is_some()
    }

    /// Pending with a layout up.
    fn settling(&self) -> bool {
        self.pending() && self.layout.is_some()
    }

    fn tier(&self, cell_zoom: Zoom, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.cell_zoom == cell_zoom && t.generation == generation)
    }

    fn retiers(&self) -> bool {
        self.layout.is_some_and(|(p, _)| p != self.desired)
    }

    /// The cell zoom it builds now: the desired one, except a drawn piece the frontier
    /// has not let retier rebuilds its shown cell zoom in place, so an edit still shows.
    fn target(&self) -> Zoom {
        match self.layout {
            Some((p, _)) if self.role == Role::Drawn && self.retiers() && !self.retier => p,
            _ => self.desired,
        }
    }

    /// Slots held for it and not yet taken: none while it has a tier of the target cell zoom.
    fn reserves(&self, generation: u32) -> Option<(Pool, u32)> {
        let cell_zoom = self.target();
        (!self.tiers.iter().any(|t| t.cell_zoom == cell_zoom)).then(|| (pool_of(self.chunk, cell_zoom), self.slots_for(cell_zoom, generation)))
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
    /// Bit z: a cell of zoom z waited for a slot.
    pub wait_zooms: u32,
    pub chained: u32,
    pub fine_peak: u32,
    pub coarse_peak: u32,
    pub refused: u32,
    pub waiting: u32,
    pub zoom_boxes: [u32; ZOOMS],
    /// Chunks wanting a layout the commit pass did not reach: whether the frame's cap is the bottleneck.
    pub commit_left: u32,
}

/// An index in a `.fixed.mounts` lane, stored plus one so a zeroed lane reads as none.
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

/// `.fixed.mounts` lanes: slot holder, slot next, mount place, free mounts, index.
const MOUNT_LANES: [u32; 5] = [
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<Nth>()) as u32,
    (OCCUPANCY * size_of::<u32>()) as u32,
    (OCCUPANCY * size_of::<(Chunk, u32)>()) as u32,
];
const MOUNT_ALIGN: u32 = 8;

pub const MOUNTS_BYTES: u32 = span_of(&MOUNT_LANES, MOUNT_ALIGN);

/// # Safety
/// Each lane is taken once, by the table alone; zero is valid for every entry.
unsafe fn mount_lane<T>(k: usize) -> &'static mut [T] {
    unsafe { lane(range(Range::Mounts).nth(&MOUNT_LANES, MOUNT_ALIGN, k), 0, OCCUPANCY) }
}

pub struct Meshes {
    holder: &'static mut [Nth],
    /// Next slot in a tier's chain; a tier past one slot continues in any free slot of its pool.
    next: &'static mut [Nth],
    /// The tier the open bake is landing; all its slots are held at once in `reserve`.
    landing: Option<Landing>,
    epoch: u32,
    pub chained: u32,
    /// Bumped whenever a tier needs more slots than were held; the quality policy watches it.
    needs: u32,
    fine_peak: u32,
    coarse_peak: u32,
    waits: u32,
    wait_zooms: u32,
}

pub struct Scheduler {
    pub meshes: Meshes,
    zoom_boxes: [u32; ZOOMS],
    pub builder: Builder,
    pub draw: DrawList,
    pub ghost: GhostDraw,
    pub models: Models,
    pub markers: Markers,
    pub billboards: Billboards,
    pub environment: EnvironmentDraw,
    pub debug: DebugLines,
    coverage: CoverageGizmo,
    slots: Vec<Slot>,
    /// Index into `slots` by mount. Nothing naming a mount outlives it (`remove`),
    /// so a freed mount can be reused at once.
    place: &'static mut [Nth],
    mounts: Free,
    index: Index,
    domain_seen: u32,
    frame: Option<Frame>,
    /// An input changed that no recheck has seen; the next tick rechecks every
    /// chunk. Frames are requested until then, or the world stops at the last frame.
    view_dirty: bool,
    /// Pending chunks the last commit pass did not reach.
    commit_left: u32,
    counts: ChunkCounts,
    builds: Vec<u32>,
    landed: Vec<u32>,
    build_committed: u32,
    moves: (u32, u32),
    /// Recut pace for the profiler.
    refined: u32,
    retiered: u32,
    changes: u32,
    /// Moves whenever the set of drawn chunks does.
    drawn_revision: u32,
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
        let cell_zoom = zoom_of(c, self.d.outermost(c));
        (pool_of(c, cell_zoom), self.table.slots_of(c, cell_zoom))
    }

    fn buildable(&self, c: Chunk) -> bool {
        Terrain::pyramid().residency().drawn_from(TileRead::of(GROUND, c).tile()).is_some()
    }

    fn order(&self, c: Chunk) -> Order {
        self.d.order_of(c)
    }
}

impl Scheduler {
    pub fn new() -> Scheduler {
        arena_configure(&group_bytes());
        // SAFETY: the five lanes, each taken once here and the table's alone.
        let (holder, next, place, free, index) = unsafe { (mount_lane(0), mount_lane(1), mount_lane(2), mount_lane::<u32>(3), mount_lane(4)) };
        for (k, id) in free.iter_mut().enumerate() {
            *id = (OCCUPANCY - 1 - k) as u32;
        }
        Scheduler {
            zoom_boxes: [0; ZOOMS],
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
                wait_zooms: 0,
            },
            builder: Builder::new(),
            draw: DrawList::default(),
            ghost: GhostDraw::default(),
            models: Models::default(),
            markers: Markers::default(),
            billboards: Billboards::default(),
            environment: EnvironmentDraw::default(),
            debug: DebugLines::default(),
            coverage: CoverageGizmo::default(),
            slots: Vec::new(),
            place,
            mounts: Free { lane: free, len: OCCUPANCY },
            index: Index { lane: index, len: 0 },
            domain_seen: u32::MAX,
            frame: None,
            view_dirty: true,
            commit_left: 0,
            counts: ChunkCounts::default(),
            scratch: Scratch::default(),
            builds: Vec::new(),
            landed: Vec::new(),
            build_committed: 0,
            moves: (0, 0),
            refined: 0,
            retiered: 0,
            changes: 0,
            drawn_revision: 0,
            changes_peak: 0,
        }
    }

    pub fn slots_of(&self, c: Chunk, cell_zoom: Zoom) -> u32 {
        self.find(&c).map_or(1, |i| self.slots[i].slots_for(cell_zoom, self.generation(i)))
    }

    pub fn pace(&self) -> [u32; 3] {
        [self.refined, self.retiered, self.changes_peak]
    }

    /// The chunks counted for the profiler at `t0`, the slots' waits taken
    /// whether or not they are: a recording starts from now.
    pub fn census(&mut self, counted: bool, t0: f64) -> Census {
        let mut c = Census {
            chunks: self.slots.len() as u32,
            commit_left: self.commit_left,
            refused: self.moves.0,
            waiting: self.moves.1,
            zoom_boxes: self.zoom_boxes,
            slot_waits: core::mem::take(&mut self.meshes.waits),
            wait_zooms: core::mem::take(&mut self.meshes.wait_zooms),
            chained: core::mem::take(&mut self.meshes.chained),
            fine_peak: core::mem::take(&mut self.meshes.fine_peak),
            coarse_peak: core::mem::take(&mut self.meshes.coarse_peak),
            ..Census::default()
        };
        if !counted {
            return c;
        }
        for s in &self.slots {
            c.in_frustum += u32::from(s.in_frustum);
            c.settling += u32::from(s.settling());
            c.pending += u32::from(s.pending());
            c.ready_waiting += u32::from(s.role == Role::Building && s.layout.is_some());
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
        c
    }

    /// Each mounted chunk, whether it is drawn, and whether it is in the frustum.
    pub fn mounted(&self, out: &mut Vec<(Chunk, bool, bool)>) {
        out.clear();
        out.extend(self.slots.iter().map(|s| (s.chunk, s.role == Role::Drawn, s.in_frustum)));
    }

    fn drawn_slot(&self, p: &Pvf3) -> Option<&Slot> {
        (TOP_CHUNK..=FINEST_CHUNK)
            .rev()
            .find_map(|z| self.slots.iter().find(|s| s.role == Role::Drawn && s.chunk.named() == Chunk::holding(z, p)))
    }

    /// The cells the finest drawn chunk holding `p` is drawn in.
    pub fn cells_at(&self, p: &Pvf3) -> Option<Zoom> {
        self.drawn_slot(p)?.layout.map(|(cell_zoom, _)| cell_zoom)
    }

    /// The finest drawn chunk holding `p`, as tile zooms, as the gauge reads
    /// them: its own tile's, its cells', and the one it is read from.
    pub fn drawn_at(&self, p: &Pvf3) -> Option<(i32, Option<i32>, Option<i32>)> {
        let s = self.drawn_slot(p)?;
        Some((
            TileRead::of(GROUND, s.chunk).tile_zoom() as i32,
            s.layout.map(|(cell_zoom, _)| GROUND.z_of(cell_zoom)),
            s.sources.read_from.map(|z| z as i32),
        ))
    }

    /// Whether the last frame drew any ground on screen.
    pub fn shows_ground(&self) -> bool {
        self.zoom_boxes.iter().any(|&n| n > 0)
    }

    pub fn drawn_revision(&self) -> u32 {
        self.drawn_revision
    }

    pub fn needs_revision(&self) -> u32 {
        self.meshes.needs
    }

    fn add(&mut self, k: Chunk) {
        // The frontier starts no move without room, so running out of mounts is a bug.
        let id = self.mounts.pop().or_bug(Errno::ENOSPC);
        let edits = render_edits();
        let sources = Sources::of(k, edits);
        self.builder.add(id, k);
        self.builder.source(id);
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
            desired: k.cell_zoom(),
            sources,
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
        self.recheck(i, xpute_runtime::clock::now());
        self.view_dirty = true;
    }

    fn remove(&mut self, k: Chunk) {
        let Ok(n) = self.index.binary_search_by_key(&k, |&(c, _)| c) else { return };
        let (_, id) = self.index.remove(n);
        let i = self.place[id as usize].get().or_bug(Errno::ENOTRECOVERABLE);
        self.view_dirty = true;
        self.counts.removed += 1;
        self.builder.remove(id);
        settle(&mut self.meshes, &self.builder);
        for t in self.slots[i].tiers.iter() {
            let_go(&mut self.meshes, t.slot.index, id);
        }
        self.slots.swap_remove(i);
        if i < self.slots.len() {
            self.place[self.slots[i].id as usize] = Nth::of(i);
        }
        self.place[id as usize] = Nth::NONE;
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

    /// Sources changed: the old tiers stay drawn until a layout of the new
    /// generation is whole, and the rebuild goes into the old tier's slot.
    fn source(&mut self, i: usize) {
        self.view_dirty = true;
        let id = self.slots[i].id;
        self.builder.source(id);
        let s = &mut self.slots[i];
        if s.layout.is_none() {
            s.unmeasured();
        }
        s.fresh = true;
    }

    /// A tile that goes is not a change: a build finds no tile and waits, and
    /// what is drawn stays drawn.
    pub fn sources_step(&mut self) {
        let edits = render_edits();
        for i in 0..self.slots.len() {
            let k = self.slots[i].chunk;
            let sources = Sources::of(k, edits);
            if sources != self.slots[i].sources {
                self.slots[i].sources = sources;
                self.source(i);
            }
        }
    }

    /// The build task's step. Returns whether any build did work.
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
                if self.commit(i) {
                    self.build_committed += 1;
                }
            }
        }
        self.landed = landed;
        worked
    }

    pub fn take_counts(&mut self) -> ChunkCounts {
        core::mem::take(&mut self.counts)
    }

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

    fn materialize(&mut self, sc: &mut Scratch) {
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
        let seen = (self.drawn_revision, domain::revision(), Terrain::pyramid().residency().revision());
        self.coverage.step(&mut self.draw, settings_store().gizmos, seen, d.pieces(), &sc.drawn);
        self.moves = (plan.refused, plan.waiting + plan.held);
        self.refined = self.refined.wrapping_add(plan.refined);
        self.changes_peak = self.changes_peak.max(self.changes);
        self.changes = plan.swaps().count() as u32;
        if self.changes > 0 || !plan.drop.is_empty() {
            self.drawn_revision = self.drawn_revision.wrapping_add(1);
        }

        for &k in plan.cancel.iter().chain(&plan.drop) {
            self.remove(k);
        }
        for (leave, enter) in plan.swaps() {
            for &k in leave {
                self.remove(k);
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
            self.add(k);
            self.counts.added += 1;
        }
        for &k in &plan.retier {
            if let Some(i) = self.find(&k) {
                self.slots[i].retier = true;
                self.recheck(i, now);
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
    /// ready over lost tiers would draw nothing until its cell zoom changed.
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

    /// One frame. Returns whether the canvas has something new to draw.
    pub fn tick(&mut self, frame: Frame, now: &impl Fn() -> f64) -> bool {
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

        // Not under the budget: this states demand, and cutting it left chunks
        // drawing a coarse tier, which sort behind empty ones, never told to refine.
        // A recheck touches only its own chunk, so order does not matter.
        let full_recheck = self.view_dirty;
        sc.eligible.clear();
        sc.eligible.extend((0..self.slots.len()).filter(|&i| full_recheck || (self.slots[i].fresh || self.slots[i].settling())));
        for k in 0..sc.eligible.len() {
            let i = sc.eligible[k];
            self.recheck(i, t0);
        }
        if full_recheck {
            self.view_dirty = false;
        }

        self.materialize(&mut sc);
        self.order(&mut sc.order, &mut sc.keys);

        sc.pending.clear();
        sc.pending.extend(sc.order.iter().copied().filter(|&i| self.slots[i].pending()));
        let pending_total = sc.pending.len() as u32;

        let commits = under_budget(&sc.pending, param::COMMITS_PER_FRAME, deadline, hard, now, |i| self.commit(i));
        let commit_left = sc.pending.len() as u32 - commits.visited;
        let mut committed = commits.ran;

        let slots = &self.slots;
        self.builds.clear();
        self.builds.extend(sc.pending.iter().filter(|&&i| slots[i].pending()).map(|&i| slots[i].id));
        if !self.builds.is_empty() {
            builds().notify();
        }
        committed += core::mem::take(&mut self.build_committed);

        self.scratch = sc;
        let animating = self.models.tick(&mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
        self.draw_frame(&frame);

        self.commit_left = commit_left;
        let moving = self.moves.1 > 0 || self.slots.iter().any(|s| s.role == Role::Building && s.layout.is_some());
        committed > 0 || pending_total > committed || moving || self.view_dirty || animating
    }

    /// Chunks built for a move are not drawn until the move is made; the piece
    /// they replace covers their ground until then.
    fn draw_frame(&mut self, frame: &Frame) {
        self.draw.begin_frame();
        self.zoom_boxes = [0; ZOOMS];
        self.ghost.draw(&mut self.draw, frame.eye);
        self.models.draw(&mut self.draw, frame.eye);
        self.markers.draw(&mut self.draw, frame.eye);
        self.debug.draw(&mut self.draw, frame.eye);
        self.coverage.draw(&mut self.draw, frame.eye);
        let holder = &self.meshes.holder;
        for s in &self.slots {
            let Some((cell_zoom, generation)) = s.layout else { continue };
            if s.role == Role::Building || !s.in_frustum {
                continue;
            }
            let (x0, y0) = s.base();
            let translation = DrawList::translation([pxl_spec::space::zoom::nearest(x0, frame.eye[0]), y0, 0.0], frame.eye);
            let Some(t) = s.tier(cell_zoom, generation) else { continue };
            // Slots are recycled between chunks, so a chunk must draw only from a slot
            // it holds; otherwise it can draw another chunk's ground.
            xpute_core::ensure!(holder[t.slot.index] == Nth::of(s.id as usize), ENOTRECOVERABLE);
            if t.box_cnt == 0 {
                continue;
            }
            // Mesh heights are steps from the tier's floor, so the floor goes in the translation.
            let at = [translation[0], translation[1], translation[2] + t.heights.floor as f32];
            let (unit, z_step) = (units(cell_zoom) as f32, t.heights.step as f32);
            let (mut left, mut slot) = (t.box_cnt, Some(t.slot.index));
            while let (Some(at_slot), true) = (slot, left > 0) {
                let m = slot_at(at_slot);
                let n = left.min(m.capacity);
                self.draw.draw_arena(m.group, m.first_box, n, at, unit, z_step, group_is_fine(m.group));
                left -= n;
                slot = self.meshes.next[at_slot].get();
            }
            self.zoom_boxes[GROUND.z_of(t.cell_zoom) as usize] += t.box_cnt;
        }
    }

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

    fn stop_work(&mut self, i: usize) {
        self.builder.want(self.slots[i].id, [None, None]);
        settle(&mut self.meshes, &self.builder);
    }

    /// Sets the desired cell zoom and whether the chunk is pending. A drawn piece
    /// wanting another cell zoom waits for the frontier (`materialize`); a building
    /// chunk drops a tier no longer wanted.
    fn recheck(&mut self, i: usize, now: f64) {
        self.slots[i].fresh = false;
        let Some(d) = domain::current() else { return };
        let s = &self.slots[i];
        let chunk = s.chunk;
        let current = self.generation(i);
        let desired = zoom_of(chunk, d.outermost(chunk));
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
            self.stop_work(i);
        }
    }

    fn release_tiers(&mut self, i: usize, keep: impl Fn(Zoom, u32) -> bool) {
        let s = &mut self.slots[i];
        prune(s, &mut self.meshes, |t| keep(t.cell_zoom, t.generation), None);
        if s.layout.is_some_and(|(p, g)| s.tier(p, g).is_none()) {
            s.layout = None;
        }
    }

    /// Whether the chunk holds `primary` of the current generation; asks the
    /// builder otherwise, unless the tier is known not to fit the free slots.
    fn ask(&mut self, i: usize, primary: Zoom) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let lacks = s.tier(primary, generation).is_none();
        let room = !lacks || s.slots_for(primary, generation) <= claimable(&self.meshes, pool_of(s.chunk, primary));
        self.builder.want(s.id, [(lacks && room).then_some(primary), None]);
        settle(&mut self.meshes, &self.builder);
        if !room {
            self.slots[i].fresh = true;
        }
        !lacks
    }

    fn apply_layout(&mut self, i: usize, primary: Zoom, generation: u32) {
        self.counts.layouts += 1;
        let s = &mut self.slots[i];
        s.layout = Some((primary, generation));
        match s.tier(primary, generation).and_then(|t| t.bounds) {
            Some(b) => (s.bc, s.radius) = (b.center(), b.radius()),
            None => s.unmeasured(),
        }
    }

    fn trim(&mut self, i: usize) {
        let layout = self.slots[i].layout;
        self.release_tiers(i, move |cell_zoom, generation| layout == Some((cell_zoom, generation)));
    }

    fn commit(&mut self, i: usize) -> bool {
        let s = &self.slots[i];
        if !s.pending() {
            return false;
        }
        let (p, generation) = (s.target(), self.generation(i));
        let asked = self.ask(i, p);
        if !asked {
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
}

fn same_view(a: &Frame, b: &Frame) -> bool {
    a.anchor == b.anchor && a.frustum == b.frustum && a.camera == b.camera
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
    // Cull where the renderer draws it, on the globe; a flat-world cull leaves
    // holes at the horizon.
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

/// A slot holds at most the drawn tier and the one just built.
const HELD: usize = 2;

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

fn chained(meshes: &Meshes, at: usize, n: u32) -> Option<usize> {
    (0..n).try_fold(at, |at, _| meshes.next[at].get())
}

struct Landing {
    id: u32,
    generation: u32,
    cell_zoom: Zoom,
    first: usize,
    slots: u32,
    /// Slots of the tier it is rebuilt over, which it writes over.
    over: u32,
    bounds: Option<Aabb>,
}

/// Holds every slot a tier of `boxes` fills at once, or records the need and
/// refuses; a rebuild reuses the old tier's slots and claims only the extra.
fn reserve(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) -> bool {
    let BuildEvent::Reserve { id, generation, cell_zoom, boxes } = e else { return false };
    // One bake is open at a time: a leftover landing is from one that closed
    // without landing.
    if let Some(l) = meshes.landing.take() {
        release(meshes, l);
    }
    let Some(i) = place[id as usize].get() else { return false };
    let s = &slots[i];
    if cell_zoom != s.target() {
        return false;
    }
    let pool = pool_of(s.chunk, cell_zoom);
    let k = boxes.div_ceil(capacity(s.chunk.zoom(), cell_zoom)).max(1);
    let over = s.tiers.iter().find(|t| t.cell_zoom == cell_zoom).map(|t| t.slot.index);
    let taken = match over {
        Some(first) => Some((first, (0..).take_while(|&n| chained(meshes, first, n).is_some()).count() as u32)),
        None => claim(meshes, pool, k, id).map(|first| (first, 0)),
    };
    let Some((first, held)) = taken else {
        return refuse(slots, i, meshes, generation, cell_zoom, k);
    };
    if held > 0 && k > held {
        let last = chained(meshes, first, held - 1).unwrap_or(first);
        let more = claim(meshes, pool, k - held, id);
        match more {
            Some(more) => meshes.next[last] = Nth::of(more),
            None => return refuse(slots, i, meshes, generation, cell_zoom, k),
        }
    }
    meshes.landing = Some(Landing {
        id,
        generation,
        cell_zoom,
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
    if let Some(l) = meshes.landing.take_if(|l| open != Some((l.id, l.cell_zoom, l.generation))) {
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

fn refuse(slots: &mut [Slot], i: usize, meshes: &mut Meshes, generation: u32, cell_zoom: Zoom, k: u32) -> bool {
    slots[i].need = Some((generation, cell_zoom, k));
    slots[i].fresh = true;
    meshes.needs = meshes.needs.wrapping_add(1);
    meshes.waits += 1;
    meshes.wait_zooms |= 1 << GROUND.z_of(cell_zoom);
    false
}

fn part(_slots: &[Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Part {
        id,
        generation,
        cell_zoom,
        at,
        first,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(l) = meshes.landing.as_ref().filter(|l| (l.id, l.generation, l.cell_zoom) == (id, generation, cell_zoom)) else {
        return;
    };
    let Some(_i) = place[id as usize].get() else { return };
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
    let unit = units(cell_zoom);
    let mut bounds = meshes.landing.as_ref().and_then(|l| l.bounds);
    for r in words.chunks_exact(box_words as usize) {
        let (min, size) = box_bounds(r, unit, heights);
        let b = Aabb {
            min,
            max: std::array::from_fn(|a| min[a] + size[a]),
        };
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

fn land(slots: &mut [Slot], place: &[Nth], meshes: &mut Meshes, e: BuildEvent) {
    let BuildEvent::Tier {
        id,
        generation,
        cell_zoom,
        box_cnt,
        heights,
    } = e
    else {
        return;
    };
    let Some(i) = place[id as usize].get() else { return };
    let s = &slots[i];
    if s.tier(cell_zoom, generation).is_some() || cell_zoom != s.target() {
        return;
    }
    let landing = match meshes.landing.take() {
        Some(l) if (l.id, l.generation, l.cell_zoom) == (id, generation, cell_zoom) => l,
        other => {
            meshes.landing = other;
            // An empty mesh reserved nothing; it takes one slot here.
            if box_cnt > 0 {
                return;
            }
            let over = s.tiers.iter().find(|t| t.cell_zoom == cell_zoom).map(|t| t.slot.index);
            let taken = match over {
                Some(first) => Some(first),
                None => claim(meshes, pool_of(s.chunk, cell_zoom), 1, id),
            };
            let Some(first) = taken else {
                refuse(slots, i, meshes, generation, cell_zoom, 1);
                return;
            };
            Landing {
                id,
                generation,
                cell_zoom,
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
    prune(s, meshes, |t| layout == Some((t.cell_zoom, t.generation)), Some(slot.index));
    // An older-generation tier on the same slots now shows these boxes.
    for t in s.tiers.iter_mut().filter(|t| t.slot.index == slot.index) {
        t.box_cnt = box_cnt;
        t.bounds = landing.bounds;
        t.heights = heights;
    }
    s.tiers.push(Tier {
        cell_zoom,
        generation,
        bounds: landing.bounds,
        slot,
        box_cnt,
        heights,
    });
}
