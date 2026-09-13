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

use std::collections::BTreeMap;

use crate::build::{Builder, Event as BuildEvent, PageLanes, World};
use crate::lod::{Ladder, COARSEST};

/// A chunk's side, in world units.
const CHUNK_WORLD_SPAN: f64 = 64.0;
/// The sphere a chunk nothing has measured is culled by.
const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;
/// How often every chunk is rechecked, whatever is moving.
const RECHECK_INTERVAL_S: f64 = 0.15;
/// Layouts applied per frame on screen, and releases off it.
const MAX_COMMITS_PER_FRAME: u32 = 8;
const MAX_RELEASES_PER_FRAME: u32 = 2;
/// Off-screen chunks that may give their built work back in one frame.
const MAX_RECLAIMS_PER_FRAME: u32 = 16;
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
}

/// What the glue is told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Build(BuildEvent),
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
    /// Let go of a tier the glue holds; the layout keeps what it shows.
    Evict {
        id: u32,
        step: u32,
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
        self.runs_bytes as f64 + self.tiers.iter().map(|t| t.bytes as f64).sum::<f64>()
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
    slots: Vec<Slot>,
    index: BTreeMap<u32, usize>,
    ladder: Ladder,
    recheck_elapsed: f64,
    frame: Option<Frame>,
    retained_on: f64,
    retained_off: f64,
    pub census: Census,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new()
    }
}

