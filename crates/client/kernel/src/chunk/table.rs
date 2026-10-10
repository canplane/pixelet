// pxl-kernel/chunk/table.rs

//! The chunks' table: which chunks are mounted, the role each has in the
//! frontier (frontier.rs), which generation of its tier each shows, and what
//! to build next.
//! Every mounted chunk holds a mesh slot or has one held for it, so the table
//! is never larger than the arena.

mod census;
mod mesh;
mod mount;

pub use census::{Census, ChunkCounts, ChunkHeld};
pub use mesh::Meshes;
pub use mount::MOUNTS_BYTES;

use crate::layer::Query;
use crate::query::World;
use mesh::*;
use mount::*;

use pxl_render::frame::draw::DrawList;
use pxl_render::gpu::arena::{arena_configure, arena_epoch};
use pxl_render::mesh::chunk::Heights;
use pxl_spec::space::coord::globe::Globe;

use crate::profiler::frame::ZOOMS;
use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::{dot3, len3, V3};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::sched::edge::Notify;
use xpute_guest::sched::frame_budget::{run_under_budget, BudgetOpts, PassStats};

use crate::chunk::build::{draws_as_held, BuildEvent, Builder};
use crate::chunk::frontier::{pool_of, Env, Frontier, Now, Slots};
use crate::chunk::mesh_slot::{group_bytes, group_is_fine, slot_at, MeshSlot, Pool, ARENA, OCCUPANCY};
use crate::chunk::source::{compared, draws, written_in};
use crate::layer::object::preview::RenderEdits;
use crate::layer::object::store::EntryAt;
use crate::param;
use crate::setting::store::settings_store;
use crate::Build;
use pxl_spec::space::coord::zoom::{units, Zoom};
use xpute_guest::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::chunk::domain::{self, Domain};
use crate::chunk::Chunk;
use crate::io::demand::Order;
use crate::layer::Base as _;
use pxl_spec::bake::tile::terrain::TerrainTile;

/// `.fixed.mounts`, handed in at boot.
pub static MOUNTS_RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

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
        let x = pxl_spec::space::coord::world::nearest(at[0], self.anchor[0]);
        [x - self.anchor[0], at[1] - self.anchor[1], at[2] - self.anchor[2]]
    }
}

#[derive(Clone, Copy)]
struct Tier {
    generation: u32,
    bounds: Option<Aabb>,
    /// First slot of the chain the tier's boxes fill (`Meshes::next`).
    slot: MeshSlot,
    box_cnt: u32,
    heights: Heights,
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
    /// Slots a tier of a generation needs, found when a bake did not fit
    /// (`BuildEvent::Reserve`); not asked for again until that many are free.
    need: Option<(u32, u32)>,
    /// The layout's tier plus the last one landed. A source change keeps the
    /// old generation's tier drawn until a layout of the new one is applied.
    tiers: Held<Tier>,
    /// The applied layout's generation; none until one is, or once its tier is gone.
    layout: Option<u32>,
    sources: Sources,
}

#[derive(Clone, PartialEq, Eq)]
struct Sources {
    /// A tier above a leaf bakes one color with no texture layer, so the setting
    /// is part of what the tier was built from.
    textured: bool,
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
        let drawn = |at: &EntryAt| draws(k, *at) && draws_as_held(k, at.zoom());
        let world = World::now();
        let column = world.read(k.node(), k.cell_zoom()).value();
        Sources {
            textured: settings.textures,
            hide: edits.hide.filter(|at| edits.hide_in.0 && drawn(at)),
            carve: edits.carve.filter(drawn),
            read_from: column.map(|c| c.ground.from.zoom),
            beside_from: column.map_or([None; 8], |c| c.beside.map(|b| b.map(|s| s.from.zoom))),
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
        let b = World::now().bounds(self.chunk.node(), self.chunk.cell_zoom()).value().flatten().map_or(region, |b| b.aabb);
        let c = b.center();
        self.bc = [c[0] - region.min[0], c[1] - region.min[1], c[2]];
        self.radius = b.radius();
    }

    fn slots_for(&self, generation: u32) -> u32 {
        self.need.filter(|&(g, _)| g == generation).map_or(1, |(_, k)| k)
    }

    fn pending(&self) -> bool {
        self.pending_since.is_some()
    }

    /// Pending with a layout up.
    fn settling(&self) -> bool {
        self.pending() && self.layout.is_some()
    }

    fn tier(&self, generation: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.generation == generation)
    }

    /// Slots held for it and not yet taken: none while it has a tier, whose slots a rebuild takes.
    fn reserves(&self, generation: u32) -> Option<(Pool, u32)> {
        (self.tiers.len() == 0).then(|| (pool_of(self.chunk), self.slots_for(generation)))
    }
}

