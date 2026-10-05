// pxl-kernel/world/chunk/frontier.test.rs

use std::collections::{BTreeMap, BTreeSet};

use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_span_of, LEAF_Z};
use pxl_spec::space::v3::V3;

use super::*;

use crate::allocations;
use crate::world::fixture::{self, eye, World};
use crate::world::pyramid::domain::Domain;

/// Objects go finer than the ground's scale only within the leaves' reach.
#[test]
fn objects_draw_the_grounds_cube_and_go_finer_only_under_the_leaves() {
    let leaf = GROUND.side_at(LEAF_Z);
    for shamt in -6..=8 {
        let scale = Scale::of(shamt);
        let (ground, objects) = (drawn_scale(scale, LEAF_Z, false, false), drawn_scale(scale, LEAF_Z, false, true));
        if scale >= leaf {
            assert_eq!(objects, ground, "{scale:?}");
        } else {
            assert_eq!(ground, leaf, "{scale:?}");
            assert_eq!(objects, scale.max(OBJECT_FINEST), "{scale:?}");
        }
    }
}

/// 16 × 16 leaf chunks.
const ROOT: u32 = 4;

fn at(z: u32, x: i64, y: i64) -> Chunk {
    let side = chunk_span_of(LEAF_Z) as i64;
    Chunk::of(
        z,
        &Pvf3 {
            i: ivec::gather(x * side, y * side, 0),
            f: 0,
        },
    )
}

/// Every chunk at `z` over the root's ground.
fn chunks_at(z: u32) -> Vec<Chunk> {
    let steps = LEAF_Z - z;
    let n = 1i64 << (ROOT - steps);
    let step = 1i64 << steps;
    let mut out: Vec<Chunk> = (0..n).flat_map(|y| (0..n).map(move |x| at(z, x * step, y * step))).collect();
    out.sort_unstable();
    out
}

/// The ground at `fine` inside the square `[0, w)` of leaf chunks and at
/// `coarse` outside it; all `coarse` where `w` is zero.
fn goal(fine: u32, coarse: u32, w: i64) -> Vec<Chunk> {
    let inside = |c: &Chunk| {
        let (x, y, side) = c.rect();
        let unit = chunk_span_of(LEAF_Z);
        x + side <= w as f64 * unit && y + side <= w as f64 * unit
    };
    let mut out: Vec<Chunk> = chunks_at(fine).into_iter().filter(|c| inside(c)).collect();
    let covered = |c: &Chunk| out.iter().any(|f| f.ancestors().any(|a| a == *c));
    let rest: Vec<Chunk> = chunks_at(coarse).into_iter().filter(|c| !covered(c) && !inside(c)).collect();
    out.extend(rest);
    out.sort_unstable();
    out
}

/// leaf chunks in the fine pool, every other in the coarse, one slot each,
/// all resident, nearer the corner first.
struct Flat;

impl Env for Flat {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        if c.z() == LEAF_Z {
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
            near: libm::hypot(x + side / 2.0, y + side / 2.0),
        }
    }
}

struct Table {
    drawn: BTreeSet<Chunk>,
    building: BTreeMap<Chunk, u32>,
    arena: Slots,
    seed: u64,
    frontier: Frontier,
    allocated: u64,
    /// One hold's length a step, so a coarsening asked in one step is let in the next.
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

    /// Building chunks hold their slots from the moment they start.
    fn held(&self) -> Slots {
        let mut held = [0; POOLS];
        for &c in self.drawn.iter().chain(self.building.keys()) {
            let (pool, k) = Flat.slots(c);
            held[pool as usize] += k;
        }
        held
    }

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
            retier: &[],
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

struct Heavy(u32);

impl Env for Heavy {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let (pool, k) = Flat.slots(c);
        (pool, if c.z() == LEAF_Z { self.0 } else { k })
    }

    fn buildable(&self, c: Chunk) -> bool {
        Flat.buildable(c)
    }

    fn order(&self, c: Chunk) -> Order {
        Flat.order(c)
    }
}

/// A refinement into eight leaf chunks of three slots each waits for all
/// twenty-four plus the reserve.
#[test]
fn a_piece_filling_several_slots_starts_only_with_all_of_them_free() {
    let parent = at(LEAF_Z - 1, 0, 0);
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
            retier: &[],
            free: [fine, 100, 0],
        };
        Frontier::default().plan(&now, &Heavy(3)).start.len() as u32
    };
    let occupied = children.iter().filter(|c| c.z() == LEAF_Z).count() as u32;
    assert!(occupied > 0);
    let all = 3 * occupied + RESERVE[Pool::Fine as usize];
    assert_eq!(with(all - 1), 0, "started with fewer slots free than its pieces fill");
    assert_eq!(with(all), occupied, "not started with every slot its pieces fill free");
}

