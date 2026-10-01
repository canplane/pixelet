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

/// A place of no kind, called `name`, of id `id`.
fn called(name: &str, id: u64) -> Place<'_> {
    Place { name, kind: 0, id }
}

/// Each cell's count and its places, as (name, where it stands).
fn cells(page: &PoiPage) -> Vec<(u32, Vec<(&str, [u32; 2])>)> {
    page.cells.iter().map(|c| (c.count, page.shown(c).map(|(_, p, at)| (p.name, at)).collect())).collect()
}

/// A page survives the wire whole: its cells, their places and where each
/// stands, the pool and the ids.
#[test]
fn a_page_and_its_places_come_back_as_they_went() {
    let station = Place {
        name: "Seoul Station",
        kind: kind_of("subway"),
        id: 7,
    };
    let page = PoiPage::of_places(
        0,
        [
            (point(1, 0, 1, 2), station),
            (point(2, 0, 0, 0), called("서울역", 3)),
            (point(2, 0, 3, 3), called("남대문", 9)),
            (point(3, 0, 2, 0), called("", 4)),
            (point(5, 1, 0, 3), station),
            (
                point(6, 1, 0, 0),
                Place {
                    name: "Seoul Station",
                    kind: 0,
                    id: 8,
                },
            ),
        ],
        true,
    );
    assert_eq!(decode(&encode(&page).unwrap()).unwrap(), page);
    // A place standing in two cells is held once, two of one kind are held
    // as the one of least id, a nameless one is counted and not held, and a
    // name is cut to what a badge shows.
    assert_eq!(page.places.len(), 3);
    assert_eq!(
        cells(&page),
        [
            (1, vec![("Seoul Stati…", [1, 2])]),
            (2, vec![("서울역", [0, 0])]),
            (1, vec![]),
            (1, vec![("Seoul Stati…", [0, 3])]),
            (1, vec![("Seoul Stati…", [0, 0])]),
        ]
    );
}

/// A lone place is its cell's at every level: a parent with one place under
/// it holds that place, its kind and id with it, and stands it on the same
/// z16 ground cell its leaf did.
#[test]
fn a_lone_place_folds_up_with_its_kind_and_its_ground_cell() {
    let at = point(5, 4, 3, 1);
    let ground = GROUND.cell_of(0, &at);
    let empty = PoiPage::default();
    let tower = Place {
        name: "Namsan Tower",
        kind: kind_of("attraction"),
        id: 11,
    };
    let mut below = (leaf(), PoiPage::of_places(0, [(at, tower)], true));
    for level in 1..=TOP_LEVEL {
        let p = LevelPage::over(level, below.0.page);
        let up = PoiPage::fold_page(p, |k| Some(if k == below.0 { &below.1 } else { &empty })).unwrap();
        assert_eq!(up.cells.len(), 1);
        let c = up.cells[0];
        let (_, place, at) = up.shown(&c).next().unwrap();
        assert_eq!(place, tower);
        assert_eq!(POI_LATTICE.cell_at(p, c.cell).inner_at(GROUND, 0, at), ground, "level {level}");
        below = (p, up);
    }
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
        (crowd, PoiPage::of_places(0, [(at(crowd, 0), called("A", 1)), (at(crowd, 1), called("B", 2))], true)),
        (lone, PoiPage::of_places(0, [(at(lone, 0), called("C", 3))], true)),
    ];
    let none = PoiPage::default();
    let page = PoiPage::fold_page(p, |k| Some(leaves.iter().find(|(q, _)| *q == k).map_or(&none, |(_, page)| page))).unwrap();
    assert!(!page.settles(p, crowd));
    assert!(page.settles(p, lone));
    assert!(page.settles(p, empty));
}

