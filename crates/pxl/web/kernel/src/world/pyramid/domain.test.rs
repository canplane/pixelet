// pxl-kernel/world/pyramid/domain.test.rs
//
// The goal's laws on worlds known exactly (world/fixture.test.rs): the cover
// stands all around and ground with no answer is no piece; a piece refines
// only where it contributes to a cell and its error, projected, crosses one
// — so ground at a grazing angle refines across the screen as far as its
// error crosses a cell and no further along its depth, a far pole inside a
// cell is neither refined nor a hole, and hidden ground keeps its depth;
// every split lies within its level's reach whatever stands behind what is
// seen; and a finer quality splits every chunk a coarser one does.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::TOP_LEVEL;
use pxl_spec::space::v3::V3;

use super::*;
use crate::world::observer::cell_angle;

use crate::world::fixture::{self, eye, World};

/// A few qualities, finest first.
fn ladder() -> Vec<Tau> {
    (0..6).map(|k| Tau::of(2f64.powf(k as f64 / 2.0))).collect()
}

fn domain(o: Observation, world: &World) -> Domain {
    fixture::domain(o, &ladder(), world)
}

/// A point in units, as a position.
fn point(p: V3) -> Pvf3 {
    let mut at = Pvf3::default();
    projection::project(&mut at, p[0], p[1], p[2]);
    at
}

/// The pieces `p` lies in: exactly one where the goal has no gap.
fn pieces_at(pieces: &[Chunk], p: V3) -> Vec<Chunk> {
    let at = point(p);
    (0..=TOP_LEVEL).map(|level| Chunk::of(level, &at)).filter(|c| pieces.binary_search(c).is_ok()).collect()
}

/// Points of the ground about `eye[0..2]` out to `radius`, all around.
fn around(eye: V3, radius: f64, z: f64) -> Vec<V3> {
    let mut out = Vec::new();
    for ring in 1..=12 {
        let r = radius * ring as f64 / 12.0;
        for k in 0..48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            out.push([eye[0] + r * a.cos(), eye[1] + r * a.sin(), z]);
        }
    }
    out
}

/// Every cut of the ladder, made.
fn cuts(d: &mut Domain) -> Vec<(Tau, Vec<Chunk>, HashSet<Chunk>)> {
    (0..d.ladder().len())
        .map(|k| {
            let cut = d.cut_at(k);
            (cut.tau(), cut.pieces().to_vec(), cut.splits().collect())
        })
        .collect()
}

const GROUND: f64 = 10.5;
const EYE: V3 = [1_000.5, 2_000.25, GROUND + 2.0];
const FAR: f64 = 30_000.0;

/// Before anything over the ground is resident, nothing is known of it: no
/// piece, and the ground counted as having no answer yet. Once it is, the
/// cover stands all around the eye, behind it as much as before it, and
/// every point of the ground within the reach is in exactly one piece.
#[test]
fn the_cover_stands_all_around_and_ground_with_no_answer_is_no_piece() {
    let o = eye(EYE, Some([1.0, 0.0, 0.0]), FAR, GROUND);
    let mut unknown = World::flat(GROUND);
    unknown.answered = false;
    let d = domain(o, &unknown);
    assert!(d.pieces().is_empty(), "a piece where nothing is known");
    assert!(d.unanswered() > 0, "ground with no answer, not counted as such");

    let mut d = domain(o, &World::flat(GROUND));
    assert_eq!(d.unanswered(), 0);
    for (tau, pieces, _) in cuts(&mut d) {
        for p in around(EYE, FAR * 0.9, GROUND) {
            assert_eq!(pieces_at(&pieces, p).len(), 1, "{tau:?}: {p:?} in a gap or under two pieces");
        }
    }
    // Behind the eye nothing is seen, so nothing is refined there: the cover
    // stands at the top.
    let behind = pieces_at(d.pieces(), [EYE[0] - 20_000.0, EYE[1], GROUND]);
    assert_eq!(behind[0].level(), TOP_LEVEL, "ground behind the eye refined");
}

