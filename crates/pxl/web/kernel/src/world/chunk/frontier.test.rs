// pxl-kernel/world/chunk/frontier.test.rs
//
// The frontier's laws on fixtures: flat ground a chunk thick, where the
// pieces are the chunks of one square of ground at whatever levels a goal
// says, driven by a table of its own — builds that land after a few steps,
// slots counted by pool — against arenas too small for the goals asked of
// them. The same table driven by the goal an eye observes of a world known
// exactly (world/fixture.test.rs), from before the floor is resident to a
// turn toward ground never seen. And the count the quality policy decides
// by, on that world, known exactly and loosened.

use std::collections::{BTreeMap, BTreeSet};

use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::Pvf3;
use pxl_spec::space::pyramid::chunk_span_of;
use pxl_spec::space::v3::V3;

use super::*;

use crate::allocations;
use crate::world::fixture::{self, eye, World};

/// An object's cube and the ground's are one size wherever both have data:
/// only nearer than a z16 cell's reach do a z16 chunk's objects go finer,
/// down to their finest tier, while the ground stays at its atom.
#[test]
fn objects_draw_the_grounds_cube_and_go_finer_only_under_z16() {
    let z16 = GROUND.scale(0);
    for shamt in -6..=8 {
        let cube = Scale::of(shamt);
        let (ground, objects) = (drawn_cube(cube, 0, false, false), drawn_cube(cube, 0, false, true));
        if cube >= z16 {
            assert_eq!(objects, ground, "{cube:?}");
        } else {
            assert_eq!(ground, z16, "{cube:?}");
            assert_eq!(objects, cube.max(OBJECT_FINEST), "{cube:?}");
        }
    }
}

// ---- the frontier ----

/// The level the fixture's square is a chunk of: 16 × 16 z16 chunks.
const ROOT: u32 = 4;

/// The chunk of `level` over the fixture's ground `x`, `y` z16 chunks from
/// its corner, at the ground's height.
fn at(level: u32, x: i64, y: i64) -> Chunk {
    let side = chunk_span_of(0) as i64;
    Chunk::of(
        level,
        &Pvf3 {
            i: ivec::gather(x * side, y * side, 0),
            f: 0,
        },
    )
}

/// Every chunk of `level` over the fixture's ground.
fn level(l: u32) -> Vec<Chunk> {
    let n = 1i64 << (ROOT - l);
    let step = 1i64 << l;
    let mut out: Vec<Chunk> = (0..n).flat_map(|y| (0..n).map(move |x| at(l, x * step, y * step))).collect();
    out.sort_unstable();
    out
}

/// A goal: the ground at `fine` inside the square of z16 chunks `[0, w)`, and
/// at `coarse` outside it, or where `w` is zero, at `coarse` everywhere.
fn goal(fine: u32, coarse: u32, w: i64) -> Vec<Chunk> {
    let inside = |c: &Chunk| {
        let (x, y, side) = c.rect();
        let unit = chunk_span_of(0);
        x + side <= w as f64 * unit && y + side <= w as f64 * unit
    };
    let mut out: Vec<Chunk> = level(fine).into_iter().filter(|c| inside(c)).collect();
    let covered = |c: &Chunk| out.iter().any(|f| f.ancestors().any(|a| a == *c));
    let rest: Vec<Chunk> = level(coarse).into_iter().filter(|c| !covered(c) && !inside(c)).collect();
    out.extend(rest);
    out.sort_unstable();
    out
}

/// The fixture's view of the pieces: z16 chunks in the fine pool, every
/// other in the coarse, each a slot, everything resident, nearer the corner
/// first.
struct Flat;

impl Env for Flat {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        if c.level() == 0 {
            (Pool::Fine, 1)
        } else {
            (Pool::Coarse, 1)
        }
    }

    fn buildable(&self, _c: Chunk) -> bool {
        true
    }

    fn order(&self, c: Chunk) -> Order {
        let (x, y, side) = c.rect();
        Order {
            seen: true,
            near: (x + side / 2.0).hypot(y + side / 2.0),
        }
    }
}