/// A crowd holds one place of each kind among it, the one of least id, and a
/// parent the least of its children's: folded from the leaves it is what a
/// page made at that level from the places has.
#[test]
fn a_crowd_holds_one_place_of_each_kind() {
    let station = Place {
        name: "Gangnam",
        kind: kind_of("subway"),
        id: 50,
    };
    let cafe = Place {
        name: "Cafe",
        kind: kind_of("cafe"),
        id: 20,
    };
    let places = [(point(0, 0, 0, 0), cafe), (point(0, 0, 2, 0), Place { id: 10, ..cafe }), (point(1, 0, 5, 0), station)];
    let leaf_page = PoiPage::of_places(0, places, true);
    assert_eq!(cells(&leaf_page), [(2, vec![("Cafe", [2, 0])]), (1, vec![("Gangnam", [5, 0])])]);
    let p = LevelPage::over(1, leaf().page);
    let empty = PoiPage::default();
    let folded = PoiPage::fold_page(p, |k| Some(if k == leaf() { &leaf_page } else { &empty })).unwrap();
    assert_eq!(folded, PoiPage::of_places(1, places, true));
    // Of kinds in the table's order: the cafe of least id, two cells in, and
    // the station, 64 ground cells and 5 in from the parent's corner.
    let mut held: Vec<_> = folded.shown(&folded.cells[0]).map(|(_, p, at)| (p.kind, p.id, at)).collect();
    held.sort_unstable();
    assert_eq!(folded.cells[0].count, 3);
    assert_eq!(held, {
        let mut want = vec![(kind_of("cafe"), 10, [2, 0]), (kind_of("subway"), 50, [69, 0])];
        want.sort_unstable();
        want
    });
}

/// What the wire cannot hold is refused: a place past the pool, a cell's
/// places out of kind order, a name split across a character or past
/// `NAME_CHARS`, a kind past the theme.
#[test]
fn a_page_that_names_no_place_or_splits_a_character_is_refused() {
    let page = PoiPage::of_places(
        0,
        [
            (
                point(3, 0, 0, 0),
                Place {
                    name: "A",
                    kind: kind_of("cafe"),
                    id: 1,
                },
            ),
            (
                point(3, 0, 1, 1),
                Place {
                    name: "B",
                    kind: kind_of("subway"),
                    id: 2,
                },
            ),
        ],
        true,
    );
    let mut past = page.clone();
    past.shown[0].place = 9;
    assert!(encode(&past).is_err());
    let mut swapped = page.clone();
    swapped.shown.swap(0, 1);
    assert!(encode(&swapped).is_err());
    let good = encode(&PoiPage::of_places(0, [(point(1, 0, 0, 0), called("역", 1)), (point(2, 0, 0, 0), called("a", 2))], true)).unwrap();
    // Two cells, two places shown, two places: the ids, then the ends.
    let ends = 20 + 2 * CELL_BYTES + 2 * SHOWN_BYTES + 2 * 8;
    let mut split = good.clone();
    // The first name's end moved into the middle of its one character.
    split[ends..ends + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(decode(&split).is_err());
    let mut unknown = good;
    // The first place's kind, past the theme's last.
    unknown[ends + 2 * 4] = KIND_CATEGORIES.len() as u8 + 1;
    assert!(decode(&unknown).is_err());
    let mut bytes = encode(&PoiPage::of_places(0, [(point(1, 0, 0, 0), called("a", 1))], true)).unwrap();
    // One code point past the bound, written in over the pool's one name.
    let name = "a".repeat(NAME_CHARS + 1);
    bytes.truncate(bytes.len() - 1);
    bytes.extend_from_slice(name.as_bytes());
    bytes[16..20].copy_from_slice(&(name.len() as u32).to_le_bytes());
    let ends = 20 + CELL_BYTES + SHOWN_BYTES + 8;
    bytes[ends..ends + 4].copy_from_slice(&(name.len() as u32).to_le_bytes());
    assert!(decode(&bytes).is_err());
}

/// A count from a source that gave only some of what stands there is a floor
/// at every level over it, and one of a source that gave all is a total.
#[test]
fn a_floor_stays_a_floor_as_it_folds() {
    let some = PoiPage::of_places(0, [(point(0, 0, 0, 0), called("A", 1))], false);
    let all = PoiPage::of_places(0, [(point(0, 0, 0, 0), called("A", 1))], true);
    assert_eq!(decode(&encode(&some).unwrap()).unwrap(), some);
    let p = LevelPage::over(1, leaf().page);
    let empty = PoiPage::default();
    for (leaf_page, partial) in [(&some, true), (&all, false)] {
        let up = PoiPage::fold_page(p, |k| Some(if k == leaf() { leaf_page } else { &empty })).unwrap();
        assert_eq!(up.cells[0].partial, partial);
    }
}
