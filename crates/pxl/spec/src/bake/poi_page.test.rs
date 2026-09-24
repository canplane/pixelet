// pxl-spec/bake/poi_page.test.rs

use super::*;
use crate::space::pvf3::ivec;

/// A page survives the wire whole, its names with it.
#[test]
fn a_page_and_its_names_come_back_as_they_went() {
    let page = PoiPage::of_places([(7, "Seoul Station"), (9, "서울역"), (9, "남대문"), (12, ""), (40, "Seoul Station")]);
    assert_eq!(decode(&encode(&page).unwrap()).unwrap(), page);
    // A name standing in two cells is held once, and a cell of two places
    // holds none.
    assert_eq!(page.names.len(), 1);
    assert_eq!(page.cells.iter().map(|c| page.name(c)).collect::<Vec<_>>(), [Some("Seoul Station"), None, None, Some("Seoul Station")]);
}

/// A name is a cell's only while it holds one place, at every level: a
/// parent with one place under it has that place's name, and a parent with
/// two has a count and nothing else.
#[test]
fn a_name_folds_up_while_its_cell_holds_one_place() {
    let p = LevelPage::new(1, ivec::gather(0, 0, 0));
    let empty = PoiPage::default();
    let lone = PoiPage::of_places([(0, "Namsan Tower")]);
    let two = PoiPage::of_places([(0, "A"), (1, "B")]);
    let one_up = PoiPage::fold_page(p, |k| Some(if k == p.children()[0] { &lone } else { &empty })).unwrap();
    assert_eq!(one_up.cells.len(), 1);
    assert_eq!(one_up.name(&one_up.cells[0]), Some("Namsan Tower"));
    let pair_up = PoiPage::fold_page(p, |k| Some(if k == p.children()[0] { &two } else { &empty })).unwrap();
    assert_eq!(pair_up.cells[0].count, 2);
    assert_eq!(pair_up.name(&pair_up.cells[0]), None);
}

/// What the wire cannot hold is refused: a name in a cell of two, a name
/// split across a character.
#[test]
fn a_page_that_names_a_crowd_or_splits_a_character_is_refused() {
    let mut crowd = PoiPage::of_places([(3, "A"), (3, "B"), (5, "C")]);
    crowd.cells[0].name = 0;
    assert!(encode(&crowd).is_err());
    let good = encode(&PoiPage::of_places([(1, "역"), (2, "a")])).unwrap();
    let mut split = good.clone();
    // The first name's end moved into the middle of its one character.
    let at = 16 + 2 * 12;
    split[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(decode(&split).is_err());
}
