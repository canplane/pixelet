// pxl-kernel/world/chunk/frontier.rs

//! Materialization: what is drawn, made from the observation's goal
//! (world/pyramid/domain.rs), the resident records and the arena
//! (world/chunk/mesh_slot.rs) — a concern of its own, whose budget is not
//! residency's. Its contract:
//!
//! 1. It never takes a record to be there that is not: a piece is built
//!    only where a page at or over it is resident (`Env::buildable`).
//! 2. It meets the observation's quality where it can: it moves toward the
//!    goal at the effective quality — the cover all around, refined where
//!    the screen's cells see it — and nothing else.
//! 3. Where it cannot yet, a defined coarser answer stands, never a hole.
//! 4. It never goes past the arena.
//! 5. Without a cache the result is the same: a slot holds a mesh only while
//!    it is drawn or about to be, and every one is made again from the
//!    records when asked for.
//!
//! **What is drawn is a frontier with no gap.** The drawn pieces are an
//! antichain of the chunk tree, and every piece of the goal is one of them,
//! under one, or covered by those under it. A frontier moves in three ways,
//! each whole and at once, and only once everything it brings is ready:
//!
//! - a drawn piece over goal pieces is replaced by its children on the way
//!   to them, a level at a time (a refinement);
//! - drawn pieces under a goal piece are replaced by it (a coarsening);
//! - a goal piece with nothing drawn over, at or under it is added (a fill:
//!   ground new to the view, where nothing was ever drawn).
//!
//! Until then what was drawn stays drawn, so no step of any of them leaves a
//! hole: waiting for a page, a build, or a slot is always waiting with the
//! coarser piece on the screen. A drawn piece over no goal piece at all —
//! ground past the far edge, ground with no answer, or air — goes at once,
//! since nothing of the view is under it. The goal's cover stands all
//! around the eye, so turning brings no ground new to the frontier: only a
//! fill of the cover does, where the floor has just come.
//!
//! **The arena is never overrun, by admission.** A move is started only when
//! every slot it will take is free, and those are held for it from then
//! (`Now::free` counts them out), so a mesh that lands always finds its
//! slots. A refinement, a fill or a change of cube leaves free past it what
//! the one coarsening after it takes at most (`RESERVE`): a coarsening lands
//! one piece over the pieces it replaces, always of a level over z16 and so
//! in the coarse pool, and fills at most `COARSE_TIER_SLOTS` of it whatever
//! it holds. So one coarsening is always possible, and the frontier can
//! always come back to a goal the arena holds: progress needs one, not a
//! slot for each. What moves already under way hold is counted out of the
//! free slots, not again in the reserve. Refinements under way at once are
//! `param::REFINEMENTS_IN_FLIGHT`: each holds the piece it replaces on the
//! screen past the frontier, which is throughput, not progress.
//!
//! **A full arena is latency, never a hole.** A drawn piece leaves the
//! frontier in two ways only: in a move whose every piece is ready
//! (`Plan::swaps`), or over no ground the goal has (`Plan::drop`). A move
//! the arena has no room for is refused (`Plan::refused`) and what it would
//! replace stays drawn. Under that, the table never takes a slot a tier
//! holds (table.rs `claim`), and a mesh that finds not all the slots it
//! fills free is thrown away with the drawn piece untouched (table.rs
//! `reserve`). What a
//! full arena delays that nothing coarser covers is ground new to the view,
//! whose first piece waits as it waits for its page; and since the quality
//! policy keeps the goal and `HEADROOM` inside the arena, only while the
//! frontier comes down from a finer goal.
//!
//! **What fits is the quality policy's** (world/quality.rs), which counts
//! the goal before anything is allocated (`need`). Waiting and choosing
//! coarser are two things: a fault, a build, a slot not yet free leave the
//! goal as it is while the frontier converges; a goal that cannot fit at
//! all is cut coarser by the policy, and both demand and this read that.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use pxl_render::look::lod::COARSEST;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::lod::Tau;
use pxl_spec::space::pyramid::{GROUND, TOP_LEVEL};

use crate::param::REFINEMENTS_IN_FLIGHT;
use crate::world::chunk::build::{mesh_step, Cube};
use crate::world::chunk::mesh_slot::{Pool, COARSE_TIER_SLOTS, POOLS};
use crate::world::observation::Order;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{Cut, Domain};

/// Slots, by pool (`Pool` as an index).
pub type Slots = [u32; POOLS];

/// Slots each pool keeps free past a refinement, a fill or a change of cube,
/// for one coarsening: none in the fine pool, where no coarsening lands, and
/// in the coarse pool the most a coarsening's piece can fill
/// (`COARSE_TIER_SLOTS`), a bound from the representation and not from
/// what has been baked so far.
pub const RESERVE: Slots = [0, COARSE_TIER_SLOTS];