/// Runs `step` over `items` under a budget: the first item always, then
/// until `cap` items did something or the budget is spent, and never past a
/// hard deadline once one item was tried. How many did something.
fn under_budget(items: &[u32], cap: u32, deadline: f64, hard: f64, now: &impl Fn() -> f64, mut step: impl FnMut(u32) -> bool) -> u32 {
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
            slots: Vec::new(),
            index: BTreeMap::new(),
            ladder: Ladder::default(),
            recheck_elapsed: RECHECK_INTERVAL_S,
            frame: None,
            retained_on: 0.0,
            retained_off: 0.0,
            census: Census::default(),
        }
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
        self.slots.push(slot);
    }

    pub fn remove(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) {
        let Some(i) = self.index.remove(&id) else { return };
        self.builder.remove(world, id, &mut |e| emit(Event::Build(e)));
        let before = self.slots[i].retained();
        if self.slots[i].has(IN_FRUSTUM) {
            self.retained_on -= before;
        } else {
            self.retained_off -= before;
        }
        self.slots.swap_remove(i);
        if i < self.slots.len() {
            self.index.insert(self.slots[i].id, i);
        }
    }

    /// The chunk's sources changed. The tiers the glue holds are stale and it
    /// lets them go itself; what is on screen stays until the rebuild lands.
    pub fn source(&mut self, world: &mut World, id: u32, page: Option<PageLanes>, show_structures: bool, emit: &mut impl FnMut(Event)) {
        let Some(&i) = self.index.get(&id) else { return };
        let before = self.slots[i].retained();
        self.builder.source(world, id, page, show_structures, &mut |e| emit(Event::Build(e)));
        let s = &mut self.slots[i];
        s.tiers.clear();
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
            let (slots, index, ron, roff) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off);
            self.builder.tick(world, &[id], f64::INFINITY, now, &mut |e| self_record(slots, index, mem, ron, roff, e, emit));
        }
        if self.ask(world, i, p, s2) {
            self.trim(i, [p, s2], emit);
            self.apply_layout(i, p, s2, true, emit);
        } else {
            let s = &mut self.slots[i];
            s.layout_ready = false;
            s.set(EMPTY, true);
        }
    }

    // ---- the tick ----

    /// One frame: measure every chunk, recheck, commit, build and reclaim
    /// under the frame's budget. Whether the canvas has something new to draw.
    pub fn tick(&mut self, world: &mut World, frame: Frame, now: &impl Fn() -> f64, emit: &mut impl FnMut(Event)) -> bool {
        let t0 = now();
        let deadline = t0 + frame.budget_ms;
        let hard = deadline + frame.hard_slack_ms;
        self.frame = Some(frame);
        self.ladder.set_projection(frame.viewport_h, frame.fov_deg);
        self.recheck_elapsed += frame.delta_s;
        let full_recheck = self.recheck_elapsed >= RECHECK_INTERVAL_S;
        if full_recheck {
            self.recheck_elapsed = 0.0;
        }

        self.refresh(&frame, emit);
        let order = self.order(t0);

        // The pending lists from the flags as the frame found them, before
        // the recheck moves them, as the stage's pass had it.
        let (mut visible, mut offscreen) = (Vec::new(), Vec::new());
        for &id in &order {
            let s = &self.slots[self.index[&id]];
            if s.has(PENDING) {
                if s.has(IN_FRUSTUM) {
                    visible.push(id)
                } else {
                    offscreen.push(id)
                }
            }
        }

        let eligible: Vec<u32> = order
            .iter()
            .copied()
            .filter(|&id| full_recheck || self.slots[self.index[&id]].flags & (BLEND_ACTIVE | SETTLING) != 0)
            .collect();
        under_budget(&eligible, u32::MAX, deadline, hard, now, |id| {
            self.recheck(world, id, t0, &frame, emit);
            true
        });

        let pending_total = (visible.len() + offscreen.len()) as u32;
        let mut committed = under_budget(&visible, MAX_COMMITS_PER_FRAME, deadline, hard, now, |id| self.commit(world, id, emit));
        committed += under_budget(&offscreen, MAX_RELEASES_PER_FRAME, deadline, hard, now, |id| self.commit(world, id, emit));

        // The builds, visible pending first in order, with what is left; the
        // chunks a tier landed on are committed after.
        let still: Vec<u32> = visible.iter().copied().filter(|&id| self.slots[self.index[&id]].has(PENDING)).collect();
        let mut landed: Vec<u32> = Vec::new();
        let budget = (deadline - now()).max(0.0);
        {
            let (slots, index, ron, roff) = (&mut self.slots, &self.index, &mut self.retained_on, &mut self.retained_off);
            let mem = world_mem(world);
            self.builder.tick_capped(world, &still, budget, MAX_COMMITS_PER_FRAME, now, &mut |e| {
                if let BuildEvent::Tier { id, .. } = e {
                    landed.push(id);
                }
                self_record(slots, index, mem, ron, roff, e, emit);
            });
        }
        for id in landed {
            if self.commit(world, id, emit) {
                committed += 1;
            }
        }

        self.reclaim(world, &order, emit);

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
        committed > 0 || pending_total > committed || blending
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
            }
        }
    }

    /// Every chunk's id in priority order (`priority`).
    fn order(&self, now: f64) -> Vec<u32> {
        let holders = self.builder.holders();
        let keys: Vec<Priority> = self
            .slots
            .iter()
            .map(|s| Priority {
                holds: holders.contains(&Some(s.id)),
                in_frustum: s.has(IN_FRUSTUM),
                empty: s.has(EMPTY),
                lead_distance2: s.lead_distance2,
                pending_since: s.pending_since,
            })
            .collect();
        let mut idx: Vec<usize> = (0..self.slots.len()).collect();
        idx.sort_by(|&a, &b| priority(&keys[a], &keys[b], now));
        idx.into_iter().map(|i| self.slots[i].id).collect()
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
    fn recheck(&mut self, world: &mut World, id: u32, now: f64, frame: &Frame, emit: &mut impl FnMut(Event)) {
        let Some(&i) = self.index.get(&id) else { return };
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
            s.tiers.remove(k);
            emit(Event::Evict { id: s.id, step: st });
        }
        self.account(i, before);
    }

    /// chunk_node.ts's commit: a release off screen, a layout on it whose
    /// tiers are all in hand.
    fn commit(&mut self, world: &mut World, id: u32, emit: &mut impl FnMut(Event)) -> bool {
        let Some(&i) = self.index.get(&id) else { return false };
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
        if !self.ask(world, i, p, q) {
            return false;
        }
        self.trim(i, [p, q], emit);
        self.apply_layout(i, p, q, true, emit);
        self.set_pending(i, false, RELEASE_REQUEST, 0.0);
        true
    }

    /// The retain budget: off-screen built work past it, or the mesh section
    /// past MESH_PRESSURE, is given back farthest first.
    fn reclaim(&mut self, world: &mut World, order: &[u32], emit: &mut impl FnMut(Event)) {
        let budget = self.frame.map_or(f64::INFINITY, |f| f.retain_budget_bytes);
        let pressed = |world: &World| {
            let p = &world.pools[world.volumes.mesh_pool()];
            p.stat.live_bytes as f64 > p.range_bytes() as f64 * MESH_PRESSURE
        };
        let mut freed = 0;
        // The order already puts off-screen chunks farthest first.
        for &id in order {
            if freed >= MAX_RECLAIMS_PER_FRAME || !(self.retained_off > budget || pressed(world)) {
                break;
            }
            let Some(&i) = self.index.get(&id) else { continue };
            if self.slots[i].has(IN_FRUSTUM) || self.slots[i].retained() <= 0.0 {
                continue;
            }
            let before = self.slots[i].retained();
            self.builder.reclaim(world, id, &mut |e| emit(Event::Build(e)));
            let s = &mut self.slots[i];
            s.runs_bytes = 0;
            for t in s.tiers.drain(..) {
                emit(Event::Evict { id, step: t.step });
            }
            self.account(i, before);
            freed += 1;
        }
    }
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
    let key = |p: &Priority| {
        if p.pending_since < 0.0 {
            p.lead_distance2
        } else {
            p.lead_distance2 * 2f64.powf(-2.0 * (now - p.pending_since) / AGE_HALF_MS)
        }
    };
    b.holds
        .cmp(&a.holds)
        .then(b.in_frustum.cmp(&a.in_frustum))
        .then_with(|| if a.in_frustum { key(a).total_cmp(&key(b)) } else { key(b).total_cmp(&key(a)) })
        .then(b.empty.cmp(&a.empty))
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
fn self_record(slots: &mut [Slot], index: &BTreeMap<u32, usize>, mem: *const u8, retained_on: &mut f64, retained_off: &mut f64, e: BuildEvent, emit: &mut impl FnMut(Event)) {
    match e {
        BuildEvent::Tier { id, step, at, bytes, vertex_cnt, .. } => {
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
                    s.tiers.push(Tier { step, bytes, bounds });
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
