// pxl-core/sys/io/policy.test.rs

use super::*;

/// The objects' ids are files' numbers, the top's 0 no tile: it is
/// ranked without a tile read off it.
#[test]
fn object_top_ranked() {
    let r = rank(Stream::object, 0, Order::FIRST);
    assert_eq!((r.class, r.detail), (OBJECT, 0));
}

/// The pictures start before the catalog's models: the Earth's sphere and a
/// picture ahead of every model, the Earth's tiles with them; the models still
/// ahead of places.
#[test]
fn pictures_before_models() {
    use crate::sys::io::loader::id_of;
    use crate::sys::load::{Load, LoadKind};
    let model = rank(
        Stream::asset,
        id_of(LoadKind::Gltf, Load::File(0)),
        Order::LAST,
    );
    let sphere = rank(
        Stream::asset,
        id_of(LoadKind::Image, Load::Sphere),
        Order::LAST,
    );
    let picture = rank(
        Stream::asset,
        id_of(LoadKind::Image, Load::File(7)),
        Order::LAST,
    );
    let earth = rank(
        Stream::earth,
        Quadrant::of_key("0103").expect("a tile").id(),
        Order::FIRST,
    );
    let place = rank(
        Stream::place,
        Quadrant::of_key("0103").expect("a tile").id(),
        Order::FIRST,
    );
    assert!(sphere.class < model.class);
    assert!(picture.class < model.class);
    assert!(earth.class < model.class);
    assert!(model.class < place.class);
}
