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
/// for one coarsening: none in the fine pool, where no coarsening lands; in
/// the coarse pool the most a coarsening's piece can fill
/// (`COARSE_TIER_SLOTS`), a bound from the representation and not from
/// what has been baked so far; and none in the cover's, where a root not
/// drawn always finds its slot (mesh_slot.rs `COVER_SLOTS`).
pub const RESERVE: Slots = [0, COARSE_TIER_SLOTS, 0];

/// Slots each pool keeps past the frontier while it moves, which the quality
/// policy fits the goal beside: the reserve, which is progress, and a slot
/// for each refinement under way, holding the piece it replaces, which is
/// throughput. None in the cover's: a piece of the top a refinement holds
/// is its root's one slot there.
pub const HEADROOM: Slots = [REFINEMENTS_IN_FLIGHT + RESERVE[0], REFINEMENTS_IN_FLIGHT + RESERVE[1], 0];

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
/// A goal piece takes a slot of its pool; a piece of the top, the cover's.
/// On the way to it a refinement
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
            Pool::Cover => n[Pool::Cover as usize] += k,
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
    /// The pieces drawn, sorted: an antichain.
    pub drawn: &'a [Chunk],
    /// The chunks built for moves under way, and whether each is ready,
    /// sorted.
    pub building: &'a [(Chunk, bool)],
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
#[derive(Debug, Default)]
pub struct Plan {
    /// Chunks to mount and build, in the order they were let.
    pub start: Vec<Chunk>,
    /// Chunks building that no move wants any more.
    pub cancel: Vec<Chunk>,
    /// Drawn pieces over no ground the goal has.
    pub drop: Vec<Chunk>,
    /// Drawn pieces let build the tier they want.
    pub recube: Vec<Chunk>,
    /// Moves that found no room this step.
    pub refused: u32,
    /// Moves under way and not all ready.
    pub waiting: u32,
    /// The moves to make now, as what leaves and what enters, each a run of
    /// `chunks`.
    swaps: Vec<(Run, Run)>,
    /// Every move's pieces, each move's a run of them.
    chunks: Vec<Chunk>,
}

impl Plan {
    /// Moves to make now: what leaves the frontier and what enters it, at
    /// once.
    pub fn swaps(&self) -> impl Iterator<Item = (&[Chunk], &[Chunk])> {
        self.swaps.iter().map(|&(leave, enter)| (leave.of(&self.chunks), enter.of(&self.chunks)))
    }
}

/// A run of a plan's chunks.
#[derive(Clone, Copy, Debug, Default)]
struct Run {
    from: u32,
    to: u32,
}

impl Run {
    fn of(self, chunks: &[Chunk]) -> &[Chunk] {
        &chunks[self.from as usize..self.to as usize]
    }
}

/// `pieces` appended to `chunks`, as the run they take.
fn push(chunks: &mut Vec<Chunk>, pieces: impl IntoIterator<Item = Chunk>) -> Run {
    let from = chunks.len() as u32;
    chunks.extend(pieces);
    Run { from, to: chunks.len() as u32 }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Coarsen,
    Fill,
    Refine,
}

#[derive(Clone, Copy)]
struct Move {
    kind: Kind,
    order: Order,
    leave: Run,
    enter: Run,
}

/// The frontier's step and what it is worked out in, kept from one step to
/// the next for their capacity: a step over a frontier no larger than one
/// already planned allocates nothing. Nothing in it outlives a step.
#[derive(Default)]
pub struct Frontier {
    plan: Plan,
    moves: Vec<Move>,
    /// A goal piece, and a drawn piece under it.
    under: Vec<(Chunk, Chunk)>,
    /// A drawn piece, and the chunk a level under it on the way to a goal
    /// piece.
    over: Vec<(Chunk, Chunk)>,
    /// What the moves enter, sorted.
    wanted: Vec<Chunk>,
    /// The moves not all started, by index.
    open: Vec<u32>,
    recube: Vec<(Chunk, (Pool, u32))>,
}

