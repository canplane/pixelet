// pxl-core/domain/query/change.test.rs

use super::*;
use crate::domain::layer::Answer;
use crate::domain::layer::object::store::Store;
use crate::domain::observer;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed, set_eye, step_slide_by, write};
use crate::domain::query::{Ray, World};
use pxl_spec::asset::cube;
use pxl_spec::asset::id::AssetId;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::brick;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::layer::lattice::GROUND;

const RECT: [[f64; 2]; 2] = [[0.5, 0.5], [1.5, 1.5]];

/// A memo keeping `a` for the key 0, which read `RECT` and nothing of the
/// eye.
fn keeping<A>(a: A) -> Memo<u32, A> {
    let read = Read {
        rect: Some(RECT),
        ..Read::default()
    };
    Memo(Some((0, read, a)))
}

/// Whether a kept answer, exact or not, survives a frame whose one change is
/// the ground node at `zoom` over `RECT`.
fn kept(exact: bool, zoom: Zoom) -> bool {
    changes().ground = vec![Quadrant::new(zoom, [0, 0])];
    changes().damage.clear();
    let mut memo = keeping(exact);
    memo.forget_touched_at_leaf(|&exact| exact);
    memo.0.is_some()
}

#[test]
fn a_coarser_node_leaves_an_exact_answer_alone() {
    let leaf = GROUND.leaf_cell_zoom();
    let tile = leaf - GROUND.tile_cell_shamt();
    assert!(!kept(true, tile), "the leaf's tile drops it");
    assert!(
        !kept(true, brick::bricks_zoom(leaf)),
        "the leaf's brick drops it"
    );
    assert!(kept(true, tile - 1), "a coarser node leaves it");
    assert!(
        !kept(false, tile - 1),
        "a coarser node drops an answer not exact"
    );
}

#[test]
fn what_objects_wrote_drops_an_exact_answer() {
    changes().ground.clear();
    changes().damage = vec![Aabb {
        min: [1.0, 1.0, BOUND.min[2]],
        max: [2.0, 2.0, BOUND.max[2]],
    }];
    let mut memo = keeping(true);
    memo.forget_touched_at_leaf(|&exact| exact);
    assert!(memo.0.is_none());
    changes().damage.clear();
}

/// The queries report the rect they read, joined: a support reads its own
/// cell, and a walk reads every cell it passes.
#[test]
fn queries_report_the_rect_they_read() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let cell = units(GROUND.leaf_cell_zoom());
    let at = [x + 0.5 * cell, y + 0.5 * cell];
    let c = at.map(|v| libm::floor(v / cell) * cell);
    let from = point(at[0], at[1], over(5.0));
    // The eye stands where it draws the leaves' cells.
    let eye = point(at[0], at[1], 64.0);
    let ((), read) = reads(|| {
        world.surface_support(&point(at[0], at[1], 0.0));
        assert_eq!(
            read().as_ref().unwrap().rect,
            Some([c, c.map(|v| v + cell)])
        );
        step_slide_by(
            &world,
            &from,
            &point(at[0] + 3.0 * cell, at[1], 0.0),
            &eye,
            true,
        );
    });
    assert_eq!(read.rect, Some([c, [c[0] + 4.0 * cell, c[1] + cell]]));
    // A body of known height that does not show reads nothing, however
    // far it goes.
    let high = point(at[0], at[1], 1000.0);
    let (_, read) = reads(|| {
        step_slide_by(
            &world,
            &from,
            &point(at[0] + 3000.0 * cell, at[1], 0.0),
            &high,
            true,
        )
    });
    assert_eq!(read.rect, None);
}

/// A memo keeps what a cell's asset answers and asks again when a change
/// touches the cell's rect.
#[test]
fn a_memo_keeps_a_cell_until_its_rect_changes() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let mut memo = Memo::<u32, Option<u32>>::NONE;
    let at = point(x + 0.5, y + 0.5, 4.5);
    let asked = memo.of(0, || world.asset_of(&at).val());
    assert!(asked.is_some());
    assert!(memo.0.is_some(), "a cell's asset read its rect");
    changes().ground.clear();
    changes().damage.clear();
    memo.forget_touched();
    assert!(memo.0.is_some(), "no change touched the rect");
    changes().damage = vec![Aabb {
        min: [x, y, BOUND.min[2]],
        max: [x + 1.0, y + 1.0, BOUND.max[2]],
    }];
    memo.forget_touched();
    assert!(memo.0.is_none(), "a write over the rect drops it");
    changes().damage.clear();
}

