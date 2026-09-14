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

use std::collections::{BTreeMap, BTreeSet};

use crate::billboard::Billboards;
use crate::build::{Builder, Event as BuildEvent, PageLanes, World};
use crate::debug::DebugLines;
use crate::decor::{density as decor_density, Chunk as DecorChunk, DecorBuild, Kind as DecorKind, Rule as DecorRule};
use crate::draw::{DrawList, PIPELINE_CHUNK};
use crate::environment::Environment;
use crate::ghost::Ghost;
use crate::lod::{Ladder, COARSEST};
use crate::marker::Markers;
use crate::model::Models;
use crate::terrain::ChunkSample;

/// A chunk's side, in world units.
const CHUNK_WORLD_SPAN: f64 = 64.0;
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

/// What the glue is told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Build(BuildEvent),
    /// A chunk came into range: its page and its page-local base, in units.
    Mount {
        id: u32,
        page: u64,
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

#[derive(Clone, Copy)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Clone, Copy)]
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

/// A decor block the glue holds: its step and bytes, the renderer's handle,
/// and each group's lanes (DecorBuild::group_layout).
pub struct Decor {
    step: u32,
    /// Where the block is in the memory, and its size.
    pub at: u32,
    pub bytes: u32,
    handle: u32,
    /// Each group's first lane word in the block and its count.
    pub groups: Vec<(u32, u32)>,
}

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
    /// The pages with a terrain tile, which chunks are mounted from.
    pages: BTreeSet<u64>,
    pages_revision: u32,
    /// The chunks the module mounted, by page and page-local base.
    mounted: BTreeMap<(u64, u32, u32), u32>,
    next_mount_id: u32,
    mounted_for: Option<([f64; 2], f64, f64, u32)>,
    /// The decor kinds, in the order the glue taught them — the order of
    /// the groups in every decor block.
    decor_kinds: Vec<DecorKind>,
    decor_sample: ChunkSample,
    /// The builder's refusals as of the last reclaim, so a refusal since
    /// is pressure now; and whether the last pass saw one, for the next
    /// frame's release cap.
    refused_seen: u32,
    refused_last: bool,
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
            slots: Vec::new(),
            index: BTreeMap::new(),
            by_base: BTreeMap::new(),
            ladder: Ladder::default(),
            recheck_elapsed: RECHECK_INTERVAL_S,
            frame: None,
            view_dirty: true,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
            pages: BTreeSet::new(),
            pages_revision: 0,
            mounted: BTreeMap::new(),
            next_mount_id: MOUNT_ID_BASE,
            mounted_for: None,
            decor_kinds: Vec::new(),
            decor_sample: ChunkSample::default(),
            refused_seen: 0,
            refused_last: false,
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

    /// A chunk at `base` (its lattice word, and its X and Y in units),
    /// starting at page cell (tx0, ty0). Its first tier is guessed from the
    /// last frame, so the first recheck's hysteresis starts from where the
    /// camera is.
    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, id: u32, base: u64, base_x: f64, base_y: f64, tx0: u32, ty0: u32, stable_hash: u32) {
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
        for t in &self.slots[i].tiers {
            self.draw.free(t.handle);
        }
        for d in &self.slots[i].decor {
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
        for t in &s.tiers {
            self.draw.free(t.handle);
        }
        for d in &s.decor {
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
        for d in self.slots[i].decor.drain(..) {
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
    /// and handing over what it lacks: false when the pool refused a block.
    /// A chunk with no page has no decor to build.
    fn ensure_decor(&mut self, world: &mut World, i: usize, steps: [u32; 2], emit: &mut impl FnMut(Event)) -> bool {
        let id = self.slots[i].id;
        let Some((base, tx0, ty0, page, show_structures)) = self.builder.source_of(id) else { return true };
        let show_decor = self.frame.is_some_and(|f| f.show_decor);
        let (key_x, key_y) = lanes_xy(base);
        let page_idx = gather_xy(key_x >> PAGE_SHAMT, key_y >> PAGE_SHAMT);
        let mut sampled = false;
        for step in steps {
            if step == 0 || self.slots[i].decor.iter().any(|d| d.step == step) {
                continue;
            }
            let view = Builder::page_view(world, page);
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
                key: base,
                key_x,
                key_y,
                tx0,
                ty0,
                show_decor,
                hide: self.builder.hide_in(base),
            };
            let built = DecorBuild::build(&self.decor_kinds, &chunk, &self.decor_sample, world.store, step);
            let bytes = built.bytes();
            let pool = world.volumes.mesh_pool();
            let Some(at) = world.pools[pool].acquire(bytes) else {
                self.builder.stat.starved += 1;
                return false;
            };
            // SAFETY: a block of the bytes the build asked for, the glue's
            // once handed over.
            unsafe { built.write(world.volumes.mem().add(at)) };
            let before = self.slots[i].retained();
            let handle = self.draw.upload(at as u32, bytes, 0, 0);
            self.slots[i].decor.push(Decor {
                step,
                at: at as u32,
                bytes,
                handle,
                groups: built.group_layout(),
            });
            self.account(i, before);
            emit(Event::Decor { id, step, at: at as u32, bytes });
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
    pub fn page(&mut self, page: u64, present: bool) {
        let changed = if present { self.pages.insert(page) } else { self.pages.remove(&page) };
        if changed {
            self.pages_revision = self.pages_revision.wrapping_add(1);
            self.view_dirty = true;
        }
    }

    /// render/chunk_range.ts's rule, a page and a chunk at a time: a chunk
    /// whose square is within the radius of the anchor is mounted, and one
    /// already mounted is kept until it is past the radius and the
    /// hysteresis; a page with no tile, or out past both, mounts nothing.
    fn mount(&mut self, world: &mut World, frame: &Frame, emit: &mut impl FnMut(Event)) {
        let Some(anchor) = frame.anchor else { return };
        let key = (anchor, frame.render_dist, frame.range_hysteresis, self.pages_revision);
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
        let page_origin = |page: u64| {
            let (px, py) = lanes_xy(page);
            ((px << PAGE_SHAMT) as f64 - anchor[0], (py << PAGE_SHAMT) as f64 - anchor[1])
        };

        // Leave: every mounted chunk past the keep radius or on a page that left.
        let gone: Vec<((u64, u32, u32), u32)> = self
            .mounted
            .iter()
            .filter(|(&(page, bx, by), _)| {
                if !self.pages.contains(&page) {
                    return true;
                }
                let (x0, y0) = page_origin(page);
                rect(x0 + bx as f64, y0 + by as f64, CHUNK_WORLD_SPAN) > keep2
            })
            .map(|(&k, &id)| (k, id))
            .collect();
        for (k, id) in gone {
            self.mounted.remove(&k);
            self.remove(world, id, emit);
            emit(Event::Unmount { id });
        }

        // Enter: every chunk within the radius on a page with a tile.
        let pages: Vec<u64> = self.pages.iter().copied().collect();
        for page in pages {
            let (x0, y0) = page_origin(page);
            if rect(x0, y0, PAGE_SPAN) > keep2 {
                continue;
            }
            for cy in 0..PAGE_CHUNK_AXIS {
                for cx in 0..PAGE_CHUNK_AXIS {
                    let (bx, by) = (cx * CHUNK_SPAN_CELLS, cy * CHUNK_SPAN_CELLS);
                    if self.mounted.contains_key(&(page, bx, by)) || rect(x0 + bx as f64, y0 + by as f64, CHUNK_WORLD_SPAN) > r2 {
                        continue;
                    }
                    if mounts >= MAX_MOUNTS_PER_FRAME {
                        // The rest next frame: the key stays unrecorded.
                        return;
                    }
                    mounts += 1;
                    let id = self.next_mount_id;
                    self.next_mount_id = self.next_mount_id.wrapping_add(1).max(MOUNT_ID_BASE);
                    let (px, py) = lanes_xy(page);
                    let (wx, wy) = ((px << PAGE_SHAMT) + bx as i64, (py << PAGE_SHAMT) + by as i64);
                    let base = ((wx as u64 & 0xff_ffff) << 40) | ((wy as u64 & 0xff_ffff) << 16);
                    emit(Event::Mount { id, page, base_x: bx, base_y: by });
                    self.add(id, base, wx as f64, wy as f64, bx, by, blend_hash(page, bx, by));
                    self.mounted.insert((page, bx, by), id);
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

        self.mount(world, &frame, emit);
        self.refresh(&frame, emit);
        let order = self.order(t0);

        // The pending lists from the flags as the frame found them, before
        // the recheck moves them, as the stage's pass had it.
        let (mut visible, mut offscreen) = (Vec::new(), Vec::new());
        for &i in &order {
            let s = &self.slots[i];
            if s.has(PENDING) {
                if s.has(IN_FRUSTUM) {
                    visible.push(i)
                } else {
                    offscreen.push(i)
                }
            }
        }

        let eligible: Vec<usize> = order.iter().copied().filter(|&i| full_recheck || self.slots[i].flags & (BLEND_ACTIVE | SETTLING) != 0).collect();
        let rechecked = under_budget(&eligible, u32::MAX, deadline, hard, now, |i| {
            self.recheck(world, i, t0, &frame, emit);
            true
        });
        if full_recheck && rechecked as usize == eligible.len() {
            self.view_dirty = false;
        }

        let pending_total = (visible.len() + offscreen.len()) as u32;
        let mut committed = under_budget(&visible, MAX_COMMITS_PER_FRAME, deadline, hard, now, |i| self.commit(world, i, emit));
        let releases = if self.refused_last { MAX_RELEASES_WHEN_REFUSED } else { MAX_RELEASES_PER_FRAME };
        committed += under_budget(&offscreen, releases, deadline, hard, now, |i| self.commit(world, i, emit));

        // The builds, visible pending first in order, with what is left; the
        // chunks a tier landed on are committed after.
        let still: Vec<u32> = visible.iter().filter(|&&i| self.slots[i].has(PENDING)).map(|&i| self.slots[i].id).collect();
        let mut landed: Vec<u32> = Vec::new();
        let budget = (deadline - now()).max(0.0);
        {
            let (slots, index, ron, roff, draw) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off, &mut self.draw);
            let mem = world_mem(world);
            self.builder.tick_capped(world, &still, budget, MAX_COMMITS_PER_FRAME, now, &mut |e| {
                if let BuildEvent::Tier { id, .. } = e {
                    landed.push(id);
                }
                self_record(slots, index, mem, ron, roff, draw, e, emit);
            });
        }
        for id in landed {
            if let Some(&i) = self.index.get(&id) {
                if self.commit(world, i, emit) {
                    committed += 1;
                }
            }
        }

        self.reclaim(world, &order, emit);
        self.models.tick(world, &mut self.draw, frame.delta_s as f32);
        let animating = self.markers.tick(&frame) | self.environment.tick(&frame);
        self.draw_frame(&frame);

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
        let blending = c.blend_active > 0;
        self.census = c;
        committed > 0 || pending_total > committed || blending || self.view_dirty || animating
    }

    /// This frame's draws: every chunk on screen with a layout, its tiers at
    /// the weights the blend decided, placed where the chunk is in the frame
    /// the camera is written in.
    fn draw_frame(&mut self, frame: &Frame) {
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
            let translation = [(s.base_x - frame.origin[0]) as f32, (s.base_y - frame.origin[1]) as f32, (-frame.origin[2]) as f32];
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
                for (group, &(offset, count)) in d.groups.iter().enumerate() {
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
    fn order(&self, now: f64) -> Vec<usize> {
        let holders = self.builder.holders();
        let keys: Vec<Keyed> = self
            .slots
            .iter()
            .map(|s| {
                Priority {
                    holds: holders.contains(&Some(s.id)),
                    in_frustum: s.has(IN_FRUSTUM),
                    empty: s.has(EMPTY),
                    lead_distance2: s.lead_distance2,
                    pending_since: s.pending_since,
                }
                .keyed(now)
            })
            .collect();
        let mut idx: Vec<usize> = (0..self.slots.len()).collect();
        idx.sort_by(|&a, &b| priority_keyed(&keys[a], &keys[b]));
        idx
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
        let missing: Vec<u32> = steps.iter().copied().filter(|&st| st != 0 && s.tier(st).is_none()).collect();
        let want = [missing.first().copied().unwrap_or(0), missing.get(1).copied().unwrap_or(0)];
        self.builder.want(world, s.id, want);
        missing.is_empty()
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
        if !self.ask(world, i, p, q) || !self.ensure_decor(world, i, [p, q], emit) {
            return false;
        }
        self.trim(i, [p, q], emit);
        self.apply_layout(i, p, q, true, emit);
        self.set_pending(i, false, RELEASE_REQUEST, 0.0);
        true
    }

    /// The retain budget: off-screen built work past it, or the mesh section
    /// past MESH_PRESSURE, is given back farthest first. A block refused
    /// since the last pass is pressure too, whatever the numbers say: the
    /// section has room in bytes but not in one piece, and what gives it
    /// back in pieces that join is the off-screen work, farthest first.
    fn reclaim(&mut self, world: &mut World, order: &[usize], emit: &mut impl FnMut(Event)) {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let refused = self.builder.stat.starved != self.refused_seen;
        self.refused_seen = self.builder.stat.starved;
        self.refused_last = refused;
        let pressed = |world: &World| {
            let p = &world.pools[world.volumes.mesh_pool()];
            p.stat.live_bytes as f64 > p.range_bytes() as f64 * MESH_PRESSURE
        };
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
            let mut steps: Vec<u32> = Vec::new();
            for t in s.tiers.drain(..) {
                self.draw.free(t.handle);
                steps.push(t.step);
            }
            for d in s.decor.drain(..) {
                self.draw.free(d.handle);
                if !steps.contains(&d.step) {
                    steps.push(d.step);
                }
            }
            for step in steps {
                emit(Event::Evict { id, step });
            }
            self.account(i, before);
            freed += 1;
        }
        if !refused || freed >= cap {
            return;
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
            let mut steps: Vec<u32> = Vec::new();
            let draw = &mut self.draw;
            s.tiers.retain(|t| {
                let keep = t.step == p || t.step == q;
                if !keep {
                    draw.free(t.handle);
                    steps.push(t.step);
                }
                keep
            });
            s.decor.retain(|d| {
                let keep = d.step == p || d.step == q;
                if !keep {
                    draw.free(d.handle);
                    if !steps.contains(&d.step) {
                        steps.push(d.step);
                    }
                }
                keep
            });
            for step in steps {
                emit(Event::Evict { id, step });
            }
            self.account(i, before);
            freed += 1;
        }
    }
}

/// Ids the module gives the chunks it mounts, apart from any the glue adds.
const MOUNT_ID_BASE: u32 = 1 << 31;
/// A page's side in cells and units, its chunks on a side, a chunk's side
/// in cells, and the lattice shift from a page index to its base.
const PAGE_SPAN: f64 = 1024.0;
const PAGE_CHUNK_AXIS: u32 = 16;
const CHUNK_SPAN_CELLS: u32 = 64;
const PAGE_SHAMT: u32 = 10;

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

/// A lattice word's X and Y lanes, signed: pvf3/ivec.ts's layout, X in the
/// top 24 bits, Y in the next 24, Z in the low 16.
fn lanes_xy(v: u64) -> (i64, i64) {
    ((v as i64) >> 40, ((v << 24) as i64) >> 40)
}

/// The lattice word of (x, y, 0).
fn gather_xy(x: i64, y: i64) -> u64 {
    ((x as u64 & 0xff_ffff) << 40) | ((y as u64 & 0xff_ffff) << 16)
}

/// The hash that picks a chunk's side of a per-chunk blend: FNV-1a over the
/// key chunk_node.ts named it by, `<page index>:<base x>,<base y>`.
fn blend_hash(page: u64, bx: u32, by: u32) -> u32 {
    let key = format!("{}:{},{}", page, bx, by);
    let mut h: u32 = 2166136261;
    for b in key.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
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
    let m = &frame.root;
    let lx = s.base_x - frame.origin[0] + s.bc[0];
    let ly = s.base_y - frame.origin[1] + s.bc[1];
    let lz = s.bc[2] - frame.origin[2];
    let x = m[0] * lx + m[4] * ly + m[8] * lz + m[12];
    let y = m[1] * lx + m[5] * ly + m[9] * lz + m[13];
    let z = m[2] * lx + m[6] * ly + m[10] * lz + m[14];
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