/// The fixture's table: what is drawn, what is building and how many steps
/// each has left before it is ready, and the arena.
struct Table {
    drawn: BTreeSet<Chunk>,
    building: BTreeMap<Chunk, u32>,
    arena: Slots,
    seed: u64,
    frontier: Frontier,
    /// What the frontier's steps have allocated.
    allocated: u64,
    /// The instant, in milliseconds: a hold's length a step, so a
    /// coarsening asked for in one step is let in the next.
    ms: f64,
}

impl Table {
    fn new(arena: Slots) -> Table {
        Table {
            drawn: BTreeSet::new(),
            building: BTreeMap::new(),
            arena,
            seed: 0x2545_f491,
            frontier: Frontier::default(),
            allocated: 0,
            ms: 0.0,
        }
    }

    /// Slots each pool holds: every piece drawn, and every chunk building,
    /// whose slot is held for it from the moment it starts.
    fn held(&self) -> Slots {
        let mut held = [0; POOLS];
        for &c in self.drawn.iter().chain(self.building.keys()) {
            let (pool, k) = Flat.slots(c);
            held[pool as usize] += k;
        }
        held
    }

    /// One step toward `goal`: the plan made, what it says done, and the
    /// builds a step nearer landing.
    fn step(&mut self, goal: &[Chunk]) {
        let drawn: Vec<Chunk> = self.drawn.iter().copied().collect();
        let ready: Vec<(Chunk, bool)> = self.building.iter().map(|(&c, &left)| (c, left == 0)).collect();
        let held = self.held();
        let free = core::array::from_fn(|p| self.arena[p] - held[p]);
        self.ms += crate::param::COARSEN_HOLD_MS;
        let now = Now {
            ms: self.ms,
            goal,
            drawn: &drawn,
            building: &ready,
            recubing: 0,
            recube: &[],
            free,
        };
        let (allocated, plan) = allocations::during(|| self.frontier.plan(&now, &Flat));
        self.allocated += allocated;
        for c in plan.cancel.iter() {
            self.building.remove(c);
        }
        for c in plan.drop.iter() {
            self.drawn.remove(c);
        }
        for (leave, enter) in plan.swaps() {
            for c in leave {
                assert!(self.drawn.remove(c), "{c:?} left the frontier without being drawn");
            }
            for &c in enter {
                assert_eq!(self.building.remove(&c), Some(0), "{c:?} entered the frontier before it was ready");
                self.drawn.insert(c);
            }
        }
        for &c in plan.start.iter() {
            self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            assert!(self.building.insert(c, 1 + (self.seed >> 40) as u32 % 4).is_none(), "{c:?} started twice");
        }
        for left in self.building.values_mut() {
            *left = left.saturating_sub(1);
        }
    }
}

/// The fixture's pieces with every z16 chunk's tier filling `0` slots.
struct Heavy(u32);

impl Env for Heavy {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let (pool, k) = Flat.slots(c);
        (pool, if c.level() == 0 { self.0 } else { k })
    }

    fn buildable(&self, c: Chunk) -> bool {
        Flat.buildable(c)
    }

    fn order(&self, c: Chunk) -> Order {
        Flat.order(c)
    }
}

/// A piece whose tier fills several slots is started only with all of them
/// free past the reserve, and never with some of them: a refinement into
/// eight z16 chunks of three slots each waits for twenty-four and the
/// reserve, however many fewer are free.
#[test]
fn a_piece_filling_several_slots_starts_only_with_all_of_them_free() {
    let parent = at(1, 0, 0);
    let children: Vec<Chunk> = {
        let mut c = parent.children().to_vec();
        c.sort_unstable();
        c
    };
    let drawn = [parent];
    let with = |fine: u32| {
        let now = Now {
            ms: 0.0,
            goal: &children,
            drawn: &drawn,
            building: &[],
            recubing: 0,
            recube: &[],
            free: [fine, 100, 0],
        };
        Frontier::default().plan(&now, &Heavy(3)).start.len() as u32
    };
    let occupied = children.iter().filter(|c| c.level() == 0).count() as u32;
    assert!(occupied > 0);
    let all = 3 * occupied + RESERVE[Pool::Fine as usize];
    assert_eq!(with(all - 1), 0, "started with fewer slots free than its pieces fill");
    assert_eq!(with(all), occupied, "not started with every slot its pieces fill free");
}