/// The eye `dist` units from `pivot`, tilted `polar` from straight over it
/// and turned `azimuth` about it.
fn eye_about(pivot: V3, dist: f64, polar: f64, azimuth: f64) -> V3 {
    let (sp, cp) = libm::sincos(polar);
    let (sa, ca) = libm::sincos(azimuth);
    [
        pivot[0] + dist * sp * ca,
        pivot[1] + dist * sp * sa,
        pivot[2] + dist * cp,
    ]
}

/// The eye led through `eyes`, a read asked through a memo at each. Wherever
/// the eye stands the memo gives what a fresh read gives. Returns how many
/// times it asked and the answers it gave.
fn led<A: Copy + PartialEq + core::fmt::Debug>(
    eyes: impl Iterator<Item = V3>,
    read: impl Fn() -> A,
) -> (u32, Vec<A>) {
    let mut memo = Memo::<u32, A>::NONE;
    let ask_cnt = core::cell::Cell::new(0);
    let mut answers = Vec::new();
    for eye in eyes {
        set_eye(eye);
        let a = memo.of(0, || {
            ask_cnt.set(ask_cnt.get() + 1);
            read()
        });
        assert_eq!(a, read(), "from {eye:?}");
        answers.push(a);
    }
    (ask_cnt.get(), answers)
}

/// A cube placed on the flat ground, which the eye draws from near and
/// folds into a coarser cell from far, so the floor over it differs by the
/// zoom it is read at. Returns the middle of the cube's rect.
fn cube_on_flat() -> (World, [f64; 2]) {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    write(
        &mut store,
        [x, y, 5.0],
        OBJECT_COARSEST,
        cube::Object::Red as AssetId,
    );
    (placed(store), [x + 0.5, y + 0.5])
}

/// A floor that differs by size, read through a memo. A whole turn about it
/// at one distance chooses one size for its cell, so it is asked once; drawn
/// back, it is asked again at each size and gives that size's floor, as a
/// fresh read does.
#[test]
fn a_memo_keeps_a_floor_while_the_eye_chooses_its_size() {
    let (world, [x, y]) = cube_on_flat();
    let pivot = [x, y, over(6.0)];
    let at = point(x, y, pivot[2]);
    let floor = || world.floor_under(&at);
    let turn = core::f64::consts::TAU / 240.0;
    let (ask_cnt, floors) = led(
        (0..240).map(|k| eye_about(pivot, 64.0, 0.5, f64::from(k) * turn)),
        floor,
    );
    assert_eq!((ask_cnt, floors[0]), (1, Answer::Exact(Some(6.0))));
    let (_, floors) = led(
        (0..200).map(|k| eye_about(pivot, 8.0 + 3.0 * f64::from(k), 0.5, 0.3)),
        floor,
    );
    let tops: Vec<_> = floors.iter().map(|f| f.val().flatten()).collect();
    assert!(
        tops.iter().any(|t| *t != tops[0]),
        "the floor differs by size: {tops:?}"
    );
}

/// The eye as the measured drive leads it about a standing pivot from `dist`
/// units, a tick each: a whole turn in 240 ticks, and with `zooming` the
/// wheel's sweep in to the nearest and out again, 60 ticks each way.
fn driven(pivot: V3, dist: f64, zooming: bool) -> impl Iterator<Item = V3> {
    let polar = core::f64::consts::FRAC_PI_3 + 0.12 * core::f64::consts::PI;
    (0..240u32).map(move |k| {
        let leg = k % 120;
        let steps = f64::from(if leg < 60 { leg } else { 120 - leg });
        let d = match zooming {
            true => (dist * libm::exp(-0.22 * steps)).max(2.0),
            false => dist,
        };
        eye_about(
            pivot,
            d,
            polar,
            f64::from(k) * core::f64::consts::TAU / 240.0,
        )
    })
}

