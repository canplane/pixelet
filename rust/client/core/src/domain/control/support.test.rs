// pxl-core/domain/control/support.test.rs

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
        let w = placed(store);
        standing(w, &at).exact().flatten().unwrap()
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

/// A pan walks the pivot only where a body shows for the eye it is handed:
/// seen from near it ends on the floor, seen from far it goes where it is
/// moved with its height not known.
#[test]
fn a_pan_walks_only_where_a_body_shows() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let near = world::point(x + 0.5, y - 3.0, 8.0);
    let far = world::point(x + 0.5, y + 0.5, over(5.0) + 500.0);
    set_eye(to_world(&near));
    let walked = pan_to(&from, 2.0, 0.0, 0.0, &near);
    assert_eq!(height(&walked), over(5.0));
    set_eye(to_world(&far));
    assert!(is_unknown(&pan_to(&from, 2.0, 0.0, 0.0, &far)));
}