/// A coarsening waits until the goal has asked for it for its hold, so a
/// goal that comes back within it finds the finer pieces still drawn; a
/// step that found the arena short lets it go at once.
#[test]
fn a_coarsening_waits_its_hold_unless_the_arena_is_short() {
    fn now<'a>(ms: f64, goal: &'a [Chunk], drawn: &'a [Chunk], free: Slots) -> Now<'a> {
        Now {
            ms,
            goal,
            drawn,
            building: &[],
            recubing: 0,
            recube: &[],
            free,
        }
    }
    let parent = at(1, 0, 0);
    let mut children = parent.children().to_vec();
    children.sort_unstable();
    let (hold, roomy, goal) = (crate::param::COARSEN_HOLD_MS, [100; POOLS], [parent]);
    let mut f = Frontier::default();
    assert!(f.plan(&now(0.0, &goal, &children, roomy), &Flat).start.is_empty(), "coarsened the moment it was asked for");
    assert!(f.plan(&now(hold / 2.0, &goal, &children, roomy), &Flat).start.is_empty(), "coarsened inside its hold");
    assert_eq!(f.plan(&now(hold, &goal, &children, roomy), &Flat).start, vec![parent], "not coarsened past its hold");

    // Asked for again after the goal let it go: it waits again.
    let mut f = Frontier::default();
    f.plan(&now(0.0, &goal, &children, roomy), &Flat);
    f.plan(&now(hold / 2.0, &children, &children, roomy), &Flat);
    assert!(f.plan(&now(hold, &goal, &children, roomy), &Flat).start.is_empty(), "a coarsening asked for again kept its first time");

    // A step that refused a move room: the next coarsens at once.
    let mut f = Frontier::default();
    f.plan(&now(0.0, &[parent, at(1, 4, 4)], &children, [0; POOLS]), &Flat);
    assert_eq!(f.plan(&now(1.0, &goal, &children, roomy), &Flat).start, vec![parent], "held a coarsening while the arena was short");
}

/// The fixture's pieces with every piece over z16 filling the most a coarse
/// tier can.
struct Widest;

impl Env for Widest {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let (pool, k) = Flat.slots(c);
        (pool, if c.level() > 0 { COARSE_TIER_SLOTS } else { k })
    }

    fn buildable(&self, c: Chunk) -> bool {
        Flat.buildable(c)
    }

    fn order(&self, c: Chunk) -> Order {
        Flat.order(c)
    }
}

/// One coarsening can always be built: with the coarse pool down to the
/// reserve, a piece over z16 filling the most a coarse tier can — however
/// its mesh came to be that large — is started, where a refinement or a fill
/// is not.
#[test]
fn a_coarsening_is_always_started_with_only_the_reserve_free() {
    let parent = at(1, 0, 0);
    let mut children = parent.children().to_vec();
    children.sort_unstable();
    let now = Now {
        ms: 0.0,
        goal: &[parent],
        drawn: &children,
        building: &[],
        recubing: 0,
        recube: &[],
        free: RESERVE,
    };
    // Asked for past its hold.
    let mut frontier = Frontier::default();
    frontier.plan(&now, &Widest);
    let later = Now {
        ms: crate::param::COARSEN_HOLD_MS,
        ..now
    };
    assert_eq!(frontier.plan(&later, &Widest).start, vec![parent], "a coarsening not started with the reserve free");
    let fill = Now {
        goal: &[at(1, 4, 4)],
        drawn: &[],
        ..now
    };
    assert!(Frontier::default().plan(&fill, &Widest).start.is_empty(), "a fill took the reserve");
}

