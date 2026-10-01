// pxl-spec/bake/poi_page.test.rs

use super::*;
use crate::space::pvf3::ivec;

/// The z16 page every place below stands in.
fn leaf() -> LevelPage {
    LevelPage::new(0, ivec::gather(3, -2, 0))
}

/// A point in `leaf()`: the corner of its POI cell (tx, ty), then `dx`, `dy`
/// z16 ground cells in.
fn point(tx: u32, ty: u32, dx: u32, dy: u32) -> Pvf3 {
    let base = POI_LATTICE.cell_base(leaf(), tx, ty);
    POI_LATTICE.cell_of(0, &base).inner_at(GROUND, 0, [dx, dy]).mid()
}

/// A place of no kind, called `name`.
fn called(name: &str) -> Place<'_> {
    Place { name, kind: 0, rank: 100 }
}

/// A page survives the wire whole, its places and positions with it.
#[test]
fn a_page_and_its_places_come_back_as_they_went() {
    let station = Place {
        name: "Seoul Station",
        kind: kind_of("subway"),
        rank: 4,
    };
    let page = PoiPage::of_places(
        0,
        [
            (point(1, 0, 1, 2), station),
            (point(2, 0, 0, 0), called("서울역")),
            (point(2, 0, 3, 3), called("남대문")),
            (point(3, 0, 2, 0), called("")),
            (point(5, 1, 0, 3), station),
            (point(6, 1, 0, 0), called("Seoul Station")),
        ],
    );
    assert_eq!(decode(&encode(&page).unwrap()).unwrap(), page);
    // A place standing in two cells is held once, the same name of another
    // kind is another place, a cell of two places of one class is shown by
    // the first by name and stands where it does, and a nameless one by none.
    assert_eq!(page.places.len(), 3);
    assert_eq!(
        page.cells.iter().map(|c| page.place(c)).collect::<Vec<_>>(),
        [Some(station), Some(called("남대문")), None, Some(station), Some(called("Seoul Station"))]
    );
    assert_eq!(page.cells.iter().map(|c| c.at).collect::<Vec<_>>(), [[1, 2], [3, 3], [2, 0], [0, 3], [0, 0]]);
}

/// A lone place is its cell's at every level: a parent with one place under
/// it has that place, its kind with it, and stands it on the same z16 ground
/// cell its leaf did; a parent with two counts both and is shown by one.
#[test]
fn a_lone_place_folds_up_with_its_kind_and_its_ground_cell() {
    let at = point(5, 4, 3, 1);
    let ground = GROUND.cell_of(0, &at);
    let empty = PoiPage::default();
    let tower = Place {
        name: "Namsan Tower",
        kind: kind_of("attraction"),
        rank: 7,
    };
    let mut below = (leaf(), PoiPage::of_places(0, [(at, tower)]));
    for level in 1..=TOP_LEVEL {
        let p = LevelPage::over(level, below.0.page);
        let up = PoiPage::fold_page(p, |k| Some(if k == below.0 { &below.1 } else { &empty })).unwrap();
        assert_eq!(up.cells.len(), 1);
        let c = up.cells[0];
        assert_eq!(up.place(&c), Some(tower));
        assert_eq!(POI_LATTICE.cell_at(p, c.cell).inner_at(GROUND, 0, c.at), ground, "level {level}");
        below = (p, up);
    }
    let two = PoiPage::of_places(0, [(point(0, 0, 0, 0), called("A")), (point(1, 0, 0, 0), called("B"))]);
    let p = LevelPage::over(1, leaf().page);
    let pair_up = PoiPage::fold_page(p, |k| Some(if k == leaf() { &two } else { &empty })).unwrap();
    assert_eq!(pair_up.cells[0].count, 2);
    assert_eq!(pair_up.place(&pair_up.cells[0]), Some(called("A")));
}

