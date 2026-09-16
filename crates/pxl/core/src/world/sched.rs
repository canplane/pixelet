//! The chunks' scheduler: which chunks are on screen, which tier each shows,
//! what to build and in what order, and what to give back — what
//! module/world/chunk_stage.tsx's visible pass, chunk_node.ts's recheck and
//! commit, sched/chunk_table.ts and sched/priority.ts did, in the core.
//!
//! The glue says which chunks exist and what they are made of, writes the
//! frame — the camera, the anchor root's matrix, the frustum — where the
//! core reads it, and calls the tick. What comes back is events: a chunk's
//! group shown or hidden, the layout it should show (its tiers), how the two
//! tiers of a blend are weighted, a tier's mesh handed over and a tier to
//! let go. The glue holds the meshes and the scene; the core holds every
//! decision about them. Why each rule is what it is lives in the TypeScript
//! it came from; the history there is the reasoning here.

use core::fmt::Write;

use crate::bake::terrain::ChunkSample;
use std::collections::BTreeMap;
use crate::render::draw::{DrawList, PIPELINE_CHUNK};
use crate::space::constant::{CHUNK_SPAN, CHUNK_WORLD_SPAN, PAGE_CHUNK_AXIS, PAGE_SHAMT, PAGE_SPAN};
use crate::world::billboard::Billboards;
use crate::world::build::{Builder, Event as BuildEvent, PageLanes, World, MAX_CHUNKS};
use crate::world::debug::DebugLines;
use crate::world::decor::{decor_groups, density as decor_density, Chunk as DecorChunk, DecorBuild, DecorScratch, Kind as DecorKind, Rule as DecorRule};
use crate::world::environment::Environment;
use crate::world::ghost::Ghost;
use crate::world::lod::{Ladder, COARSEST, LOD_STEPS};
use crate::world::marker::Markers;
use crate::world::model::Models;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};

/// The sphere a chunk nothing has measured is culled by.
const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;
/// How often every chunk is rechecked, whatever is moving.
const RECHECK_INTERVAL_S: f64 = 0.15;
/// Layouts applied per frame on screen, and releases off it — more of the
/// latter after a frame that had a block refused: a release is cheap, and
/// what an off-screen chunk still shows is what the refused block needs
/// room among. A zoom-in leaves a thousand chunks showing their last
/// layout off screen; at two a frame that is eight seconds of the fine
/// tiers finding no room (2026-09-14).
const MAX_COMMITS_PER_FRAME: u32 = 8;
const MAX_RELEASES_PER_FRAME: u32 = 2;
const MAX_RELEASES_WHEN_REFUSED: u32 = 64;
/// Off-screen chunks that may give their built work back in one frame, and
/// how many more when a block was refused this frame: a refusal is a build
/// on screen going nowhere, and the pool has no spill to fall back on.
const MAX_RECLAIMS_PER_FRAME: u32 = 16;
const MAX_RECLAIMS_WHEN_REFUSED: u32 = 64;
/// Chunks mounted in one frame: a page comes into range as 256, and the
/// glue makes a node for each, which is a frame's worth of work.
const MAX_MOUNTS_PER_FRAME: u32 = 64;
/// The mesh section's live share past which off-screen chunks are reclaimed
/// whatever the retain budget says.
const MESH_PRESSURE: f64 = 0.9;
/// Tiers one chunk may keep: the active pair, the off-screen tier, and one
/// of hysteresis.
const MAX_CACHED_TIERS: usize = 4;
/// A wait halves a chunk's distance key every this many milliseconds.
const AGE_HALF_MS: f64 = 4000.0;
/// The byte budget, paid in LOD: when the pool refuses a block and reclaim
/// found nothing to give back, the ladder is pulled in a step (lod.rs's
/// pressure) and the set on screen gets coarser until it fits; a step at
/// most this often, so the coarser layouts have time to land and free
/// what they replace before the next is taken, and up to three tiers. A
/// step is let out again after this long with no refusal and the section
/// off its pressure mark. Seen on the desktop, 2026-09-14: a zoom through
/// a city left 1,091 chunks pending on tiers the section could not hold,
/// nothing off screen to reclaim, for the rest of the session.
const MAX_PRESSURE: u32 = 6;
const PRESSURE_STEP_MS: f64 = 500.0;
const PRESSURE_RELIEF_MS: f64 = 5000.0;

pub const IN_FRUSTUM: u8 = 1;
pub const EMPTY: u8 = 2;
pub const PENDING: u8 = 4;
pub const BLEND_ACTIVE: u8 = 16;
pub const SETTLING: u8 = 32;

/// A release, as a request: off screen, nothing built.
const RELEASE_REQUEST: i32 = -1;

fn build_request(primary: u32, secondary: u32) -> i32 {
    ((primary << 8) | secondary) as i32
}

/// What the glue writes before a tick.
#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The anchor root's world matrix, column-major.
    pub root: [f64; 16],
    /// The map origin, in units.
    pub origin: [f64; 3],
    /// The frustum as six planes of (nx, ny, nz, constant).
    pub planes: [f64; 24],
    pub camera: [f64; 3],
    /// Where the camera is heading.
    pub lead: [f64; 3],
    pub viewport_h: f64,
    pub fov_deg: f64,
    pub delta_s: f64,
    pub budget_ms: f64,
    pub hard_slack_ms: f64,
    pub pixel_dither: bool,
    pub retain_budget_bytes: f64,
    /// The range anchor — the map centre's chunk, in units — when there is
    /// one, the radius chunks are mounted within, and how much further out a
    /// mounted chunk is kept.
    pub anchor: Option<[f64; 2]>,
    pub render_dist: f64,
    pub range_hysteresis: f64,
    /// Whether procedural decor is scattered at all (the setting).
    pub show_decor: bool,
}