/// A step over a frontier no larger than one already planned allocates
/// nothing: moving through goals it has moved through before, the frontier
/// works in what it held from then.
#[test]
fn planning_a_frontier_it_has_held_allocates_nothing() {
    let mut t = Table::new([300, 200, 0]);
    let goals = [goal(3, 3, 0), goal(0, 2, 16), goal(1, 3, 8), goal(0, 0, 16), goal(2, 3, 8), goal(0, 1, 8)];
    let pass = |t: &mut Table| {
        t.allocated = 0;
        for g in &goals {
            for _ in 0..200 {
                t.step(g);
            }
        }
        t.allocated
    };
    assert!(pass(&mut t) > 0, "the first pass allocated nothing: the count measures nothing");
    pass(&mut t);
    assert_eq!(pass(&mut t), 0, "a pass through goals already planned allocated");
}

// ---- the goal an eye observes ----

/// The height of the fixture world's ground.
const LAND: f64 = 10.5;
const EYE: V3 = [1_000.5, 2_000.25, LAND + 2.0];
const FAR: f64 = 20_000.0;

impl Table {}

// ---- the count ----

/// A world with some relief, known as tightly or loosely as `slack`.
fn relief(slack: f64) -> World {
    let mut w = World::flat(LAND);
    for k in 0..8 {
        let x = 1_050.0 + 170.0 * k as f64;
        w = w.with([x, 1_700.0 + 40.0 * k as f64, LAND], [x + 60.0, 2_500.0, LAND + 30.0 * (k + 1) as f64]);
    }
    w.slack = slack;
    w
}

fn ladder() -> Vec<Tau> {
    (0..8).map(|k| Tau::of(2f64.powf(k as f64 / 2.0))).collect()
}

fn domain_at(height: f64, world: &World) -> Domain {
    let at = [EYE[0], EYE[1], LAND + height];
    fixture::domain(eye(at, Some([1.0, 0.05, -0.25]), FAR, LAND), &ladder(), world)
}

/// Every step's count, finest first.
fn counts(d: &mut Domain) -> Vec<Slots> {
    (0..d.ladder().len())
        .map(|k| {
            d.cut_at(k);
            need(d, d.cut(k).expect("made above"), |_, _| 1)
        })
        .collect()
}

/// The count never asks less at a finer quality than at a coarser one, and
/// never less than the cut at that quality is.
#[test]
fn the_count_is_never_less_at_a_finer_quality_nor_less_than_the_cut() {
    for height in [3.0, 150.0, 2_000.0] {
        let mut d = domain_at(height, &relief(0.0));
        let n = counts(&mut d);
        for k in 0..n.len() {
            if let Some(coarser) = n.get(k + 1) {
                for (p, (finer, coarser)) in n[k].iter().zip(coarser).enumerate() {
                    assert!(finer >= coarser, "height {height}: pool {p} asks less at step {k} than coarser");
                }
            }
            let cut = d.cut_at(k);
            let (tau, pieces) = (cut.tau(), cut.pieces().to_vec());
            let of = |pool: Pool| pieces.iter().filter(|&&c| pool_of(c, cube_of(c, d.outermost(c), d.nearest_to(c), tau)) == pool).count() as u32;
            assert!(
                n[k][Pool::Coarse as usize] + n[k][Pool::Cover as usize] >= pieces.len() as u32,
                "height {height}, {tau:?}: coarse under the cut"
            );
            assert!(n[k][Pool::Fine as usize] >= of(Pool::Fine), "height {height}, {tau:?}: fine under the cut");
            assert!(n[k][Pool::Cover as usize] >= of(Pool::Cover), "height {height}, {tau:?}: cover under the cut");
        }
    }
}