/// A page settles a child where no cell over it holds a crowd: a lone place
/// and an empty stretch are said whole a level up, and only a crowd comes
/// apart under it.
#[test]
fn a_page_settles_every_child_but_one_under_a_crowd() {
    let p = LevelPage::over(1, leaf().page);
    let [crowd, lone, empty, _] = p.children();
    let at = |k: LevelPage, dx: u32| POI_LATTICE.cell_of(0, &POI_LATTICE.cell_base(k, 0, 0)).inner_at(GROUND, 0, [dx, 0]).mid();
    let leaves = [
        (crowd, PoiPage::of_places(0, [(at(crowd, 0), called("A")), (at(crowd, 1), called("B"))])),
        (lone, PoiPage::of_places(0, [(at(lone, 0), called("C"))])),
    ];
    let none = PoiPage::default();
    let page = PoiPage::fold_page(p, |k| Some(leaves.iter().find(|(q, _)| *q == k).map_or(&none, |(_, page)| page))).unwrap();
    assert!(!page.settles(p, crowd));
    assert!(page.settles(p, lone));
    assert!(page.settles(p, empty));
}

/// A crowd is shown by its most prominent place, wherever it stands in the
/// cell, and a parent by the most prominent of its children's: folded from
/// the leaves it is what a page made at that level from the places has.
#[test]
fn a_crowd_is_shown_by_its_most_prominent_place() {
    let station = Place {
        name: "Gangnam",
        kind: kind_of("subway"),
        rank: 4,
    };
    let cafe = Place { name: "A", kind: kind_of("cafe"), rank: 100 };
    let places = [(point(0, 0, 0, 0), cafe), (point(0, 0, 2, 0), called("B")), (point(1, 0, 5, 0), station)];
    let leaf_page = PoiPage::of_places(0, places);
    assert_eq!(
        leaf_page.cells.iter().map(|c| (c.count, leaf_page.place(c), c.at)).collect::<Vec<_>>(),
        [(2, Some(cafe), [0, 0]), (1, Some(station), [5, 0])]
    );
    let p = LevelPage::over(1, leaf().page);
    let empty = PoiPage::default();
    let folded = PoiPage::fold_page(p, |k| Some(if k == leaf() { &leaf_page } else { &empty })).unwrap();
    assert_eq!(folded, PoiPage::of_places(1, places));
    // The station, 64 ground cells and 5 in from the parent's corner.
    assert_eq!((folded.cells[0].count, folded.place(&folded.cells[0]), folded.cells[0].at), (3, Some(station), [69, 0]));
}

/// What the wire cannot hold is refused: a place past the pool, a name split
/// across a character or past `NAME_CHARS`, a kind past the theme.
#[test]
fn a_page_that_names_no_place_or_splits_a_character_is_refused() {
    let crowd = PoiPage::of_places(0, [(point(3, 0, 0, 0), called("A")), (point(3, 0, 1, 1), called("B")), (point(5, 0, 0, 0), called("C"))]);
    let mut named = crowd;
    named.cells[0].place = 9;
    assert!(encode(&named).is_err());
    let good = encode(&PoiPage::of_places(0, [(point(1, 0, 0, 0), called("역")), (point(2, 0, 0, 0), called("a"))])).unwrap();
    let mut split = good.clone();
    // The first name's end moved into the middle of its one character.
    let at = 16 + 2 * CELL_BYTES;
    split[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(decode(&split).is_err());
    let mut unknown = good;
    // The first place's kind, past the theme's last.
    unknown[at + 2 * 4] = KIND_CATEGORIES.len() as u8 + 1;
    assert!(decode(&unknown).is_err());
    let mut bytes = encode(&PoiPage::of_places(0, [(point(1, 0, 0, 0), called("a"))])).unwrap();
    // One code point past the bound, written in over the pool's one name.
    let name = "a".repeat(NAME_CHARS + 1);
    bytes.truncate(bytes.len() - 1);
    bytes.extend_from_slice(name.as_bytes());
    bytes[12..16].copy_from_slice(&(name.len() as u32).to_le_bytes());
    let ends = 16 + CELL_BYTES;
    bytes[ends..ends + 4].copy_from_slice(&(name.len() as u32).to_le_bytes());
    assert!(decode(&bytes).is_err());
}
