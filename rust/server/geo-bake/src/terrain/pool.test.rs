// pxl-geo-bake/terrain/pool.test.rs

use super::*;
use crate::pyramid::TileSet;
use fnv::FnvHashSet;
use pxl_spec::directory;
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

#[test]
fn a_set_with_tiles_and_no_fits_is_refused() {
    let root = directory::temp_dir(&format!("pool-{}", std::process::id()));
    let (set, stage) = (
        TileSet::new(root.join("set")),
        TileSet::new(root.join("stage")),
    );
    let whole = FnvHashSet::default();
    let staged = Staged {
        base: &set,
        stage: &stage,
        leaf: LEAF_ZOOM,
        whole: &whole,
        gone: FnvHashSet::default(),
    };
    assert!(check(&staged, 1).unwrap().is_empty(), "an empty set is new");

    let tile = Quadrant::of_key("010332102301230").unwrap();
    set.write(tile, b"a tile").unwrap();
    assert!(check(&staged, 1).unwrap_err().contains("no fits"));
}