impl Frame {
    /// Where `at`, in the map's units, is in the world the camera is
    /// written in: against the origin, then through the anchor root.
    pub fn world_of(&self, at: [f64; 3]) -> [f64; 3] {
        let m = &self.root;
        let (lx, ly, lz) = (at[0] - self.origin[0], at[1] - self.origin[1], at[2] - self.origin[2]);
        [
            m[0] * lx + m[4] * ly + m[8] * lz + m[12],
            m[1] * lx + m[5] * ly + m[9] * lz + m[13],
            m[2] * lx + m[6] * ly + m[10] * lz + m[14],
        ]
    }
}

/// What the glue is told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Build(BuildEvent),
    /// A chunk came into range: its page and its page-local base, in units.
    Mount {
        id: u32,
        page_idx: IVec,
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
    /// The layout to show: its tiers, all of them already handed over.
    /// `ready` false is the release: nothing shown.
    Layout {
        id: u32,
        primary: u32,
        secondary: u32,
        ready: bool,
    },
    /// How the layout's two tiers are weighted, and which one decor takes.
    Blend {
        id: u32,
        primary: f32,
        secondary: f32,
        decor_secondary: bool,
    },
    /// Let go of a tier the glue holds, and the decor at its step; the
    /// layout keeps what it shows.
    Evict {
        id: u32,
        step: u32,
    },
    /// The chunk's decor at a tier, as decor.rs's block of `bytes` at `at`,
    /// the glue's to release; every tier of a layout has its decor handed
    /// over before the layout.
    Decor {
        id: u32,
        step: u32,
        at: u32,
        bytes: u32,
    },
}

/// Pages with a terrain tile at once: the resident pages, and room past them.
const MAX_PAGES: usize = 1024;

#[derive(Clone, Copy, Default)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy, Default)]
struct Tier {
    step: u32,
    bytes: u32,
    bounds: Option<Bounds>,
    /// The renderer's name for the block, and the counts its lanes are
    /// read by.
    handle: u32,
    vertex_cnt: u32,
    index_cnt: u32,
}

/// A decor block the glue holds: its step and bytes, and the renderer's
/// handle. Each group's lanes are where the block's own header says.
#[derive(Clone, Copy, Default)]
pub struct Decor {
    step: u32,
    /// Where the block is in the memory, and its size.
    pub at: u32,
    pub bytes: u32,
    handle: u32,
}

impl Decor {
    /// Each group's first lane word in the block and its count
    /// (decor.rs's decor_groups), the block read in `mem`.
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
    base_x: f64,
    base_y: f64,
    stable_hash: f64,
    /// The bounding sphere in the chunk's own space.
    bc: [f64; 3],
    radius: f64,
    distance2: f64,
    lead_distance2: f64,
    flags: u8,
    pending_since: f64,
    request: i32,
    runs_bytes: u32,
    /// The tiers the glue holds, oldest built first.
    tiers: Vec<Tier>,
    /// The decor blocks the glue holds.
    decor: Vec<Decor>,
    layout_primary: u32,
    layout_secondary: u32,
    layout_ready: bool,
    desired: (u32, u32),
    /// The last blend told, so an unchanged one is not resent.
    told: Option<(f32, f32, bool)>,
    /// The heap's release count when its decor was last refused a
    /// block: not committed again until a block is given back.
    refused: Option<u32>,
}

impl Slot {
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

    fn tier(&self, step: u32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.step == step)
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
    /// The hover ghost (ghost.rs).
    pub ghost: Ghost,
    /// Models and the actors that play them (model.rs).
    pub models: Models,
    /// The location puck (marker.rs).
    pub markers: Markers,
    /// The billboards (billboard.rs).
    pub billboards: Billboards,
    /// The model each decor group is drawn with, by group, once the glue
    /// registered it: what a hit test reads the triangles of.
    pub decor_models: Vec<Option<u32>>,
    /// The sun, the moon, the light and the sky (environment.rs).
    pub environment: Environment,
    /// The dev gizmos (debug.rs).
    pub debug: DebugLines,
    slots: Vec<Slot>,
    index: BTreeMap<u32, usize>,
    /// The chunk at a base, in units, for the raycast's walk.
    by_base: BTreeMap<(i64, i64), u32>,
    ladder: Ladder,
    recheck_elapsed: f64,
    frame: Option<Frame>,
    /// Something changed that a recheck has not seen: the view moved, a
    /// chunk came or went, a source was told. The canvas is asked for
    /// frames until a full recheck has run over it, or the world stops at
    /// whatever the last frame left — nothing else asks for a frame now
    /// that chunks are not React children.
    view_dirty: bool,
    retained_on: f64,
    retained_off: f64,
    pub census: Census,
    /// The pages with a terrain tile, which chunks are mounted from. In
    /// ascending order, which is the order chunks are mounted in.
    page_idxs: Vec<IVec>,
    page_idxs_revision: u32,
    /// The chunks the module mounted, by page and page-local base.
    mounted: BTreeMap<(IVec, u32, u32), u32>,
    /// The mounted chunks a pass finds leaving, before they are let go.
    gone: Vec<((IVec, u32, u32), u32)>,
    next_mount_id: u32,
    mounted_for: Option<([f64; 2], f64, f64, u32)>,
    /// The decor kinds, in the order the glue taught them — the order of
    /// the groups in every decor block.
    decor_kinds: Vec<DecorKind>,
    decor_sample: ChunkSample,
    /// Where a decor build gathers its instances, made at the first build:
    /// the scheduler is made before the sections are.
    decor_scratch: Option<DecorScratch>,
    /// The builder's refusals as of the last reclaim, so a refusal since
    /// is pressure now; and whether the last pass saw one, for the next
    /// frame's release cap.
    refused_seen: u32,
    refused_last: bool,
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
    still: Vec<u32>,
    landed: Vec<u32>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new()
    }
}

