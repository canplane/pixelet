// pxl-geo-bake/pyramid.test.rs

use super::*;
use crate::util::{pack, Level};
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile::place::PLACE_LATTICE;
use pxl_spec::directory;
use pxl_spec::space::layer::lattice::GROUND;

/// A count, summed up the pyramid, read as `PLACE` says: places' lattice or the ground's.
#[derive(Clone, PartialEq)]
struct Count<const PLACE: bool>(u32);

impl<const PLACE: bool> Base for Count<PLACE> {
    const LAYER: BaseLayer = BaseLayer::place;
    const LATTICE: Lattice = if PLACE { PLACE_LATTICE } else { GROUND };

    fn encode(&self, _: Quadrant) -> Result<Vec<u8>, String> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        Ok(Count(u32::from_le_bytes(
            raw.try_into().map_err(|_| "four bytes")?,
        )))
    }

    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self> {
        Some(Count(
            p.children()
                .iter()
                .filter_map(|&c| get(c))
                .map(|c| c.0)
                .sum(),
        ))
    }

    fn occupancy(&self) -> Occupancy {
        Occupancy::of_slots([true; 4])
    }
}

/// The zooms one leaf folds to.
fn folded<const PLACE: bool>() -> Vec<Zoom> {
    let root = directory::tmp_dir(&format!("pyramid-{PLACE}-{}", std::process::id()));
    let (set, stage) = (
        TileSet::new(root.join("set")),
        TileSet::new(root.join("stage")),
    );
    let leaf = Quadrant::of_key("010332102301230").unwrap();
    stage
        .write(
            leaf,
            &pack(&Count::<PLACE>(1).encode(leaf).unwrap(), Level::Live).unwrap(),
        )
        .unwrap();
    let whole: FnvHashSet<Quadrant> = [leaf.shard()].into_iter().collect();
    let mut staged = Staged {
        base: &set,
        stage: &stage,
        leaf: LEAF_ZOOM,
        whole: &whole,
        removed: FnvHashSet::default(),
    };
    fold::<Count<PLACE>>(
        &mut staged,
        [leaf].into_iter().collect(),
        &Count::<PLACE>(0),
        1,
    )
    .unwrap();
    let zooms = stage.by_zoom().unwrap().into_keys().collect();
    fs::remove_dir_all(root).unwrap();
    zooms
}

#[test]
fn fold_to_coarsest() {
    assert_eq!(
        folded::<false>(),
        (Count::<false>::COARSEST..=LEAF_ZOOM).collect::<Vec<_>>()
    );
    assert_eq!(
        folded::<true>(),
        (Count::<true>::COARSEST..=LEAF_ZOOM).collect::<Vec<_>>()
    );
}

/// A tile a fold cannot encode: each that fails is said, the zoom is finished, and the fold
/// fails with the number, staging no `fold.done` for a commit to take.
#[derive(Clone, PartialEq)]
struct Unencodable(u32);

impl Base for Unencodable {
    const LAYER: BaseLayer = BaseLayer::place;
    const LATTICE: Lattice = PLACE_LATTICE;

    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        Err(format!("tile {} size 9 > 1", at.key()))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        Ok(Unencodable(u32::from_le_bytes(
            raw.try_into().map_err(|_| "four bytes")?,
        )))
    }

    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self> {
        Some(Unencodable(
            p.children()
                .iter()
                .filter_map(|&c| get(c))
                .map(|c| c.0)
                .sum(),
        ))
    }

    fn occupancy(&self) -> Occupancy {
        Occupancy::of_slots([true; 4])
    }
}

#[test]
fn fold_failure_stops() {
    let root = directory::tmp_dir(&format!("pyramid-fail-{}", std::process::id()));
    let (set, stage) = (
        TileSet::new(root.join("set")),
        TileSet::new(root.join("stage")),
    );
    let leaf = Quadrant::of_key("010332102301230").unwrap();
    stage
        .write(leaf, &pack(&1u32.to_le_bytes(), Level::Live).unwrap())
        .unwrap();
    let whole: FnvHashSet<Quadrant> = [leaf.shard()].into_iter().collect();
    let mut staged = Staged {
        base: &set,
        stage: &stage,
        leaf: LEAF_ZOOM,
        whole: &whole,
        removed: FnvHashSet::default(),
    };
    let err = fold::<Unencodable>(
        &mut staged,
        [leaf].into_iter().collect(),
        &Unencodable(0),
        1,
    )
    .unwrap_err();
    assert_eq!(err, format!("z{}: 1 tiles failed", LEAF_ZOOM - 1));
    assert_eq!(
        stage.by_zoom().unwrap().into_keys().collect::<Vec<_>>(),
        vec![LEAF_ZOOM],
        "nothing past the leaf is staged"
    );
    fs::remove_dir_all(root).unwrap();
}
