// pxl-kernel/world/chunk/frontier.rs

//! Materialization: what is drawn, from the domain's goal, the resident
//! records and the arena.
//!
//! The drawn pieces are an antichain of the chunk tree covering the goal with
//! no gap. They move only by whole moves, made once everything they bring is
//! ready: a refinement (a piece replaced by its children, a zoom at a time),
//! a coarsening (pieces replaced by their goal parent), or a fill (the
//! coarsest piece that can be built over a goal piece with nothing drawn on
//! its line, refined from there as any drawn piece is). Until then the coarser
//! piece stays drawn, so a full arena or a missing tile is latency, never a
//! hole.
//!
//! The arena is never overrun: a move starts only when all its slots are
//! free, and refinements and fills leave `RESERVE` free so one coarsening is
//! always possible. A coarsening waits `COARSEN_HOLD_MS` before it is made,
//! unless the arena refused a move last step.

use pxl_spec::space::lod::Tau;

use crate::param::REFINEMENTS_IN_FLIGHT;
use crate::world::chunk::mesh_slot::{Pool, ARENA, COARSE_TIER_SLOTS, POOLS};
use crate::world::observation::Order;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::Price;
use crate::world::quality::fits;

pub type Slots = [u32; POOLS];

/// Kept free past every non-coarsening move for one coarsening, which lands
/// in the coarse pool and fills at most `COARSE_TIER_SLOTS`.
pub const RESERVE: Slots = [0, COARSE_TIER_SLOTS];

/// What the quality policy leaves beside the goal: the reserve, plus a slot
/// for each refinement in flight holding the piece it replaces.
pub const HEADROOM: Slots = [REFINEMENTS_IN_FLIGHT + RESERVE[0], REFINEMENTS_IN_FLIGHT + RESERVE[1]];

/// The quality alone moves which chunks are drawn; a mesh too big for one
/// slot fills more slots and is never drawn coarser for it.
pub fn pool_of(c: Chunk) -> Pool {
    Pool::of(c.cell_zoom())
}

/// A coarse chunk whose children are fine: the last coarse one on its line.
/// One child stands for all: a pool is its zoom's alone.
fn into_fine(c: Chunk) -> bool {
    pool_of(c) == Pool::Coarse && pool_of(c.children()[0]) == Pool::Fine
}

/// Slots per pool, counted before allocation. A coarse chunk split into fine
/// ones keeps a coarse slot, the one it is drawn in while they are built, so a
/// finer quality never asks for less of a pool.
pub struct Need<F> {
    pub slots: F,
    /// stop counting a step past the arena
    pub admit: bool,
}

impl<F: Fn(Chunk) -> u32> Price for Need<F> {
    fn piece(&self, c: Chunk, _near: f64, _tau: Tau) -> Slots {
        let k = (self.slots)(c);
        match pool_of(c) {
            Pool::Coarse => [0, k],
            Pool::Fine => [k, 0],
        }
    }

    fn split(&self, c: Chunk) -> Slots {
        [0, u32::from(into_fine(c))]
    }

    fn over(&self, n: &Slots) -> bool {
        self.admit && !fits(*n, ARENA, HEADROOM, [0; POOLS])
    }
}

pub trait Env {
    fn slots(&self, c: Chunk) -> (Pool, u32);
    fn buildable(&self, c: Chunk) -> bool;
    fn order(&self, c: Chunk) -> Order;
}

pub struct Now<'a> {
    pub ms: f64,
    pub goal: &'a [Chunk],
    pub drawn: &'a [Chunk],
    pub building: &'a [(Chunk, bool)],
    /// Free slots after what started moves will take.
    pub free: Slots,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub start: Vec<Chunk>,
    pub cancel: Vec<Chunk>,
    pub drop: Vec<Chunk>,
    pub refused: u32,
    pub waiting: u32,
    /// Coarsenings waiting their hold; a turn is requested while any does, or
    /// nothing would wake it.
    pub held: u32,
    pub refined: u32,
    swaps: Vec<(Run, Run)>,
    chunks: Vec<Chunk>,
}

impl Plan {
    pub fn swaps(&self) -> impl Iterator<Item = (&[Chunk], &[Chunk])> {
        self.swaps.iter().map(|&(leave, enter)| (leave.of(&self.chunks), enter.of(&self.chunks)))
    }
}

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