/// Runs `step` over `items` under a budget: the first item always, then
/// until `cap` items did something or the budget is spent, and never past a
/// hard deadline once one item was tried. How many did something.
fn under_budget<T: Copy>(items: &[T], cap: u32, deadline: f64, hard: f64, now: &impl Fn() -> f64, mut step: impl FnMut(T) -> bool) -> u32 {
    let mut ran = 0;
    for (visited, &id) in items.iter().enumerate() {
        let t = now();
        if visited > 0 && t >= hard {
            break;
        }
        if ran > 0 && (ran >= cap || t >= deadline) {
            break;
        }
        if step(id) {
            ran += 1;
        }
    }
    ran
}

/// An evict for every step in `steps`, a set of the steps as bits — each
/// is a power of two (LOD_STEPS) — finest first.
fn emit_evicts(id: u32, steps: u32, emit: &mut impl FnMut(Event)) {
    for &step in &LOD_STEPS {
        if steps & step != 0 {
            emit(Event::Evict { id, step });
        }
    }
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            builder: Builder::new(),
            draw: DrawList::default(),
            ghost: Ghost::default(),
            models: Models::default(),
            markers: Markers::default(),
            billboards: Billboards::default(),
            decor_models: Vec::new(),
            environment: Environment::default(),
            debug: DebugLines::default(),
            slots: Vec::with_capacity(MAX_CHUNKS),
            index: BTreeMap::new(),
            by_base: BTreeMap::new(),
            ladder: Ladder::default(),
            recheck_elapsed: RECHECK_INTERVAL_S,
            frame: None,
            view_dirty: true,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            page_idxs: Vec::with_capacity(MAX_PAGES),
            page_idxs_revision: 0,
            mounted: BTreeMap::new(),
            gone: Vec::with_capacity(MAX_CHUNKS),
            next_mount_id: MOUNT_ID_BASE,
            mounted_for: None,
            decor_kinds: Vec::new(),
            decor_sample: ChunkSample::default(),
            decor_scratch: None,
            refused_seen: 0,
            scratch: Scratch::default(),
            refused_last: false,
            pressure_at: f64::NEG_INFINITY,
            refused_at: f64::NEG_INFINITY,
        }
    }

    /// Teaches a decor kind; its index. Its rules start as `Any` on every
    /// channel, and `decor_rule` sets them.
    pub fn decor_kind(&mut self, kind: DecorKind) -> u32 {
        self.decor_kinds.push(kind);
        (self.decor_kinds.len() - 1) as u32
    }

    /// Forgets every kind taught, for a glue that teaches them again.
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
    pub fn add(&mut self, id: u32, base: IVec, base_x: f64, base_y: f64, tx0: u32, ty0: u32, stable_hash: u32) {
        self.builder.add(id, base, tx0, ty0);
        let mut slot = Slot {
            id,
            base_x,
            base_y,
            stable_hash: stable_hash as f64 / 4_294_967_296.0,
            bc: [CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0.0],
            radius: APPROXIMATE_CHUNK_RADIUS,
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
            layout_ready: false,
            desired: (COARSEST, 0),
            told: None,
            refused: None,
        };
        if let Some(frame) = self.frame {
            let (visible, _, d2, _) = measure(&frame, &slot);
            if visible {
                slot.layout_primary = self.ladder.guide(d2.sqrt(), COARSEST).step;
                slot.desired = (slot.layout_primary, 0);
            }
        }
        self.index.insert(id, self.slots.len());
        self.by_base.insert((base_x as i64, base_y as i64), id);
        assert!(self.slots.len() < MAX_CHUNKS, "more than {MAX_CHUNKS} chunks held");
        self.slots.push(slot);
        self.view_dirty = true;
    }

    /// The chunk whose base is (x, y), in units.
    pub fn chunk_at(&self, x: i64, y: i64) -> Option<u32> {
        self.by_base.get(&(x, y)).copied()
    }

    pub fn remove(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        let Some(i) = self.index.remove(&id) else { return };
        self.view_dirty = true;
        self.by_base.remove(&(self.slots[i].base_x as i64, self.slots[i].base_y as i64));
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

    /// The chunk's sources changed. The tiers and decor the glue holds are
    /// stale and it lets them go itself; what is on screen stays until the
    /// rebuild lands.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool, emit: &mut impl FnMut(Event)) {
        let Some(&i) = self.index.get(&id) else { return };
        self.view_dirty = true;
        let before = self.slots[i].retained();
        self.builder.source(world, id, page, show_structures, &mut |e| emit(Event::Build(e)));
        let s = &mut self.slots[i];
        for t in s.tiers.iter() {
            self.draw.free(t.handle);
        }
        for d in s.decor.iter() {
            self.draw.free(d.handle);
        }
        s.tiers.clear();
        s.decor.clear();
        s.runs_bytes = 0;
        if !s.layout_ready {
            s.bc = [CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0.0];
            s.radius = APPROXIMATE_CHUNK_RADIUS;
        }
        self.account(i, before);
    }

    /// Rebuilds the layout the chunk shows, to the end, now: a placement
    /// shows in the frame it was made. Unless a block was refused, when the
    /// layout is marked not ready and lands on a later tick.
    pub fn build_now(&mut self, world: &mut World, id: u32, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) {
        let Some(&i) = self.index.get(&id) else { return };
        if !self.slots[i].layout_ready {
            return;
        }
        let (p, s2) = (self.slots[i].layout_primary, self.slots[i].layout_secondary);
        if !self.ask(world, i, p, s2) {
            let mem = world_mem(world);
            let (slots, index, ron, roff, draw) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off, &mut self.draw);
            self.builder.tick(world, &[id], f64::INFINITY, now, &mut |e| self_record(slots, index, mem, ron, roff, draw, e, emit));
        }
        if self.ask(world, i, p, s2) && self.ensure_decor(world, i, [p, s2], emit) {
            self.trim(i, [p, s2], emit);
            self.apply_layout(i, p, s2, true, emit);
        } else {
            let s = &mut self.slots[i];
            s.layout_ready = false;
            s.set(EMPTY, true);
        }
    }

    /// Only the chunk's decor changed (an entry placed or hidden, the
    /// setting): the decor the glue holds is stale and it let it go; the
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
        let (p, s2) = (self.slots[i].layout_primary, self.slots[i].layout_secondary);
        if self.ensure_decor(world, i, [p, s2], emit) {
            self.apply_layout(i, p, s2, true, emit);
        } else {
            let s = &mut self.slots[i];
            s.layout_ready = false;
            s.set(EMPTY, true);
        }
    }

    /// Whether the glue holds the decor of every tier in `steps`, building
    /// and handing over what it lacks: false when the heap refused a block.
    /// A chunk with no page has no decor to build.
    fn ensure_decor(&mut self, world: &mut World, i: usize, steps: [u32; 2], emit: &mut impl FnMut(Event)) -> bool {
        let id = self.slots[i].id;
        let Some((base, col_idx, tx0, ty0, page, show_structures)) = self.builder.source_of(id) else { return true };
        let show_decor = self.frame.is_some_and(|f| f.show_decor);
        let (col_x, col_y) = (ivec::X(base), ivec::Y(base));
        let page_idx = ivec::gather(col_x >> PAGE_SHAMT, col_y >> PAGE_SHAMT, 0);
        let mut sampled = false;
        if self.decor_scratch.is_none() {
            self.decor_scratch = DecorScratch::new();
            if self.decor_scratch.is_none() {
                self.builder.stat.starved += 1;
                self.slots[i].refused = Some(crate::host::heap_releases());
                return false;
            }
        }
        for step in steps {
            if step == 0 || self.slots[i].decor.iter().any(|d| d.step == step) {
                continue;
            }
            // SAFETY: the chunk's page, as its source named it, resident
            // while the chunk is.
            let a = crate::probe::mark();
            let view = unsafe { Builder::page_view(world, page) };
            crate::probe::add(17, a);
            // The elevations are the scatter's; a tier no kind is drawn on,
            // or decor switched off, stands only the placed entries, which
            // carry their own height.
            let scatters = show_decor && self.decor_kinds.iter().any(|k| k.density > 0.0 && decor_density(k.extent, step) > 0.0);
            if scatters && !sampled {
                self.decor_sample.read(&view, world.materials, tx0, ty0, show_structures);
                sampled = true;
            }
            let chunk = DecorChunk {
                page: &view,
                page_idx,
                col_idx,
                col_x,
                col_y,
                tx0,
                ty0,
                show_decor,
                hide: self.builder.hide_in(col_idx),
            };
            let a = crate::probe::mark();
            let Some(scratch) = self.decor_scratch.as_mut() else { return false };
            let built = DecorBuild::build(scratch, &self.decor_kinds, &chunk, &self.decor_sample, world.store, step);
            let bytes = built.bytes();
            let a = crate::probe::add(18, a);
            let Some(at) = crate::bake::volume::take_bytes(bytes as usize) else {
                self.builder.stat.starved += 1;
                self.slots[i].refused = Some(crate::host::heap_releases());
                return false;
            };
            // SAFETY: a block of the bytes the build asked for, the glue's
            // once handed over.
            unsafe { built.write(world.volumes.mem().add(at)) };
            let a = crate::probe::add(19, a);
            let before = self.slots[i].retained();
            let handle = self.draw.upload(at as u32, bytes, 0, 0);
            let a = crate::probe::add(24, a);
            self.slots[i].decor.push(Decor {
                step,
                at: at as u32,
                bytes,
                handle,
            });
            self.account(i, before);
            let a = crate::probe::add(25, a);
            emit(Event::Decor { id, step, at: at as u32, bytes });
            crate::probe::add(26, a);
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
            s.decor.iter().find(|d| d.step == decor_step).map(|d| (s.id, s.base_x, s.base_y, d))
        })
    }

    /// A page's terrain tile arrived, or left.
    pub fn page(&mut self, page_idx: IVec, present: bool) {
        let changed = match (self.page_idxs.binary_search(&page_idx), present) {
            (Err(at), true) => {
                assert!(self.page_idxs.len() < MAX_PAGES, "more than {MAX_PAGES} pages with a tile");
                self.page_idxs.insert(at, page_idx);
                true
            }
            (Ok(at), false) => {
                self.page_idxs.remove(at);
                true
            }
            _ => false,
        };
        if changed {
            self.page_idxs_revision = self.page_idxs_revision.wrapping_add(1);
            self.view_dirty = true;
        }
    }

    /// Decor instances the build's lanes had no room for, since the kernel
    /// started: the ledger's, so a chunk short of its decor says so.
    pub fn decor_dropped(&self) -> u32 {
        self.decor_scratch.as_ref().map_or(0, |s| s.dropped)
    }

    /// Whether a chunk of `page_idx` is still mounted: what reads its lanes.
    pub fn page_mounted(&self, page_idx: IVec) -> bool {
        self.mounted.range((page_idx, 0, 0)..=(page_idx, u32::MAX, u32::MAX)).next().is_some()
    }

    /// module/world/frame/range.ts's rule, a page and a chunk at a time: a chunk
    /// whose square is within the radius of the anchor is mounted, and one
    /// already mounted is kept until it is past the radius and the
    /// hysteresis; a page with no tile, or out past both, mounts nothing.
    fn mount(&mut self, world: &mut World, frame: &Frame, emit: &mut impl FnMut(Event)) {
        let Some(anchor) = frame.anchor else { return };
        let key = (anchor, frame.render_dist, frame.range_hysteresis, self.page_idxs_revision);
        if self.mounted_for == Some(key) {
            return;
        }
        let mut mounts = 0;
        let (r2, keep2) = (frame.render_dist.powi(2), (frame.render_dist + frame.range_hysteresis).powi(2));
        let rect = |x0: f64, y0: f64, span: f64| {
            let dx = if 0.0 < x0 {
                x0
            } else if 0.0 > x0 + span {
                -(x0 + span)
            } else {
                0.0
            };
            let dy = if 0.0 < y0 {
                y0
            } else if 0.0 > y0 + span {
                -(y0 + span)
            } else {
                0.0
            };
            dx * dx + dy * dy
        };
        let page_origin = |page_idx: IVec| {
            let (px, py) = (ivec::X(page_idx), ivec::Y(page_idx));
            ((px << PAGE_SHAMT) as f64 - anchor[0], (py << PAGE_SHAMT) as f64 - anchor[1])
        };

        // Leave: every mounted chunk past the keep radius or on a page_idx that left.
        self.gone.clear();
        for (&(page_idx, bx, by), &id) in self.mounted.iter() {
            let leaving = self.page_idxs.binary_search(&page_idx).is_err() || {
                let (x0, y0) = page_origin(page_idx);
                rect(x0 + bx as f64, y0 + by as f64, CHUNK_WORLD_SPAN) > keep2
            };
            if leaving {
                self.gone.push(((page_idx, bx, by), id));
            }
        }
        for g in 0..self.gone.len() {
            let (k, id) = self.gone[g];
            self.mounted.remove(&k);
            self.remove(world, id, emit);
            emit(Event::Unmount { id });
        }

        // Enter: every chunk within the radius on a page_idx with a tile.
        for p in 0..self.page_idxs.len() {
            let page_idx = self.page_idxs[p];
            let (x0, y0) = page_origin(page_idx);
            if rect(x0, y0, PAGE_SPAN) > keep2 {
                continue;
            }
            for cy in 0..PAGE_CHUNK_AXIS {
                for cx in 0..PAGE_CHUNK_AXIS {
                    let (bx, by) = (cx * CHUNK_SPAN, cy * CHUNK_SPAN);
                    if self.mounted.contains_key(&(page_idx, bx, by)) || rect(x0 + bx as f64, y0 + by as f64, CHUNK_WORLD_SPAN) > r2 {
                        continue;
                    }
                    if mounts >= MAX_MOUNTS_PER_FRAME {
                        // The rest next frame: the key stays unrecorded.
                        return;
                    }
                    if self.slots.len() >= MAX_CHUNKS {
                        // How many chunks a range holds is the viewport's and the
                        // anchor's, not a number the kernel hands itself: at the
                        // ceiling the pass mounts no more and leaves the key
                        // unrecorded, as it does for the frame's mount budget, so
                        // the next pass tries again. A trap here would take the
                        // world down over a tall window.
                        return;
                    }
                    mounts += 1;
                    let id = self.next_mount_id;
                    self.next_mount_id = self.next_mount_id.wrapping_add(1).max(MOUNT_ID_BASE);
                    let (px, py) = (ivec::X(page_idx), ivec::Y(page_idx));
                    let (wx, wy) = ((px << PAGE_SHAMT) + bx as i64, (py << PAGE_SHAMT) + by as i64);
                    let base = ivec::gather(wx, wy, 0);
                    emit(Event::Mount { id, page_idx, base_x: bx, base_y: by });
                    self.add(id, base, wx as f64, wy as f64, bx, by, blend_hash(page_idx, bx, by));
                    self.mounted.insert((page_idx, bx, by), id);
                }
            }
        }
        self.mounted_for = Some(key);
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, commit, build and reclaim
    /// under the frame's budget. Whether the canvas has something new to draw.
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

        let a = crate::probe::mark();
        self.mount(world, &frame, emit);
        let a = crate::probe::add(0, a);
        self.refresh(&frame, emit);
        let a = crate::probe::add(1, a);
        let mut sc = core::mem::take(&mut self.scratch);
        self.order(t0, &mut sc.order, &mut sc.keys);
        let a = crate::probe::add(2, a);
        let Scratch {
            order,
            visible,
            offscreen,
            eligible,
            still,
            landed,
            ..
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

        let a = crate::probe::add(3, a);
        let pending_total = (visible.len() + offscreen.len()) as u32;
        // A commit on screen may build decor, and a block refused ends the
        // commits this tick as it ends the builds (Builder::tick_capped):
        // the pool is the same until reclaim, and the decor is scattered
        // before its block is asked for.
        let starved = self.builder.stat.starved;
        let mut committed = under_budget(visible, MAX_COMMITS_PER_FRAME, deadline, hard, now, |i| {
            self.builder.stat.starved == starved && self.commit(world, i, emit)
        });
        let releases = if self.refused_last { MAX_RELEASES_WHEN_REFUSED } else { MAX_RELEASES_PER_FRAME };
        committed += under_budget(offscreen, releases, deadline, hard, now, |i| self.commit(world, i, emit));

        let a = crate::probe::add(4, a);
        // The builds, visible pending first in order, with what is left; the
        // chunks a tier landed on are committed after.
        still.clear();
        still.extend(visible.iter().filter(|&&i| self.slots[i].has(PENDING)).map(|&i| self.slots[i].id));
        landed.clear();
        let budget = (deadline - now()).max(0.0);
        {
            let (slots, index, ron, roff, draw) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off, &mut self.draw);
            let mem = world_mem(world);
            self.builder.tick_capped(world, still, budget, MAX_COMMITS_PER_FRAME, now, &mut |e| {
                if let BuildEvent::Tier { id, .. } = e {
                    landed.push(id);
                }
                self_record(slots, index, mem, ron, roff, draw, e, emit);
            });
        }
        let a = crate::probe::add(5, a);
        for &id in landed.iter() {
            if let Some(&i) = self.index.get(&id) {
                if self.commit(world, i, emit) {
                    committed += 1;
                }
            }
        }

        let a = crate::probe::add(6, a);
        let freed = self.reclaim(world, order, emit);
        self.scratch = sc;
        let a = crate::probe::add(7, a);
        self.press(world, t0, freed);
        let a = crate::probe::add(8, a);
        let animating = self.models.tick(world, &mut self.draw, frame.delta_s as f32) | self.markers.tick(&frame) | self.environment.tick(&frame);
        let a = crate::probe::add(9, a);
        self.draw_frame(&frame, world_mem(world));
        crate::probe::add(10, a);

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
        for s in &self.slots {
            if !s.has(IN_FRUSTUM) || !s.layout_ready {
                continue;
            }
            let (wp, ws) = s.told.map_or((1.0, 0.0), |t| (t.0, t.1));
            let translation = DrawList::translation([s.base_x, s.base_y, 0.0], frame.origin);
            for (step, visible) in [(s.layout_primary, wp), (s.layout_secondary, ws)] {
                if step == 0 {
                    continue;
                }
                let Some(t) = s.tier(step) else { continue };
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
            if let Some(d) = s.decor.iter().find(|d| d.step == decor_step) {
                for (group, (offset, count)) in d.groups(mem).enumerate() {
                    if count > 0 {
                        self.draw.draw_decor(d.handle, group as u32, count, offset, translation);
                    }
                }
            }
        }
    }

    /// Where every chunk is and whether it is on screen, from the anchor
    /// root's matrix: sched/chunk_table.ts's refresh.
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

    /// chunk_node.ts's recheck: the tier wanted, the blend, what to commit.
    fn recheck(&mut self, world: &mut World, i: usize, now: f64, frame: &Frame, emit: &mut impl FnMut(Event)) {
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
        let guide = self.ladder.guide(s.distance2.sqrt(), s.layout_primary);
        let (primary, secondary) = match guide.blend {
            Some((finer, coarser, _)) => (finer, coarser),
            None => (guide.step, 0),
        };
        self.slots[i].desired = (primary, secondary);
        let s = &self.slots[i];
        let pending = !s.layout_ready || s.layout_primary != primary || s.layout_secondary != secondary;
        self.set_pending(i, pending, build_request(primary, secondary), now);
        let s = &mut self.slots[i];
        let settling = s.has(PENDING) && s.layout_ready;
        s.set(SETTLING, settling);
        let pending = s.has(PENDING);
        s.set(BLEND_ACTIVE, guide.blend.is_some());
        if !pending {
            self.stop_work(world, i);
        }
        let Some((finer_step, _, f)) = guide.blend else {
            self.decide(i, 1.0, 0.0, emit);
            return;
        };
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

    /// Whether the glue holds every tier of (primary, secondary), asking the
    /// builder for the ones it lacks when not.
    fn ask(&mut self, world: &mut World, i: usize, primary: u32, secondary: u32) -> bool {
        let s = &self.slots[i];
        let steps = if secondary == 0 || secondary == primary { [primary, 0] } else { [primary, secondary] };
        let mut want = [0; 2];
        let mut n = 0;
        for st in steps.into_iter().filter(|&st| st != 0 && s.tier(st).is_none()) {
            want[n] = st;
            n += 1;
        }
        self.builder.want(world, s.id, want);
        n == 0
    }

    fn apply_layout(&mut self, i: usize, primary: u32, secondary: u32, ready: bool, emit: &mut impl FnMut(Event)) {
        let s = &mut self.slots[i];
        s.layout_primary = primary;
        s.layout_secondary = if secondary == primary { 0 } else { secondary };
        s.layout_ready = ready;
        s.set(EMPTY, !ready);
        s.told = None;
        if ready {
            let mut b: Option<Bounds> = None;
            for st in [s.layout_primary, s.layout_secondary] {
                if let Some(Some(t)) = s.tier(st).map(|t| t.bounds) {
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
                None => {
                    s.bc = [CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0.0];
                    s.radius = APPROXIMATE_CHUNK_RADIUS;
                }
            }
        }
        emit(Event::Layout {
            id: s.id,
            primary: s.layout_primary,
            secondary: s.layout_secondary,
            ready,
        });
    }

    /// Lets go of tiers past MAX_CACHED_TIERS, oldest built first, never the
    /// active ones or the off-screen tier.
    fn trim(&mut self, i: usize, active: [u32; 2], emit: &mut impl FnMut(Event)) {
        let before = self.slots[i].retained();
        let s = &mut self.slots[i];
        let mut k = 0;
        while s.tiers.len() > MAX_CACHED_TIERS && k < s.tiers.len() {
            let st = s.tiers[k].step;
            if st == COARSEST || active.contains(&st) {
                k += 1;
                continue;
            }
            let gone = s.tiers.remove(k);
            self.draw.free(gone.handle);
            let draw = &mut self.draw;
            s.decor.retain(|d| {
                if d.step == st {
                    draw.free(d.handle);
                }
                d.step != st
            });
            emit(Event::Evict { id: s.id, step: st });
        }
        self.account(i, before);
    }

    /// chunk_node.ts's commit: a release off screen, a layout on it whose
    /// tiers are all in hand.
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
            self.apply_layout(i, COARSEST, 0, false, emit);
            self.set_pending(i, false, RELEASE_REQUEST, 0.0);
            self.census.released += 1;
            return true;
        }
        let (p, q) = s.desired;
        if s.refused.is_some_and(|r| r == crate::host::heap_releases()) {
            return false;
        }
        let a = crate::probe::mark();
        let asked = self.ask(world, i, p, q);
        let a = crate::probe::add(13, a);
        if !asked || !self.ensure_decor(world, i, [p, q], emit) {
            crate::probe::add(14, a);
            return false;
        }
        let a = crate::probe::add(14, a);
        self.trim(i, [p, q], emit);
        let a = crate::probe::add(15, a);
        self.apply_layout(i, p, q, true, emit);
        crate::probe::add(16, a);
        self.set_pending(i, false, RELEASE_REQUEST, 0.0);
        true
    }

    /// The retain budget: off-screen built work past it, or the mesh section
    /// past MESH_PRESSURE, is given back farthest first. A block refused
    /// since the last pass is pressure too, whatever the numbers say: the
    /// section has room in bytes but not in one piece, and what gives it
    /// back in pieces that join is the off-screen work, farthest first.
    fn reclaim(&mut self, world: &mut World, order: &[usize], emit: &mut impl FnMut(Event)) -> u32 {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let refused = self.builder.stat.starved != self.refused_seen;
        self.refused_seen = self.builder.stat.starved;
        self.refused_last = refused;
        let pressed = mesh_pressed;
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
            let mut steps = 0;
            for t in core::mem::take(&mut s.tiers) {
                self.draw.free(t.handle);
                steps |= t.step;
            }
            for d in core::mem::take(&mut s.decor) {
                self.draw.free(d.handle);
                steps |= d.step;
            }
            emit_evicts(id, steps, emit);
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
            let spare = s.runs_bytes > 0 || s.tiers.iter().any(|t| t.step != s.layout_primary && t.step != s.layout_secondary);
            if !spare {
                continue;
            }
            let id = s.id;
            let (p, q) = (s.layout_primary, s.layout_secondary);
            let before = s.retained();
            self.builder.reclaim(world, id, &mut |e| emit(Event::Build(e)));
            let s = &mut self.slots[i];
            s.runs_bytes = 0;
            let mut steps = 0;
            let draw = &mut self.draw;
            s.tiers.retain(|t| {
                let keep = t.step == p || t.step == q;
                if !keep {
                    draw.free(t.handle);
                    steps |= t.step;
                }
                keep
            });
            s.decor.retain(|d| {
                let keep = d.step == p || d.step == q;
                if !keep {
                    draw.free(d.handle);
                    steps |= d.step;
                }
                keep
            });
            emit_evicts(id, steps, emit);
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
        } else if pressure > 0 && now - self.refused_at >= PRESSURE_RELIEF_MS && now - self.pressure_at >= PRESSURE_RELIEF_MS && !mesh_pressed(world) {
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

/// Whether the heap is live past MESH_PRESSURE.
fn mesh_pressed(_world: &World) -> bool {
    crate::host::heap_live() as f64 > crate::host::heap_bytes() as f64 * MESH_PRESSURE
}

/// Ids the module gives the chunks it mounts, apart from any the glue adds.
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
        && a.render_dist == b.render_dist
        && a.show_decor == b.show_decor
}

/// The hash that picks a chunk's side of a per-chunk blend: FNV-1a over the
/// key chunk_node.ts named it by, `<page index>:<base x>,<base y>`.
fn blend_hash(page_idx: IVec, bx: u32, by: u32) -> u32 {
    /// FNV-1a over what is written, as it is written.
    struct Fnv(u32);
    impl Write for Fnv {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for b in s.bytes() {
                self.0 ^= b as u32;
                self.0 = self.0.wrapping_mul(16777619);
            }
            Ok(())
        }
    }
    let mut h = Fnv(2166136261);
    let _ = write!(h, "{page_idx}:{bx},{by}");
    h.0
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

/// The order, as sched/priority.ts had it: the slots' holders, then what is
/// on screen nearest to where the camera is heading, then what is off it
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
fn measure(frame: &Frame, s: &Slot) -> (bool, [f64; 3], f64, f64) {
    let [x, y, z] = frame.world_of([s.base_x + s.bc[0], s.base_y + s.bc[1], s.bc[2]]);
    let c = frame.camera;
    let d2 = (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2);
    let l = frame.lead;
    let ld2 = (x - l[0]).powi(2) + (y - l[1]).powi(2) + (z - l[2]).powi(2);
    let half = CHUNK_WORLD_SPAN * 0.5;
    let mut visible = (c[0] - x).abs() <= half && (c[1] - y).abs() <= half;
    if !visible {
        visible = true;
        for p in frame.planes.chunks_exact(4) {
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
            step,
            at,
            bytes,
            vertex_cnt,
            index_cnt,
            ..
        } => {
            if let Some(&i) = index.get(&id) {
                let s = &mut slots[i];
                if s.tier(step).is_none() {
                    // SAFETY: the mesh's position lane, the first three floats
                    // a vertex of the block the bake just wrote.
                    let bounds = (vertex_cnt > 0).then(|| unsafe {
                        let p = core::slice::from_raw_parts(mem.add(at as usize) as *const f32, vertex_cnt as usize * 3);
                        let mut b = Bounds {
                            min: [f32::INFINITY; 3],
                            max: [f32::NEG_INFINITY; 3],
                        };
                        for v in p.chunks_exact(3) {
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
mod tests {
    use super::*;

    fn p(in_frustum: bool, lead_distance2: f64) -> Priority {
        Priority {
            holds: false,
            in_frustum,
            empty: false,
            lead_distance2,
            pending_since: -1.0,
        }
    }

    fn ordered(items: &[(&'static str, Priority)], now: f64) -> Vec<&'static str> {
        let mut v = items.to_vec();
        v.sort_by(|a, b| priority(&a.1, &b.1, now));
        v.into_iter().map(|(n, _)| n).collect()
    }

    #[test]
    fn slot_holders_then_on_screen_then_nearest() {
        let items = [
            ("far refine", p(true, 900.0)),
            ("near hidden", Priority { empty: true, ..p(false, 1.0) }),
            ("holder", Priority { holds: true, ..p(false, 5000.0) }),
            ("near refine", p(true, 4.0)),
            ("far hole", Priority { empty: true, ..p(true, 2500.0) }),
        ];
        assert_eq!(ordered(&items, 0.0), ["holder", "near refine", "far refine", "far hole", "near hidden"]);
    }

    #[test]
    fn a_near_coarse_chunk_goes_before_a_far_empty_one_and_a_hole_wins_a_tie() {
        assert_eq!(
            ordered(&[("far, empty", Priority { empty: true, ..p(true, 1e6) }), ("near, coarse", p(true, 100.0))], 0.0),
            ["near, coarse", "far, empty"]
        );
        assert_eq!(ordered(&[("coarse", p(true, 400.0)), ("empty", Priority { empty: true, ..p(true, 400.0) })], 0.0), ["empty", "coarse"]);
    }

    #[test]
    fn a_wait_overtakes_after_log2_of_how_much_farther_and_never_lifts_onto_the_screen() {
        let first_at = |now: f64| {
            let far = Priority {
                pending_since: 0.0,
                ..p(true, 64.0 * 64.0)
            };
            let near = Priority {
                pending_since: now,
                ..p(true, 8.0 * 8.0)
            };
            ordered(&[("near, fresh", near), ("far, waiting", far)], now)[0]
        };
        assert_eq!(first_at(3.0 * AGE_HALF_MS - 1.0), "near, fresh");
        assert_eq!(first_at(3.0 * AGE_HALF_MS + 1.0), "far, waiting");
        let aged_release = Priority { pending_since: 0.0, ..p(false, 1.0) };
        let fresh_build = Priority { pending_since: 1e6, ..p(true, 1e12) };
        assert_eq!(ordered(&[("release", aged_release), ("build", fresh_build)], 1e6)[0], "build");
        assert_eq!(ordered(&[("near", p(false, 4.0)), ("far", p(false, 900.0))], 0.0), ["far", "near"]);
        let a = Priority { pending_since: 100.0, ..p(true, 9.0) };
        let b = Priority { pending_since: 100.0, ..p(true, 4.0) };
        assert_eq!(ordered(&[("a", a), ("b", b)], 5000.0)[0], "b");
    }
}