/// Slots each pool keeps past the frontier while it moves, which the quality
/// policy fits the goal beside: the reserve, which is progress, and a slot
/// for each refinement under way, holding the piece it replaces, which is
/// throughput.
pub const HEADROOM: Slots = [REFINEMENTS_IN_FLIGHT + RESERVE[0], REFINEMENTS_IN_FLIGHT + RESERVE[1]];

/// The cube the rule wants ground drawn in at `near` from the eye at `tau`:
/// the level wanted there, whose cells terrain draws averaged 2 × 2 — a cube
/// a level over it. It has no floor at terrain's atom: nearer than z16's
/// split distance it is finer than a z16 cell, which terrain draws as it is
/// and objects fold to, down to their finest tier (build.rs `Cube`).
pub fn wanted_cube(near: f64, tau: Tau) -> Cube {
    let finest = OBJECT_FINEST.above(GROUND.scale(0));
    let level = tau.level(near).max(finest - 1) + 1;
    GROUND.scale(0).up(level.min((COARSEST.trailing_zeros() + TOP_LEVEL) as i32))
}

/// A wanted cube as a chunk of `level` draws it. A level draws its own cells
/// averaged 2 × 2 — the cube the level above would otherwise draw as its
/// vote — and z16's chunks their own cells too, and finer than those only
/// where they hold objects, which fold to the cube. Finer than that a chunk
/// is not drawn — the split serves it — and coarser it is merged into its
/// parent.
///
/// A chunk read from a page over it is drawn at its own cube like any other:
/// its heights are that page's surface sampled at its own cells, which
/// invents no step, and only its color is the coarser page's. Drawn at the
/// coarser page's cells instead, a chunk far under its page stood one cube
/// as wide as itself, and those stood apart as blocks over the sea after a
/// jump (2026-09-26).
///
/// The source's top is the cover, and the cover holds completeness, not
/// quality: a piece of it is drawn in the coarsest cube, one cell a chunk,
/// so it is at most two boxes and one slot whatever it holds, and the
/// cover's count is its slots (world/quality.rs asserts it fits). Where its
/// ground is seen closely enough for that to leak past a cell, the domain
/// splits it, measuring it as it is drawn (world/pyramid/domain.rs).
pub fn drawn_cube(cube: Cube, level: u32, outermost: bool, objects: bool) -> Cube {
    if outermost {
        return coarsest(level);
    }
    // A level draws its own cells one cube coarser than they are, and no
    // coarser: level k's cells averaged 4 × 4 are the ground level k+1 draws
    // averaged 2 × 2, and that ring is the one holding it.
    let own = GROUND.scale(level);
    let finest = match (level, objects) {
        (0, true) => OBJECT_FINEST,
        (0, false) => own,
        _ => own.up(1),
    };
    cube.clamp(finest, own.up(1))
}

/// The coarsest cube a chunk of `level` is ever drawn in: its own cells
/// COARSEST a side.
pub fn coarsest(level: u32) -> Cube {
    GROUND.scale(level).up(COARSEST.trailing_zeros() as i32)
}

/// The cube a piece of `c`, `near` the eye, is drawn in at `tau`;
/// `outermost` where it is of the source's top, which no level over it
/// draws. Objects move a z16 piece's cube, never its pool: both of its cubes
/// finer than a z16 cell draw at step one. Nothing but the quality moves
/// it: a mesh with more boxes than a slot holds fills more slots, and is
/// never drawn coarser for it (world/chunk/mesh_slot.rs).
pub fn cube_of(c: Chunk, outermost: bool, near: f64, tau: Tau) -> Cube {
    drawn_cube(wanted_cube(near, tau), c.level(), outermost, false)
}

/// The pool a piece of `c` takes, drawn in `cube`.
pub fn pool_of(c: Chunk, cube: Cube) -> Pool {
    Pool::of(c.level(), mesh_step(cube, c.level()))
}

