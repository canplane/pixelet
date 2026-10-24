// pxl-geo-bake/pyramid.test.rs

use super::*;
use crate::util::{pack, Level};
use pxl_spec::bake::published::BaseLayer;
use pxl_spec::bake::tile::place::PLACE_LATTICE;
use pxl_spec::space::layer::lattice::GROUND;

/// A count, summed up the pyramid, read as `PLACE` says: places' lattice or the ground's.
#[derive(Clone, PartialEq)]
struct Count<const PLACE: bool>(u32);

impl<const PLACE: bool> Base for Count<PLACE> {
    const LAYER: BaseLayer = BaseLayer::place;
    const LATTICE: Lattice = if PLACE { PLACE_LATTICE } else { GROUND };

    fn encode(&self, _: Tile) -> Result<Vec<u8>, String> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        Ok(Count(u32::from_le_bytes(raw.try_into().map_err(|_| "four bytes")?)))
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        Some(Count(p.children().iter().filter_map(|&c| held(c)).map(|c| c.0).sum()))
    }

    fn occupancy(&self) -> Occupancy {
        Occupancy::of_slots([true; 4])
    }
}

/// The zooms one leaf folds to.
fn folded<const PLACE: bool>() -> Vec<Zoom> {
    let root = std::env::temp_dir().join(format!("pxl-pyramid-{PLACE}-{}", std::process::id()));
    let (set, stage) = (TileSet::new(root.join("set")), TileSet::new(root.join("stage")));
    let leaf = Tile::of_name("010332102301230").unwrap();
    stage.write(leaf, &pack(&Count::<PLACE>(1).encode(leaf).unwrap(), Level::Live).unwrap()).unwrap();
    let whole: FnvHashSet<Tile> = [ancestor(leaf, SHARD_ZOOM)].into_iter().collect();
    let mut staged = Staged {
        base: &set,
        stage: &stage,
        leaf: LEAF_ZOOM,
        whole: &whole,
        gone: FnvHashSet::default(),
    };
    fold::<Count<PLACE>>(&mut staged, [leaf].into_iter().collect(), &Count::<PLACE>(0), 1).unwrap();
    let zooms = stage.by_zoom().unwrap().into_keys().collect();
    fs::remove_dir_all(root).unwrap();
    zooms
}

#[test]
fn a_kind_is_folded_up_to_its_coarsest_and_no_further() {
    assert_eq!(folded::<false>(), (Count::<false>::COARSEST..=LEAF_ZOOM).collect::<Vec<_>>());
    assert_eq!(folded::<true>(), (Count::<true>::COARSEST..=LEAF_ZOOM).collect::<Vec<_>>());
    const { assert!(Count::<true>::COARSEST > Count::<false>::COARSEST, "places stop short of the ground's top") };
}