/// The floor under a standing body while the camera turns about it, and
/// while it turns and zooms: it is read once a size the eye sets for the
/// body's cell. A turn keeps the body's distance, so one read. A sweep
/// crosses a zoom's edge wherever the distance passes 48 units times a power
/// of two, and the finest zoom placed leaves no edge under 12: from 16 units
/// in to 2 and out, twice, the edge at 12 is crossed four times, five reads;
/// from 157.8 the edges at 96, 48, 24 and 12, sixteen crossings, seventeen.
#[test]
fn a_floor_is_read_once_a_size() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let pivot = [x + 2.5, y + 2.5, over(5.0)];
    let at = point(pivot[0], pivot[1], pivot[2]);
    for (dist, swept) in [(16.0, 5), (157.8, 17)] {
        let asked = |zooming| led(driven(pivot, dist, zooming), || world.floor_under(&at)).0;
        assert_eq!(asked(false), 1, "a turn at {dist}");
        assert_eq!(asked(true), swept, "a sweep from {dist}");
    }
}

/// A ray across what the eye sees, the orbit's line from the pivot toward
/// the eye, asks each node by the eye's distance: its answer is of the eye
/// where it stood, and is asked again wherever else the eye stands.
#[test]
fn a_memo_keeps_the_eyes_ray_for_where_the_eye_stood() {
    let (world, [x, y]) = cube_on_flat();
    let ray = Ray {
        origin: [x - 4.0, y - 3.0, 9.5],
        dir: [0.6, 0.48, -0.64],
        t_max: 40.0,
    };
    let hit = || world.first_hit(&ray).map(|h| h.map(|h| h.cube.max[2]));
    let eye = eye_about([x, y, 5.5], 64.0, 0.5, 0.0);
    let (ask_cnt, hits) = led([eye, eye, [eye[0] + 1.0, eye[1], eye[2]]].into_iter(), hit);
    assert_eq!(ask_cnt, 2);
    assert!(hits[0].val().flatten().is_some());
}

/// A read from a height not known settles on a cell from the point over it
/// at the eye's height: it is kept while the eye chooses the same sizes
/// there and for the cells it went on to, and asked again when it does not.
#[test]
fn a_memo_keeps_a_surface_while_the_eye_chooses_its_sizes() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let at = point(x + 0.5, y + 0.5, 0.0);
    let eye = [x + 0.5, y + 0.5, 64.0];
    let (ask_cnt, tops) = led(
        [
            eye,
            [eye[0] + 3.0, eye[1], eye[2] + 2.0],
            [eye[0], eye[1], 400.0],
        ]
        .into_iter(),
        || world.surface_support(&at),
    );
    assert_eq!(ask_cnt, 2);
    assert_eq!(tops[0], Answer::Exact(Some(5.0)));
}

/// With no eye a floor is a lack and is not kept; a cell's asset, which
/// takes nothing of the eye, is kept whether the eye stands or not.
#[test]
fn a_memo_with_no_eye_keeps_what_takes_no_eye() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let at = point(x + 0.5, y + 0.5, over(5.0));
    let eye = [x + 0.5, y + 0.5, 64.0];
    observer::set_pose(None);
    let mut floor = Memo::<u32, Answer<Option<f64>>>::NONE;
    assert!(matches!(
        floor.of(0, || world.floor_under(&at)),
        Answer::Lack(_)
    ));
    assert!(floor.0.is_none(), "a lack read nothing");
    let mut cell = Memo::<u32, Option<u32>>::NONE;
    let ask_cnt = core::cell::Cell::new(0);
    let asset = |memo: &mut Memo<u32, Option<u32>>| {
        memo.of(0, || {
            ask_cnt.set(ask_cnt.get() + 1);
            world.asset_of(&point(x + 0.5, y + 0.5, 4.5)).val()
        })
    };
    asset(&mut cell);
    set_eye(eye);
    asset(&mut cell);
    assert_eq!(ask_cnt.get(), 1);
    // An ask that took the eye's absence goes when the eye stands.
    observer::set_pose(None);
    let mut both = Memo::<u32, (Option<u32>, bool)>::NONE;
    let ask = |memo: &mut Memo<u32, (Option<u32>, bool)>| {
        memo.of(0, || {
            (
                world.asset_of(&point(x + 0.5, y + 0.5, 4.5)).val(),
                world.floor_under(&at).val().is_some(),
            )
        })
    };
    assert!(!ask(&mut both).1);
    set_eye(eye);
    assert!(ask(&mut both).1, "the eye stands: asked again");
}
