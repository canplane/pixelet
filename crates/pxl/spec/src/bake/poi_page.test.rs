// pxl-spec/bake/poi_page.test.rs

use super::*;
use crate::space::pvf3::ivec;

/// The level-0 page every place below stands in.
fn leaf() -> LevelPage {
    LevelPage::new(0, ivec::gather(3, -2, 0))
}

/// A point in `leaf()`: the corner of its POI cell (tx, ty), then `dx`, `dy`
/// level-0 ground cells in.
fn point(tx: u32, ty: u32, dx: u32, dy: u32) -> Pvf3 {
    let base = POI_LATTICE.cell_base(leaf(), tx, ty);
    POI_LATTICE.cell_of(0, &base).inner_at(GROUND, 0, [dx, dy]).mid()
}

/// A page survives the wire whole, its names and positions with it.
#[test]
fn a_page_and_its_names_come_back_as_they_went() {
    let page = PoiPage::of_places(
        0,
        [
            (point(7, 0, 1, 2), "Seoul Station"),
            (point(9, 0, 0, 0), "서울역"),
            (point(9, 0, 3, 3), "남대문"),
            (point(12, 0, 2, 0), ""),
            (point(40, 1, 0, 3), "Seoul Station"),
        ],
    );
    assert_eq!(decode(&encode(&page).unwrap()).unwrap(), page);
    // A name standing in two cells is held once, and a cell of two places
    // holds none and stands nowhere of its own.
    assert_eq!(page.names.len(), 1);
    assert_eq!(page.cells.iter().map(|c| page.name(c)).collect::<Vec<_>>(), [Some("Seoul Station"), None, None, Some("Seoul Station")]);
    assert_eq!(page.cells.iter().map(|c| c.at).collect::<Vec<_>>(), [Some([1, 2]), None, Some([2, 0]), Some([0, 3])]);
}

/// A name and a position are a cell's only while it holds one place, at
/// every level: a parent with one place under it has that place's name and
/// stands it on the same level-0 ground cell its leaf did, and a parent with
/// two has a count and nothing else.
#[test]
fn a_lone_place_folds_up_with_its_name_and_its_ground_cell() {
    let at = point(101, 77, 3, 1);
    let ground = GROUND.cell_of(0, &at);
    let empty = PoiPage::default();
    let mut below = (leaf(), PoiPage::of_places(0, [(at, "Namsan Tower")]));
    for level in 1..=TOP_LEVEL {
        let p = LevelPage::over(level, below.0.page);
        let up = PoiPage::fold_page(p, |k| Some(if k == below.0 { &below.1 } else { &empty })).unwrap();
        assert_eq!(up.cells.len(), 1);
        let c = up.cells[0];
        assert_eq!(up.name(&c), Some("Namsan Tower"));
        assert_eq!(POI_LATTICE.cell_at(p, c.cell).inner_at(GROUND, 0, c.at.unwrap()), ground, "level {level}");
        below = (p, up);
    }
    let two = PoiPage::of_places(0, [(point(0, 0, 0, 0), "A"), (point(1, 0, 0, 0), "B")]);
    let p = LevelPage::over(1, leaf().page);
    let pair_up = PoiPage::fold_page(p, |k| Some(if k == leaf() { &two } else { &empty })).unwrap();
    assert_eq!(pair_up.cells[0].count, 2);
    assert_eq!((pair_up.name(&pair_up.cells[0]), pair_up.cells[0].at), (None, None));
}

/// What the wire cannot hold is refused: a name or a position in a cell of
/// two, a lone place with no position, a name split across a character.
#[test]
fn a_page_that_names_a_crowd_or_splits_a_character_is_refused() {
    let crowd = PoiPage::of_places(0, [(point(3, 0, 0, 0), "A"), (point(3, 0, 1, 1), "B"), (point(5, 0, 0, 0), "C")]);
    let mut named = crowd.clone();
    named.cells[0].name = 0;
    assert!(encode(&named).is_err());
    let mut placed = crowd.clone();
    placed.cells[0].at = Some([0, 0]);
    assert!(encode(&placed).is_err());
    let mut lost = crowd;
    lost.cells[1].at = None;
    assert!(encode(&lost).is_err());
    let good = encode(&PoiPage::of_places(0, [(point(1, 0, 0, 0), "역"), (point(2, 0, 0, 0), "a")])).unwrap();
    let mut split = good.clone();
    // The first name's end moved into the middle of its one character.
    let at = 16 + 2 * CELL_BYTES;
    split[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(decode(&split).is_err());
}
