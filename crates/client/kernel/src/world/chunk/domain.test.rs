// pxl-kernel/world/chunk/domain.test.rs

use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use super::*;
use crate::world::chunk::Chunk;
use crate::world::fixture::Free;
use crate::world::view::observer::cell_angle;

use crate::world::fixture::{self, eye, World};

fn ladder() -> Vec<Tau> {
    (0..6).map(|k| Tau::of(libm::pow(2f64, k as f64 / 2.0))).collect()
}

fn point(p: V3) -> Pvf3 {
    world::point(p[0], p[1], p[2])
}

fn pieces_at(pieces: &[Chunk], p: V3) -> Vec<Chunk> {
    let at = point(p);
    let mut over: Vec<Chunk> = pieces.iter().copied().filter(|c| *c == Chunk::holding(c.zoom(), &at)).collect();
    over.sort_unstable_by_key(|c| std::cmp::Reverse(c.zoom()));
    over
}

const GROUND: f64 = 10.5;
const EYE: V3 = [1_000.5, 2_000.25, GROUND + 2.0];
const FAR: f64 = 30_000.0;

/// A pole thinner and shorter than a far cell is not refined and is under
/// exactly one piece.
#[test]
fn a_far_pole_inside_a_cell_is_not_refined_and_not_a_hole() {
    let o = eye(EYE, Some([1.0, 0.0, 0.0]), FAR, GROUND);
    let tau = Tau::FINEST;
    let flat = fixture::domain(o, &[tau], &World::flat(GROUND));
    let at = [EYE[0] + 2_700.0, EYE[1] + 40.0];
    let pole = |height: f64| World::flat(GROUND).with([at[0], at[1], GROUND - 1.0], [at[0] + 0.5, at[1] + 0.5, GROUND + height]);
    let thin = fixture::domain(o, &[tau], &pole(3.0));
    assert!(3.0 / 2_700.0 < cell_angle(tau), "the pole is not inside a cell");
    assert_eq!(thin.pieces(), flat.pieces(), "a pole inside a cell refined the goal");
    let top = [at[0] + 0.25, at[1] + 0.25, GROUND + 2.5];
    assert_eq!(pieces_at(thin.pieces(), top).len(), 1, "the pole is a hole or under two pieces");
}

#[test]
fn remaking_a_moving_view_allocates_nothing_once_it_has_held_the_most() {
    let mut world = World::flat(GROUND);
    for k in 0..8 {
        let x = 1_100.0 + 150.0 * k as f64;
        world = world.with([x, 1_900.0, GROUND], [x + 20.0, 2_100.0, GROUND + 30.0 + 20.0 * k as f64]);
    }
    let ladder = ladder();
    let path = |k: u32| {
        let at = [EYE[0] + 13.0 * k as f64, EYE[1] + 7.0 * k as f64, GROUND + 2.0 + (k % 5) as f64 * 40.0];
        eye(at, Some([1.0, 0.1 * (k % 3) as f64, -0.2]), FAR, GROUND)
    };
    let mut sight = crate::world::view::observer::Sight::default();
    sight.observe(&path(0), &world, true, &[]);
    let mut d = Domain::empty(path(0), world.clone());
    let walk = |d: &mut Domain<World>, sight: &mut crate::world::view::observer::Sight| {
        for k in 0..24 {
            let o = path(k);
            sight.observe(&o, &world, false, &[]);
            d.begin(o, pxl_spec::space::layer::lattice::GROUND.top(), &ladder, sight.footprints(), core::iter::empty());
            d.run(&Free, &|| false, &mut 0);
            d.put(0);
            for step in 0..ladder.len() {
                d.cut_at(step);
            }
        }
    };
    let (first, ()) = crate::allocations::during(|| walk(&mut d, &mut sight));
    assert!(first > 0, "nothing counted the first time over, so the count says nothing");
    let (again, ()) = crate::allocations::during(|| walk(&mut d, &mut sight));
    assert_eq!(again, 0, "making the view again along the path allocated");
}
