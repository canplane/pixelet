// pxl-core/domain/control/camera/pivot.test.rs

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed, set_eye, write};
use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube::{self, Sentinel};
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::space::coord::{q3, world};

fn height(at: &Q3) -> f64 {
    q3::to_real(i64::from(at[2]))
}

/// A pivot standing on the ground follows the floor under it when the world
/// there changes. It drops when what it stood on is dug away, and it rises
/// onto what is placed where it stands.
#[test]
fn the_pivot_follows_its_floor() {
    let ground = flat_at_five();
    let at = world::point(ground[0] + 0.5, ground[1] + 0.5, over(5.0));
    let on = |store: Store| {
        stand_memo().forget_touched();
        *stand_memo() = Memo::NONE;
        placed(store);
        stood(&at)
    };
    let store = Store::default();
    assert_eq!(height(&on(store)), over(5.0));

    let mut dug = Store::default();
    write(
        &mut dug,
        [ground[0], ground[1], 4.0],
        OBJECT_COARSEST,
        Sentinel::Air as AssetId,
    );
    let dug = height(&on(dug));
    assert!(dug < over(5.0), "{dug}");

    let mut built = Store::default();
    let red = cube::Object::Red as AssetId;
    write(
        &mut built,
        [ground[0], ground[1], 5.0],
        OBJECT_COARSEST,
        red,
    );
    assert_eq!(height(&on(built)), over(6.0));
}

/// A pan walks the pivot only where the eye draws a body: seen from near it
/// ends on the floor it walked to, seen from far it goes where it is moved at
/// the height it has.
#[test]
fn a_pan_walks_only_where_a_body_shows() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    set_eye([x + 0.5, y - 3.0, 8.0]);
    let walked = pan_to(&to_world(&from), 2.0, 0.0, 0.0);
    assert_eq!(walked[2], over(5.0));
    set_eye([x + 0.5, y + 0.5, over(5.0) + 500.0]);
    let high = world::point(x + 0.5, y + 0.5, 40.0);
    let moved = pan_to(&to_world(&high), 2.0, 0.0, 0.0);
    assert_eq!(moved[2], 40.0, "at the height it has");
}

/// The pivot's kept answer is the world's, of the point alone: where the eye
/// stands drops nothing.
#[test]
fn the_kept_answer_is_by_the_point() {
    let ground = flat_at_five();
    placed(Store::default());
    let at = world::point(ground[0] + 0.5, ground[1] + 0.5, 40.0);
    *stand_memo() = Memo::NONE;
    if !settings().surface_follow {
        assert_eq!(height(&stood(&at)), 40.0);
        return;
    }
    assert_eq!(height(&stood(&at)), over(5.0));
    assert!(stand_memo().keeps(&at), "kept");
    set_eye([ground[0] + 3000.0, ground[1], 64.0]);
    assert!(stand_memo().keeps(&at), "an eye elsewhere drops nothing");
}

/// On a floor the pivot walks, and the camera still moves in reals: pans far
/// shorter than a point's step add up to their sum either way, to what a
/// double holds at the place, at the floor's height. Against a wall the pivot stops short of it and stays
/// there, with no step back, while it slides along the wall by the pan's
/// own length.
#[test]
fn a_walked_pan_keeps_its_reals() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    // A slab one cell over the floor: a body does not fit under it.
    for along in -2..=2 {
        let at = [x + 2.0, y + f64::from(along), 6.0];
        write(&mut store, at, OBJECT_COARSEST, red);
    }
    placed(store);
    set_eye([x + 0.5, y - 3.0, 8.0]);
    let from = [x + 0.5, y + 0.5, over(5.0)];
    let mut at = from;
    for _ in 0..1000 {
        at = pan_to(&at, 1e-4, 0.0, 0.0);
    }
    assert!(
        (at[0] - (from[0] + 0.1)).abs() < 1e-6,
        "{}",
        at[0] - from[0]
    );
    assert_eq!(at[2], over(5.0), "on its floor");
    for _ in 0..1000 {
        at = pan_to(&at, -1e-4, 0.0, 0.0);
    }
    assert!((at[0] - from[0]).abs() < 1e-6, "and back");
    // Into what stops it, a little along it each time.
    let mut farthest = at[0];
    for n in 0..4000 {
        let was = at;
        at = pan_to(&at, 1e-3, 1e-4, 0.0);
        assert!(at[0] >= was[0], "{n}: a step back");
        assert!((at[1] - was[1] - 1e-4).abs() < 1e-8, "{n}: along it");
        farthest = farthest.max(at[0]);
    }
    assert!(farthest < x + 2.0, "short of it: {}", farthest - x);
    assert!(farthest > x + 1.0, "and up to it: {}", farthest - x);
}
