// pxl-core/sys/io/policy.test.rs

use super::*;

/// An object's id is a file's number, and the top's id 0 names no tile, so
/// the top must rank without a tile read from its id.
#[test]
fn object_top_ranked() {
    crate::on_core(|| {
        let r = rank(Stream::object, 0, Order::FIRST);
        assert_eq!((r.class, r.detail), (OBJECT, 0));
    });
}

/// The pictures start before the catalog's models. The Earth's sphere, any
/// other picture and the Earth's tiles all rank ahead of every model, and the
/// models still rank ahead of places.
#[test]
fn pictures_before_models() {
    crate::on_core(|| {
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
    });
}
