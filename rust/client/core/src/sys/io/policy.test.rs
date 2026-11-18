// pxl-core/sys/io/policy.test.rs

use super::*;

/// An object's id is its shard's tile id, and the top's id 0 names no tile,
/// so the top must rank without a tile read from its id.
#[test]
fn object_top_ranked() {
    let r = rank(Stream::Object, 0, Order::FIRST);
    assert_eq!((r.class, r.detail), (OBJECT, 0));
}

/// The images start before the catalog's models. The imagery's global
/// image, any other image and the imagery's tiles all rank ahead of every
/// model, and the models still rank ahead of places.
#[test]
fn images_before_models() {
    use crate::sys::load::id_of;
    use crate::sys::load::{Load, LoadKind};
    let model = rank(
        Stream::Asset,
        id_of(LoadKind::Gltf, Load::File(0)),
        Order::LAST,
    );
    let global = rank(
        Stream::Asset,
        id_of(LoadKind::Image, Load::Global),
        Order::LAST,
    );
    let image = rank(
        Stream::Asset,
        id_of(LoadKind::Image, Load::File(7)),
        Order::LAST,
    );
    let imagery = rank(
        Stream::Imagery,
        Quadrant::of_key("0103").expect("a tile").id(),
        Order::FIRST,
    );
    let place = rank(
        Stream::Place,
        Quadrant::of_key("0103").expect("a tile").id(),
        Order::FIRST,
    );
    assert!(global.class < model.class);
    assert!(image.class < model.class);
    assert!(imagery.class < model.class);
    assert!(model.class < place.class);
}
