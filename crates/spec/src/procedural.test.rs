// pxl-spec/procedural.test.rs

use super::*;

#[test]
fn a_pier_stands_on_the_ground_and_not_where_the_terrain_gives_none() {
    use crate::cell::cube_types::{ground, sentinel};
    assert_eq!(pier_stands([0, 10], ground::grass, [], 0, 1.0), Some([0, 10]));
    assert_eq!(pier_stands([0, 10], sentinel::unknown, [], 0, 1.0), None);
    assert_eq!(pier_stands([0, 10], sentinel::void, [], 0, 1.0), None);
}
