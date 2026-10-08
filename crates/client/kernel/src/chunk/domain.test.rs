// pxl-kernel/chunk/domain.test.rs

use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use crate::chunk::Chunk;
use crate::view::sight::cell_angle;

use crate::fixture::{self, eye, World};

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