pub struct Scheduler {
    pub meshes: Meshes,
    zoom_boxes: [u32; ZOOMS],
    pub builder: Builder,
    slots: Vec<Slot>,
    /// Index into `slots` by mount. Nothing naming a mount outlives it (`remove`),
    /// so a freed mount can be reused at once.
    place: &'static mut [Nth],
    mounts: Free,
    index: Index,
    /// Whether a goal was published at the last tick: a recheck before one
    /// decides nothing (`recheck`), so its first one rechecks every chunk.
    domain_seen: bool,
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
        (pool_of(c), self.table.slots_of(c))
    }

    fn buildable(&self, c: Chunk) -> bool {
        TerrainTile::residency().read(c.node(), c.cell_zoom()).value().is_some()
    }

    fn order(&self, c: Chunk) -> Order {
        self.d.order_of(c)
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
                fine_peak: 0,
                coarse_peak: 0,
                waits: 0,
                wait_zooms: 0,
            },
            builder: Builder::default(),
            slots: Vec::new(),
            place,
            mounts: Free { lane: free, len: OCCUPANCY },
            index: Index { lane: index, len: 0 },
            domain_seen: false,
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
            changes: 0,
            changes_peak: 0,
        }
    }

    pub fn slots_of(&self, c: Chunk) -> u32 {
        self.find(&c).map_or(1, |i| self.slots[i].slots_for(self.generation(i)))
    }

    pub fn pace(&self) -> [u32; 2] {
        [self.refined, self.changes_peak]
    }

    /// Each mounted chunk, whether it is drawn, and whether it is in the frustum.
    pub fn mounted(&self, out: &mut Vec<(Chunk, bool, bool)>) {
        out.clear();
        out.extend(self.slots.iter().map(|s| (s.chunk, s.role == Role::Drawn, s.in_frustum)));
    }

    fn drawn_slot(&self, p: &Pvf3) -> Option<&Slot> {
        self.slots
            .iter()
            .filter(|s| s.role == Role::Drawn && s.chunk.named() == Chunk::holding(s.chunk.zoom(), p))
            .max_by_key(|s| s.chunk.zoom())
    }

    /// The cells the finest drawn chunk holding `p` is drawn in.
    pub fn cells_at(&self, p: &Pvf3) -> Option<Zoom> {
        let s = self.drawn_slot(p)?;
        s.layout.map(|_| s.chunk.cell_zoom())
    }

    /// Whether the last frame drew any ground on screen.
    pub fn shows_ground(&self) -> bool {
        self.zoom_boxes.iter().any(|&n| n > 0)
    }

    /// Each mounted chunk a tier of fills more than one mesh slot, and how
    /// many, sorted: what the walk prices it at (`slots_of`).
    pub fn priced(&self, out: &mut Vec<(Chunk, u32)>) {
        out.clear();
        out.extend(self.slots.iter().enumerate().map(|(i, s)| (s.chunk, s.slots_for(self.generation(i)))).filter(|&(_, k)| k != 1));
        out.sort_unstable();
    }

    fn add(&mut self, k: Chunk) {
        // The frontier starts no move without room, so running out of mounts is a bug.
        let id = self.mounts.pop().or_bug(Errno::ENOSPC);
        let edits = crate::chunk::source::previewed();
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
            tiers: Held::EMPTY,
            layout: None,
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
        self.recheck(i, xpute_guest::clock::now());
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
        let edits = crate::chunk::source::previewed();
        for i in 0..self.slots.len() {
            let k = self.slots[i].chunk;
            let sources = Sources::of(k, edits);
            if written_in(k) || sources != self.slots[i].sources {
                self.slots[i].sources = sources;
                self.source(i);
            }
        }
        compared();
    }

    /// The build task's step. Returns whether any build did work.
    pub fn build_step(&mut self, build: &mut Build, budget_ms: f64) -> bool {
        let (slots, place, meshes, landed) = (&mut self.slots, &*self.place, &mut self.meshes, &mut self.landed);
        landed.clear();
        let worked = self.builder.tick(build, &self.builds, budget_ms, &xpute_guest::clock::now, &mut |e| match e {
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
        for s in self.slots.iter() {
            match s.role {
                Role::Drawn => sc.drawn.push(s.chunk),
                Role::Building => sc.building.push((s.chunk, s.layout.is_some())),
            }
        }
        sc.drawn.sort_unstable();
        sc.building.sort_unstable_by_key(|&(c, _)| c);
        let now = Now {
            ms: xpute_guest::clock::now(),
            goal: d.pieces(),
            drawn: &sc.drawn,
            building: &sc.building,
            free: self.free(),
        };
        let plan = sc.frontier.plan(&now, &Pieces { d, table: self });
        self.moves = (plan.refused, plan.waiting + plan.held);
        self.refined = self.refined.wrapping_add(plan.refined);
        self.changes_peak = self.changes_peak.max(self.changes);
        self.changes = plan.swaps().count() as u32;

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
        for &k in &plan.start {
            xpute_kit::ensure!(self.find(&k).is_none(), ENOTRECOVERABLE);
            self.add(k);
            self.counts.added += 1;
        }
    }

    /// The arena was recreated with the device: forget every tier. A layout kept
    /// ready over lost tiers would draw nothing until its sources changed.
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

    /// One frame's plan, before anything is drawn. Returns whether the
    /// chunks have something new to draw.
    pub fn tick(&mut self, frame: Frame, now: &impl Fn() -> f64) -> bool {
        let t0 = now();
        let deadline = t0 + frame.remaining_ms;
        let hard = deadline + frame.hard_slack_ms;
        if !self.frame.is_some_and(|prev| same_view(&prev, &frame)) {
            self.view_dirty = true;
        }
        self.frame = Some(frame);
        let domain = domain::current().is_some();
        if domain != self.domain_seen {
            self.domain_seen = domain;
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
        self.commit_left = commit_left;
        let moving = self.moves.1 > 0 || self.slots.iter().any(|s| s.role == Role::Building && s.layout.is_some());
        committed > 0 || pending_total > committed || moving || self.view_dirty
    }

    /// The chunks drawn as this turn's plan found them.
    pub fn drawn(&self) -> &[Chunk] {
        &self.scratch.drawn
    }

    /// Chunks built for a move are not drawn until the move is made; the piece
    /// they replace covers their ground until then.
    pub fn draw(&mut self, frame: &Frame, draw: &mut DrawList) {
        self.zoom_boxes = [0; ZOOMS];
        let holder = &self.meshes.holder;
        for s in &self.slots {
            let Some(generation) = s.layout else { continue };
            if s.role == Role::Building || !s.in_frustum {
                continue;
            }
            let (x0, y0) = s.base();
            let translation = DrawList::translation([pxl_spec::space::coord::world::nearest(x0, frame.eye[0]), y0, 0.0], frame.eye);
            let Some(t) = s.tier(generation) else { continue };
            // Slots are recycled between chunks, so a chunk must draw only from a slot
            // it holds; otherwise it can draw another chunk's ground.
            xpute_kit::ensure!(holder[t.slot.index] == Nth::of(s.id as usize), ENOTRECOVERABLE);
            if t.box_cnt == 0 {
                continue;
            }
            // Mesh heights are steps from the tier's floor, so the floor goes in the translation.
            let at = [translation[0], translation[1], translation[2] + t.heights.floor as f32];
            let (unit, z_step) = (units(s.chunk.cell_zoom()) as f32, t.heights.step as f32);
            let (mut left, mut slot) = (t.box_cnt, Some(t.slot.index));
            while let (Some(at_slot), true) = (slot, left > 0) {
                let m = slot_at(at_slot);
                let n = left.min(m.capacity);
                draw.draw_arena(m.group, m.first_box, n, at, unit, z_step, group_is_fine(m.group));
                left -= n;
                slot = self.meshes.next[at_slot].get();
            }
            self.zoom_boxes[GROUND.tile_zoom(s.chunk.cell_zoom()) as usize] += t.box_cnt;
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
        self.builder.want(self.slots[i].id, false);
        settle(&mut self.meshes, &self.builder);
    }

    /// Sets whether the chunk is pending: its layout is not of the current generation.
    fn recheck(&mut self, i: usize, now: f64) {
        self.slots[i].fresh = false;
        if domain::current().is_none() {
            return;
        }
        let current = self.generation(i);
        let pending = self.slots[i].layout != Some(current);
        self.set_pending(i, pending, now);
        let s = &mut self.slots[i];
        if !s.pending() {
            self.stop_work(i);
        }
    }

    fn release_tiers(&mut self, i: usize, keep: impl Fn(u32) -> bool) {
        let s = &mut self.slots[i];
        prune(s, &mut self.meshes, |t| keep(t.generation), None);
        if s.layout.is_some_and(|g| s.tier(g).is_none()) {
            s.layout = None;
        }
    }

    /// Whether the chunk holds its tier of the current generation; asks the
    /// builder otherwise, unless the tier is known not to fit the free slots.
    fn ask(&mut self, i: usize) -> bool {
        let generation = self.generation(i);
        let s = &self.slots[i];
        let lacks = s.tier(generation).is_none();
        let room = !lacks || s.slots_for(generation) <= claimable(&self.meshes, pool_of(s.chunk));
        self.builder.want(s.id, lacks && room);
        settle(&mut self.meshes, &self.builder);
        if !room {
            self.slots[i].fresh = true;
        }
        !lacks
    }

    fn apply_layout(&mut self, i: usize, generation: u32) {
        self.counts.layouts += 1;
        let s = &mut self.slots[i];
        s.layout = Some(generation);
        match s.tier(generation).and_then(|t| t.bounds) {
            Some(b) => (s.bc, s.radius) = (b.center(), b.radius()),
            None => s.unmeasured(),
        }
    }

    fn trim(&mut self, i: usize) {
        let layout = self.slots[i].layout;
        self.release_tiers(i, move |generation| layout == Some(generation));
    }

    fn commit(&mut self, i: usize) -> bool {
        let s = &self.slots[i];
        if !s.pending() {
            return false;
        }
        let generation = self.generation(i);
        if !self.ask(i) {
            return false;
        }
        self.apply_layout(i, generation);
        self.trim(i);
        self.set_pending(i, false, 0.0);
        true
    }
}