/// Kept between steps for capacity. Only `asked` and `short` carry state.
#[derive(Default)]
pub struct Frontier {
    /// Since when the goal has asked for each coarsening's target.
    asked: Vec<(Chunk, f64)>,
    asking: Vec<(Chunk, f64)>,
    short: bool,
    plan: Plan,
    moves: Vec<Move>,
    under: Vec<(Chunk, Chunk)>,
    over: Vec<(Chunk, Chunk)>,
    wanted: Vec<Chunk>,
    /// Every piece over a drawn one, which a fill must not take.
    ancestry: Vec<Chunk>,
    fills: Vec<Chunk>,
    open: Vec<u32>,
}

impl Frontier {
    /// Ready moves are made; the rest are started where there is room:
    /// coarsenings first (they give room back), then fills, then refinements,
    /// each in view order.
    pub fn plan(&mut self, now: &Now, env: &impl Env) -> &Plan {
        let Frontier {
            asked,
            asking,
            short,
            plan,
            moves,
            under,
            over,
            wanted,
            ancestry,
            fills,
            open,
        } = self;
        let Plan {
            start,
            cancel,
            drop,
            refused,
            waiting,
            held: held_n,
            refined,
            swaps,
            chunks,
        } = plan;
        for v in [&mut *start, &mut *cancel, &mut *drop, &mut *chunks] {
            v.clear();
        }
        swaps.clear();
        (*refused, *waiting, *held_n, *refined) = (0, 0, 0, 0);
        moves.clear();
        under.clear();
        over.clear();
        wanted.clear();
        ancestry.clear();
        fills.clear();
        open.clear();

        let in_goal = |c: &Chunk| now.goal.binary_search(c).is_ok();
        let drawn = |c: &Chunk| now.drawn.binary_search(c).is_ok();
        let ready = |c: &Chunk| now.building.binary_search_by_key(c, |&(b, _)| b).ok().map(|i| now.building[i].1);
        let building = |c: &Chunk| ready(c).is_some();

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
        ancestry.extend(now.drawn.iter().flat_map(|d| d.ancestors()));
        ancestry.sort_unstable();
        ancestry.dedup();
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
                // The coarsest is the cheapest to build and comes first; the
                // goal is reached by refining it.
                None => fills.push(
                    core::iter::once(g)
                        .chain(g.ancestors())
                        .filter(|a| ancestry.binary_search(a).is_err() && env.buildable(*a))
                        .last()
                        .unwrap_or(g),
                ),
            }
        }
        fills.sort_unstable();
        fills.dedup();
        for &f in fills.iter() {
            let enter = push(chunks, [f]);
            moves.push(Move {
                kind: Kind::Fill,
                order: env.order(f),
                leave: Run::default(),
                enter,
            });
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

        // A coarsening the goal stops asking for is forgotten, so if asked again it waits again.
        asking.clear();
        asking.extend(moves.iter().filter(|m| m.kind == Kind::Coarsen).map(|m| {
            let a = m.enter.of(chunks)[0];
            let t = asked.binary_search_by_key(&a, |&(c, _)| c).map_or(now.ms, |i| asked[i].1);
            (a, t)
        }));
        asking.sort_unstable_by_key(|&(c, _)| c);
        core::mem::swap(asked, asking);
        let pressed = *short;
        let held = |m: &Move| {
            m.kind == Kind::Coarsen
                && !pressed
                && asked
                    .binary_search_by_key(&m.enter.of(chunks)[0], |&(c, _)| c)
                    .is_ok_and(|i| now.ms - asked[i].1 < crate::param::COARSEN_HOLD_MS)
        };

        let mut in_flight = 0;
        for (n, m) in moves.iter().enumerate() {
            if held(m) {
                *held_n += 1;
                continue;
            }
            let enter = m.enter.of(chunks);
            if enter.iter().all(|c| ready(c) == Some(true)) {
                swaps.push((m.leave, m.enter));
                *refined += (m.kind == Kind::Refine) as u32;
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
        let mut cramped = false;
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
            let fits = take(&free, &need, reserve);
            cramped |= !fits;
            let room = missing().all(|c| env.buildable(c)) && fits && !(m.kind == Kind::Refine && fresh && in_flight >= REFINEMENTS_IN_FLIGHT);
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

        *short = cramped;
        plan
    }
}