impl Frontier {
    /// The frontier's step toward `now.goal`: the moves the drawn pieces
    /// make toward it, those whose pieces are all ready made, and of the
    /// rest, the ones the arena has room for started — coarsenings first,
    /// since they give room back, then fills, then refinements, each in the
    /// view's order.
    pub fn plan(&mut self, now: &Now, env: &impl Env) -> &Plan {
        let Frontier {
            plan,
            moves,
            under,
            over,
            wanted,
            open,
            recube,
        } = self;
        let Plan {
            start,
            cancel,
            drop,
            recube: recubed,
            refused,
            waiting,
            swaps,
            chunks,
        } = plan;
        for v in [&mut *start, &mut *cancel, &mut *drop, &mut *recubed, &mut *chunks] {
            v.clear();
        }
        swaps.clear();
        (*refused, *waiting) = (0, 0);
        moves.clear();
        under.clear();
        over.clear();
        wanted.clear();
        open.clear();
        recube.clear();

        let in_goal = |c: &Chunk| now.goal.binary_search(c).is_ok();
        let drawn = |c: &Chunk| now.drawn.binary_search(c).is_ok();
        let ready = |c: &Chunk| now.building.binary_search_by_key(c, |&(b, _)| b).ok().map(|i| now.building[i].1);
        let building = |c: &Chunk| ready(c).is_some();

        // A goal piece over drawn ones: the drawn pieces under it. A drawn
        // piece neither in the goal nor under a piece of it is dropped,
        // unless it is over one, which the refinements below say.
        for &d in now.drawn {
            if in_goal(&d) {
                continue;
            }
            match d.ancestors().find(in_goal) {
                Some(a) => under.push((a, d)),
                None => drop.push(d),
            }
        }
        under.sort_unstable();
        for group in under.chunk_by(|x, y| x.0 == y.0) {
            let a = group[0].0;
            let leave = push(chunks, group.iter().map(|&(_, d)| d));
            let enter = push(chunks, [a]);
            moves.push(Move {
                kind: Kind::Coarsen,
                order: env.order(a),
                leave,
                enter,
            });
        }
        // A drawn piece over goal ones: the chunks a level under it on the
        // way to each. And a goal piece nothing drawn meets: a fill.
        for &g in now.goal {
            if drawn(&g) || under.binary_search_by_key(&g, |&(a, _)| a).is_ok() {
                continue;
            }
            let mut below = g;
            let mut above = None;
            for a in g.ancestors() {
                if drawn(&a) {
                    above = Some(a);
                    break;
                }
                below = a;
            }
            match above {
                Some(d) => over.push((d, below)),
                None => {
                    let enter = push(chunks, [g]);
                    moves.push(Move {
                        kind: Kind::Fill,
                        order: env.order(g),
                        leave: Run::default(),
                        enter,
                    });
                }
            }
        }
        over.sort_unstable();
        over.dedup();
        for group in over.chunk_by(|x, y| x.0 == y.0) {
            let d = group[0].0;
            let leave = push(chunks, [d]);
            let enter = push(chunks, group.iter().map(|&(_, s)| s));
            moves.push(Move {
                kind: Kind::Refine,
                order: env.order(d),
                leave,
                enter,
            });
        }
        drop.retain(|d| over.binary_search_by_key(d, |&(o, _)| o).is_err());

        for m in moves.iter() {
            wanted.extend_from_slice(m.enter.of(chunks));
        }
        wanted.sort_unstable();
        cancel.extend(now.building.iter().map(|&(c, _)| c).filter(|c| wanted.binary_search(c).is_err()));

        let mut in_flight = now.recubing;
        for (n, m) in moves.iter().enumerate() {
            let enter = m.enter.of(chunks);
            if enter.iter().all(|c| ready(c) == Some(true)) {
                swaps.push((m.leave, m.enter));
                continue;
            }
            if enter.iter().any(building) {
                *waiting += 1;
                if m.kind == Kind::Refine {
                    in_flight += 1;
                }
            }
            if !enter.iter().all(building) {
                open.push(n as u32);
            }
        }
        open.sort_unstable_by(|&a, &b| {
            let (x, y) = (&moves[a as usize], &moves[b as usize]);
            x.kind.cmp(&y.kind).then(x.order.cmp(&y.order)).then(a.cmp(&b))
        });

        let mut free = now.free;
        let take = |free: &Slots, need: &Slots, reserve: Slots| (0..POOLS).all(|p| need[p] == 0 || free[p] >= need[p] + reserve[p]);
        for &n in open.iter() {
            let m = moves[n as usize];
            if start.len() as u32 >= crate::param::STARTS_PER_FRAME {
                *refused += 1;
                continue;
            }
            let enter = m.enter.of(chunks);
            let missing = || enter.iter().copied().filter(|c| !building(c));
            let fresh = !enter.iter().any(building);
            let mut need = [0; POOLS];
            for c in missing() {
                let (pool, k) = env.slots(c);
                need[pool as usize] += k;
            }
            let reserve = if m.kind == Kind::Coarsen { [0; POOLS] } else { RESERVE };
            let room = missing().all(|c| env.buildable(c)) && take(&free, &need, reserve) && !(m.kind == Kind::Refine && fresh && in_flight >= REFINEMENTS_IN_FLIGHT);
            if !room {
                *refused += 1;
                continue;
            }
            for p in 0..POOLS {
                free[p] -= need[p];
            }
            if m.kind == Kind::Refine && fresh {
                in_flight += 1;
            }
            start.extend(missing());
        }

        recube.extend_from_slice(now.recube);
        // The chunk breaks a tie in the view's order and ranks nothing: it
        // only makes the order total, so which of two pieces the view cannot
        // tell apart is let first does not follow the table's history.
        recube.sort_unstable_by_key(|&(c, _)| (env.order(c), c));
        for &(c, (pool, k)) in recube.iter() {
            let mut need = [0; POOLS];
            need[pool as usize] = k;
            if in_flight >= REFINEMENTS_IN_FLIGHT || !take(&free, &need, RESERVE) {
                *refused += 1;
                continue;
            }
            free[pool as usize] -= k;
            in_flight += 1;
            recubed.push(c);
        }
        plan
    }
}

#[cfg(test)]
#[path = "frontier.test.rs"]
mod test;