/// What the frontier can take of each pool at the quality `cut` is cut at,
/// as it is counted before anything is allocated (`frontier(O, K, τ)` in
/// the record): never less than the frontier holds on its way to that cut,
/// never less at a finer quality than at a coarser one, and never less with
/// looser bounds.
///
/// A goal piece takes a slot of its pool. On the way to it a refinement
/// draws the chunks over it a level at a time, each a chunk the cut splits,
/// and those are coarse: the chunks drawn on the way are an antichain, each
/// over some goal piece and none over another's, so in the coarse pool they
/// are never more than the goal's pieces over z16 and the z16 pieces'
/// parents. A z16 piece takes the fine pool where it is drawn at its own
/// cells and the coarse one where it is drawn coarser, which the quality and
/// what is known both move it between; so every z16 piece is counted in the
/// coarse pool whichever it takes, and in the fine one where it takes that.
///
/// Every quality's cut is made from the same footprints, and a finer one
/// holds errors to a smaller cell, so it splits every chunk a coarser one
/// does and more; a split never takes a piece away (domain.rs `cut_at`),
/// and a z16 piece wants no coarser a cube at a finer quality. So no count
/// is less at a finer quality, by the cuts themselves. With looser bounds
/// the same footprints touch no fewer chunks, a chunk's error spans no less,
/// and a split keeps no fewer of the chunks under it, so no count is less.
///
/// `slots` is how many slots a chunk's tier of a cube fills, as far as is
/// known: one until a bake of it has had more boxes than one holds. A piece
/// takes that many of its pool, and a z16 piece in the fine pool is counted
/// once in the coarse one too, as above.
pub fn need(d: &Domain, cut: &Cut, slots: impl Fn(Chunk, Cube) -> u32) -> Slots {
    let mut n = [0; POOLS];
    for &c in cut.pieces() {
        let cube = cube_of(c, d.outermost(c), d.nearest_to(c), cut.tau());
        let k = slots(c, cube);
        match pool_of(c, cube) {
            Pool::Coarse => n[Pool::Coarse as usize] += k,
            Pool::Fine => {
                n[Pool::Fine as usize] += k;
                n[Pool::Coarse as usize] += 1;
            }
        }
    }
    n[Pool::Coarse as usize] += cut.splits().filter(|c| c.level() == 1).count() as u32;
    n
}

/// What the frontier reads of the chunks it has not drawn yet.
pub trait Env {
    /// The pool a piece of `c` takes as the goal wants it drawn now, and how
    /// many of its slots, as far as is known.
    fn slots(&self, c: Chunk) -> (Pool, u32);
    /// Whether a page at or over `c` is resident, which is all a build of it
    /// needs.
    fn buildable(&self, c: Chunk) -> bool;
    /// Where `c` stands in the view's order: what moves come first.
    fn order(&self, c: Chunk) -> Order;
}

/// The frontier as it stands, and the room the arena has for it.
pub struct Now<'a> {
    /// The goal at the effective quality, sorted: an antichain.
    pub goal: &'a [Chunk],
    /// The pieces drawn: an antichain.
    pub drawn: &'a BTreeSet<Chunk>,
    /// The chunks built for moves under way, and whether each is ready.
    pub building: &'a BTreeMap<Chunk, bool>,
    /// Drawn pieces building a tier other than the one they show, each a
    /// refinement under way.
    pub recubing: u32,
    /// Drawn pieces whose tier should change and has not been let: each
    /// takes its slots of its pool beside the ones it shows.
    pub recube: &'a [(Chunk, (Pool, u32))],
    /// Slots each pool has that nothing holds and nothing already started
    /// will take.
    pub free: Slots,
}

/// The frontier's step: what to start and stop building, and the moves
/// whose pieces are all ready.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    /// Chunks to mount and build, in the order they were let.
    pub start: Vec<Chunk>,
    /// Chunks building that no move wants any more.
    pub cancel: Vec<Chunk>,
    /// Moves to make now: what leaves the frontier and what enters it.
    pub swaps: Vec<Swap>,
    /// Drawn pieces over no ground the goal has.
    pub drop: Vec<Chunk>,
    /// Drawn pieces let build the tier they want.
    pub recube: Vec<Chunk>,
    /// Moves that found no room this step.
    pub refused: u32,
    /// Moves under way and not all ready.
    pub waiting: u32,
}

/// A move of the frontier: `leave` goes and `enter` comes, at once.
#[derive(Debug, PartialEq)]
pub struct Swap {
    pub leave: Vec<Chunk>,
    pub enter: Vec<Chunk>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Coarsen,
    Fill,
    Refine,
}

struct Move {
    kind: Kind,
    order: Order,
    leave: Vec<Chunk>,
    enter: Vec<Chunk>,
}