/// From 2u over flat ground, a piece is as fine across the screen as its
/// cell crossing a cell asks — the level the criterion wants at the
/// distance, out to the far edge — and no finer: along the screen's depth
/// its error projects under a cell and stays inside.
#[test]
fn grazing_ground_refines_across_the_screen_and_not_along_its_depth() {
    let o = eye(EYE, Some([1.0, 0.0, 0.0]), FAR, GROUND);
    let mut d = domain(o, &World::flat(GROUND));
    let half = (fixture::REFERENCE[0].atan() * 0.9).to_degrees() as i32;
    for (tau, pieces, _) in cuts(&mut d) {
        for bearing in (-half..=half).step_by(5) {
            let b = (bearing as f64).to_radians();
            for r in [3.0, 30.0, 100.0, 400.0, 1_500.0, 6_000.0, 20_000.0, 29_000.0] {
                let p = [EYE[0] + r * b.cos(), EYE[1] + r * b.sin(), GROUND - 0.5];
                let over = pieces_at(&pieces, p);
                assert_eq!(over.len(), 1, "{tau:?}: {p:?} under {over:?}");
                let level = over[0].level() as i32;
                let wanted = tau.level(r).clamp(0, TOP_LEVEL as i32);
                assert!(
                    level <= wanted,
                    "{tau:?}: ground {r} away at {bearing}° left at {level} where its cell crosses a cell finer than {wanted}"
                );
                assert!(level + 1 >= wanted, "{tau:?}: ground {r} away refined to {level}, past what its error across asks ({wanted})");
            }
        }
    }
}

/// A pole thinner and shorter than a cell far away is inside one: the goal
/// is the goal of the ground without it, and the pole is under exactly one
/// piece. The same pole tall enough to cross cells up the screen refines.
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

    let tall = fixture::domain(o, &[tau], &pole(60.0));
    let (level_flat, level_tall) = (
        pieces_at(flat.pieces(), top)[0].level(),
        pieces_at(tall.pieces(), [at[0] + 0.25, at[1] + 0.25, GROUND + 30.0])[0].level(),
    );
    assert!(level_tall < level_flat, "a pole crossing cells up the screen not refined: {level_tall} against {level_flat}");
}

/// Ground behind a wall contributes to no cell, so no chunk wholly behind it
/// is split, however near the eye; the ground before it is refined down to
/// z16's parents.
#[test]
fn hidden_ground_keeps_its_depth() {
    let (x0, x1) = (1_192.0, 1_256.0);
    let world = World::flat(GROUND).with([x0, -50_000.0, GROUND], [x1, 50_000.0, GROUND + 1_000.0]);
    let mut d = domain(eye(EYE, Some([1.0, 0.0, 0.0]), FAR, GROUND), &world);
    let cuts = cuts(&mut d);
    assert!(cuts[0].2.iter().any(|c| c.level() == 1), "the ground before the wall is not refined");
    for (tau, _, splits) in cuts {
        for c in splits {
            assert!(c.rect().0 < x1, "{tau:?}: {c:?} is wholly behind the wall and split");
        }
    }
}

/// With the source's top below the pyramid's, the cover is that level's
/// chunks and nothing coarser is ever a piece or asked to split.
#[test]
fn the_cover_is_the_sources_top() {
    let top = 3;
    let o = Observation {
        top,
        far: fault_radius(top),
        ..eye(EYE, Some([1.0, 0.1, -0.2]), fault_radius(top), GROUND)
    };
    let mut d = domain(o, &World::flat(GROUND));
    assert!(d.roots().iter().all(|c| c.level() == top), "a root not of the source's top");
    for (tau, pieces, splits) in cuts(&mut d) {
        assert!(pieces.iter().chain(&splits).all(|c| c.level() <= top), "{tau:?}: a piece or split over the source's top");
        for p in around(EYE, fault_radius(top) * 0.5, GROUND) {
            assert_eq!(pieces_at(&pieces, p).len(), 1, "{tau:?}: {p:?} in a gap or under two pieces");
        }
    }
    let c = d.pieces()[0];
    assert!(d.wanted_over(c).level() <= top, "the view said to draw over the source's top");
}

/// Making the view again as the eye moves allocates nothing once it has held
/// the most it makes: the tiles cast again and the ground cut at every step
/// of the ladder, along a path over ground and boxes the second time over.
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
    let mut sight = crate::world::observer::Sight::default();
    sight.observe(&path(0), &world, true, &[]);
    let mut d = Domain::of(path(0), &ladder, 0, Box::new(world.clone()), sight.footprints());
    let walk = |d: &mut Domain, sight: &mut crate::world::observer::Sight| {
        for k in 0..24 {
            let o = path(k);
            sight.observe(&o, &world, false, &[]);
            d.remake(o, &ladder, 0, sight.footprints());
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