#[test]
fn a_coarsening_waits_its_hold_unless_the_arena_is_short() {
    fn now<'a>(ms: f64, goal: &'a [Chunk], drawn: &'a [Chunk], free: Slots) -> Now<'a> {
        Now {
            ms,
            goal,
            drawn,
            building: &[],
            recubing: 0,
            retier: &[],
            free,
        }
    }
    let parent = at(LEAF_Z - 1, 0, 0);
    let mut children = parent.children().to_vec();
    children.sort_unstable();
    let (hold, roomy, goal) = (crate::param::COARSEN_HOLD_MS, [100; POOLS], [parent]);
    let mut f = Frontier::default();
    assert!(f.plan(&now(0.0, &goal, &children, roomy), &Flat).start.is_empty(), "coarsened the moment it was asked for");
    assert!(f.plan(&now(hold / 2.0, &goal, &children, roomy), &Flat).start.is_empty(), "coarsened inside its hold");
    assert_eq!(f.plan(&now(hold, &goal, &children, roomy), &Flat).start, vec![parent], "not coarsened past its hold");

    let mut f = Frontier::default();
    f.plan(&now(0.0, &goal, &children, roomy), &Flat);
    f.plan(&now(hold / 2.0, &children, &children, roomy), &Flat);
    assert!(f.plan(&now(hold, &goal, &children, roomy), &Flat).start.is_empty(), "a coarsening asked for again kept its first time");

    let mut f = Frontier::default();
    f.plan(&now(0.0, &[parent, at(LEAF_Z - 1, 4, 4)], &children, [0; POOLS]), &Flat);
    assert_eq!(f.plan(&now(1.0, &goal, &children, roomy), &Flat).start, vec![parent], "held a coarsening while the arena was short");
}

struct Widest;

impl Env for Widest {
    fn slots(&self, c: Chunk) -> (Pool, u32) {
        let (pool, k) = Flat.slots(c);
        (pool, if c.z() < LEAF_Z { COARSE_TIER_SLOTS } else { k })
    }

    fn buildable(&self, c: Chunk) -> bool {
        Flat.buildable(c)
    }

    fn order(&self, c: Chunk) -> Order {
        Flat.order(c)
    }
}

/// With the coarse pool down to the reserve, a coarsening of the largest
/// coarse tier still starts; a refinement or a fill does not.
#[test]
fn a_coarsening_is_always_started_with_only_the_reserve_free() {
    let parent = at(LEAF_Z - 1, 0, 0);
    let mut children = parent.children().to_vec();
    children.sort_unstable();
    let now = Now {
        ms: 0.0,
        goal: &[parent],
        drawn: &children,
        building: &[],
        recubing: 0,
        retier: &[],
        free: RESERVE,
    };
    let mut frontier = Frontier::default();
    frontier.plan(&now, &Widest);
    let later = Now {
        ms: crate::param::COARSEN_HOLD_MS,
        ..now
    };
    assert_eq!(frontier.plan(&later, &Widest).start, vec![parent], "a coarsening not started with the reserve free");
    let fill = Now {
        goal: &[at(LEAF_Z - 1, 4, 4)],
        drawn: &[],
        ..now
    };
    assert!(Frontier::default().plan(&fill, &Widest).start.is_empty(), "a fill took the reserve");
}

#[test]
fn planning_a_frontier_it_has_held_allocates_nothing() {
    let mut t = Table::new([300, 200, 0]);
    let l = LEAF_Z;
    let goals = [
        goal(l - 3, l - 3, 0),
        goal(l, l - 2, 16),
        goal(l - 1, l - 3, 8),
        goal(l, l, 16),
        goal(l - 2, l - 3, 8),
        goal(l, l - 1, 8),
    ];
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

const LAND: f64 = 10.5;
const EYE: V3 = [1_000.5, 2_000.25, LAND + 2.0];
const FAR: f64 = 20_000.0;

impl Table {}

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
    (0..8).map(|k| Tau::of(libm::pow(2f64, k as f64 / 2.0))).collect()
}

fn domain_at(height: f64, world: &World) -> Domain {
    let at = [EYE[0], EYE[1], LAND + height];
    let price = Need { slots: |_, _| 1, admit: false };
    fixture::domain_priced(eye(at, Some([1.0, 0.05, -0.25]), FAR, LAND), &ladder(), world, &price)
}

fn counts(d: &mut Domain) -> Vec<Slots> {
    (0..d.ladder().len()).map(|k| d.need(k).expect("every step counted")).collect()
}

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
            let of = |pool: Pool| pieces.iter().filter(|&&c| pool_of(c, scale_of(c, d.outermost(c), d.order_of(c).near, tau, false)) == pool).count() as u32;
            assert!(
                n[k][Pool::Coarse as usize] + n[k][Pool::Cover as usize] >= pieces.len() as u32,
                "height {height}, {tau:?}: coarse under the cut"
            );
            assert!(n[k][Pool::Fine as usize] >= of(Pool::Fine), "height {height}, {tau:?}: fine under the cut");
            assert!(n[k][Pool::Cover as usize] >= of(Pool::Cover), "height {height}, {tau:?}: cover under the cut");
        }
    }
}