/// The frontier's step toward `now.goal`: the moves the drawn pieces make
/// toward it, those whose pieces are all ready made, and of the rest, the
/// ones the arena has room for started — coarsenings first, since they give
/// room back, then fills, then refinements, each in the view's order.
pub fn plan(now: &Now, env: &impl Env) -> Plan {
    let goal: HashSet<Chunk> = now.goal.iter().copied().collect();
    let mut related: HashSet<Chunk> = HashSet::new();
    // A goal piece over drawn ones: the drawn pieces under it.
    let mut coarsen: BTreeMap<Chunk, Vec<Chunk>> = BTreeMap::new();
    for &d in now.drawn {
        if goal.contains(&d) {
            related.insert(d);
        } else if let Some(a) = d.ancestors().find(|a| goal.contains(a)) {
            coarsen.entry(a).or_default().push(d);
            related.insert(d);
        }
    }
    // A drawn piece over goal ones: the chunks a level under it on the way
    // to each. And a goal piece nothing drawn meets: a fill.
    let mut refine: BTreeMap<Chunk, BTreeSet<Chunk>> = BTreeMap::new();
    let mut fills = Vec::new();
    for &g in now.goal {
        if now.drawn.contains(&g) || coarsen.contains_key(&g) {
            continue;
        }
        let mut below = g;
        let mut over = None;
        for a in g.ancestors() {
            if now.drawn.contains(&a) {
                over = Some(a);
                break;
            }
            below = a;
        }
        match over {
            Some(d) => {
                refine.entry(d).or_default().insert(below);
                related.insert(d);
            }
            None => fills.push(g),
        }
    }

    let mut moves: Vec<Move> = Vec::new();
    moves.extend(coarsen.into_iter().map(|(a, leave)| Move {
        kind: Kind::Coarsen,
        order: env.order(a),
        leave,
        enter: vec![a],
    }));
    moves.extend(fills.into_iter().map(|g| Move {
        kind: Kind::Fill,
        order: env.order(g),
        leave: Vec::new(),
        enter: vec![g],
    }));
    moves.extend(refine.into_iter().map(|(d, steps)| Move {
        kind: Kind::Refine,
        order: env.order(d),
        leave: vec![d],
        enter: steps.into_iter().collect(),
    }));

    let mut plan = Plan {
        drop: now.drawn.iter().copied().filter(|d| !related.contains(d)).collect(),
        ..Plan::default()
    };
    let wanted: HashSet<Chunk> = moves.iter().flat_map(|m| m.enter.iter().copied()).collect();
    plan.cancel = now.building.keys().copied().filter(|c| !wanted.contains(c)).collect();

    let building = |c: &Chunk| now.building.contains_key(c);
    let mut in_flight = now.recubing;
    let mut open = Vec::new();
    for m in moves {
        if m.enter.iter().all(|c| now.building.get(c) == Some(&true)) {
            plan.swaps.push(Swap { leave: m.leave, enter: m.enter });
            continue;
        }
        let started = m.enter.iter().any(building);
        if started {
            plan.waiting += 1;
            if m.kind == Kind::Refine {
                in_flight += 1;
            }
        }
        if !m.enter.iter().all(building) {
            open.push(m);
        }
    }
    open.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.order.cmp(&b.order)));

    let mut free = now.free;
    let take = |free: &Slots, need: &Slots, reserve: Slots| (0..POOLS).all(|p| need[p] == 0 || free[p] >= need[p] + reserve[p]);
    for m in open {
        if plan.start.len() as u32 >= crate::param::STARTS_PER_FRAME {
            plan.refused += 1;
            continue;
        }
        let missing: Vec<Chunk> = m.enter.iter().copied().filter(|c| !building(c)).collect();
        let fresh = !m.enter.iter().any(building);
        let mut need = [0; POOLS];
        for &c in &missing {
            let (pool, k) = env.slots(c);
            need[pool as usize] += k;
        }
        let reserve = if m.kind == Kind::Coarsen { [0; POOLS] } else { RESERVE };
        let room = missing.iter().all(|&c| env.buildable(c)) && take(&free, &need, reserve) && !(m.kind == Kind::Refine && fresh && in_flight >= REFINEMENTS_IN_FLIGHT);
        if !room {
            plan.refused += 1;
            continue;
        }
        for p in 0..POOLS {
            free[p] -= need[p];
        }
        if m.kind == Kind::Refine && fresh {
            in_flight += 1;
        }
        plan.start.extend(missing);
    }

    let mut recube: Vec<(Chunk, (Pool, u32))> = now.recube.to_vec();
    recube.sort_by_key(|&(c, _)| env.order(c));
    for (c, (pool, k)) in recube {
        let mut need = [0; POOLS];
        need[pool as usize] = k;
        if in_flight >= REFINEMENTS_IN_FLIGHT || !take(&free, &need, RESERVE) {
            plan.refused += 1;
            continue;
        }
        free[pool as usize] -= k;
        in_flight += 1;
        plan.recube.push(c);
    }
    plan
}

#[cfg(test)]
#[path = "frontier.test.rs"]
mod test;
