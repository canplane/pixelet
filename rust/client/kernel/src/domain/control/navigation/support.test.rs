// pxl-kernel/domain/control/navigation/support.test.rs

use super::*;
use crate::domain::layer::object::store::{Store, AIR};
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;
use pxl_spec::asset::cube::{self};
use pxl_spec::asset::AssetId;
use pxl_spec::bake::constant::OBJECT_COARSEST;

fn height(at: &Q3) -> f64 {
    q3::to_real(i64::from(at[2]))
}

/// A pivot standing on the ground follows the floor under it as the world
/// there changes: down where what it stood on is dug away, up onto what is
/// placed where it stands.
#[test]
fn the_pivot_follows_its_floor() {
    let _g = serial();
    let (tile_cache, ground) = flat_at_five();
    let at = world::point(ground[0] + 0.5, ground[1] + 0.5, over(5.0));
    let on = |store: &Store| {
        stand_memo().forget_touched();
        *stand_memo() = Memo::NONE;
        let w = World::new(&tile_cache, store);
        standing(w, &at).exact().flatten().unwrap()
    };
    let store = Store::default();
    assert_eq!(height(&on(&store)), over(5.0));

    let mut dug = Store::default();
    write(&mut dug, [ground[0], ground[1], 4.0], OBJECT_COARSEST, AIR);
    assert!(height(&on(&dug)) < over(5.0), "{}", height(&on(&dug)));

    let mut built = Store::default();
    let red = cube::Object::red as AssetId;
    write(
        &mut built,
        [ground[0], ground[1], 5.0],
        OBJECT_COARSEST,
        red,
    );
    assert_eq!(height(&on(&built)), over(6.0));
}
