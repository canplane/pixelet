// pxl-spec/bake/poi_page.test.rs

use super::*;
use crate::space::pvf3::ivec;

fn leaf() -> LevelPage {
    LevelPage::new(0, ivec::gather(3, -2, 0))
}

/// The corner of POI cell (tx, ty), then `dx`, `dy` z16 cells in.
fn point(tx: u32, ty: u32, dx: u32, dy: u32) -> Pvf3 {
    let base = POI_LATTICE.cell_base(leaf(), tx, ty);
    POI_LATTICE.cell_of(0, &base).inner_at(GROUND, 0, [dx, dy]).mid()
}

fn called<'a>(name: &'a str, id: &'a str) -> Place<'a> {
    Place { name, id, ..Place::default() }
}

/// A cell's count and its places as (name, where it stands).
type Cell<'a> = (u32, Vec<(&'a str, [u32; 2])>);

fn cells(page: &PoiPage) -> Vec<Cell<'_>> {
    page.cells.iter().map(|c| (c.count, page.shown(c).map(|(_, p, at)| (p.name, at)).collect())).collect()
}

#[test]
fn a_page_and_its_places_come_back_as_they_went() {
    let station = Place { name: "Seoul Station", kind: kind_of("subway"), id: "7", ..Place::default() };
    let page = PoiPage::of_places(
        0,
        [
            (point(1, 0, 1, 2), station),
            (point(2, 0, 0, 0), called("서울역", "3")),
            (point(2, 0, 3, 3), called("남대문", "9")),
            (point(3, 0, 2, 0), called("", "4")),
            (point(5, 1, 0, 3), station),
            (
                point(6, 1, 0, 0),
                Place { name: "Seoul Station", kind: 0, id: "8", ..Place::default() },
            ),
        ],
        true,
    );
    assert_eq!(decode(&encode(&page).unwrap()).unwrap(), page);
    // Held once across cells; least id per kind; nameless counted not held.
    assert_eq!(page.places.len(), 3);
    assert_eq!(
        cells(&page),
        [
            (1, vec![("Seoul Station", [1, 2])]),
            (2, vec![("서울역", [0, 0])]),
            (1, vec![]),
            (1, vec![("Seoul Station", [0, 3])]),
            (1, vec![("Seoul Station", [0, 0])]),
        ]
    );
}

/// A lone place stays its cell's at every level, on the same z16 cell.
#[test]
fn a_lone_place_folds_up_with_its_kind_and_its_ground_cell() {
    let at = point(5, 4, 3, 1);
    let ground = GROUND.cell_of(0, &at);
    let empty = PoiPage::default();
    let tower = Place { name: "Namsan Tower", kind: kind_of("attraction"), id: "11", ..Place::default() };
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

/// Only a crowd comes apart a level down.
#[test]
fn a_page_settles_every_child_but_one_under_a_crowd() {
    let p = LevelPage::over(1, leaf().page);
    let [crowd, lone, empty, _] = p.children();
    let at = |k: LevelPage, dx: u32| POI_LATTICE.cell_of(0, &POI_LATTICE.cell_base(k, 0, 0)).inner_at(GROUND, 0, [dx, 0]).mid();
    let leaves = [
        (crowd, PoiPage::of_places(0, [(at(crowd, 0), called("A", "1")), (at(crowd, 1), called("B", "2"))], true)),
        (lone, PoiPage::of_places(0, [(at(lone, 0), called("C", "3"))], true)),
    ];
    let none = PoiPage::default();
    let page = PoiPage::fold_page(p, |k| Some(leaves.iter().find(|(q, _)| *q == k).map_or(&none, |(_, page)| page))).unwrap();
    assert!(!page.settles(p, crowd));
    assert!(page.settles(p, lone));
    assert!(page.settles(p, empty));
}

/// Folded from the leaves, a crowd matches a page built at that level.
#[test]
fn a_crowd_holds_one_place_of_each_kind() {
    let station = Place { name: "Gangnam", kind: kind_of("subway"), id: "50", ..Place::default() };
    let cafe = Place { name: "Cafe", kind: kind_of("cafe"), id: "20", ..Place::default() };
    let places = [(point(0, 0, 0, 0), cafe), (point(0, 0, 2, 0), Place { id: "10", ..cafe }), (point(1, 0, 5, 0), station)];
    let leaf_page = PoiPage::of_places(0, places, true);
    assert_eq!(cells(&leaf_page), [(2, vec![("Cafe", [2, 0])]), (1, vec![("Gangnam", [5, 0])])]);
    let p = LevelPage::over(1, leaf().page);
    let empty = PoiPage::default();
    let folded = PoiPage::fold_page(p, |k| Some(if k == leaf() { &leaf_page } else { &empty })).unwrap();
    assert_eq!(folded, PoiPage::of_places(1, places, true));
    let mut held: Vec<_> = folded.shown(&folded.cells[0]).map(|(_, p, at)| (p.kind, p.id, at)).collect();
    held.sort_unstable();
    assert_eq!(folded.cells[0].count, 3);
    assert_eq!(held, {
        let mut want = vec![(kind_of("cafe"), "10", [2, 0]), (kind_of("subway"), "50", [69, 0])];
        want.sort_unstable();
        want
    });
}

/// What the wire cannot hold is refused.
#[test]
fn a_page_that_names_no_place_or_splits_a_character_is_refused() {
    let page = PoiPage::of_places(
        0,
        [
            (
                point(3, 0, 0, 0),
                Place { name: "A", kind: kind_of("cafe"), id: "1", ..Place::default() },
            ),
            (
                point(3, 0, 1, 1),
                Place { name: "B", kind: kind_of("subway"), id: "2", ..Place::default() },
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
    // A name past the bound, and a place run past its cell, are refused on read.
    let long = "a".repeat(NAME_CHARS + 1);
    assert!(decode(&lanes(&[1], &[1], &[0, 1], &[0], &[""], &[&long])).is_err());
    assert!(decode(&lanes(&[1], &[1], &[0, 2], &[0], &[""], &["a"])).is_err());
}

/// A page as the wire lays it, written lane by lane past `encode`'s checks.
fn lanes(cells: &[u32], counts: &[u32], starts: &[u32], shown: &[u32], kinds: &[&str], names: &[&str]) -> Vec<u8> {
    let (partial, at) = (vec![0u8; cells.len()], vec![0u16; 2 * shown.len()]);
    let ids: Vec<&str> = names.iter().map(|_| "x").collect();
    let named = NameLanes::of(names.iter().map(|&n| (n, [])));
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(cells)).u32_array(Some(counts)).u8_array(Some(&partial)).u32_array(Some(starts));
        b.u32_array(Some(shown)).u16_array(Some(&at)).strs(Some(kinds)).strs(Some(&ids));
        named.write(b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default()).unwrap()
}

/// A source's id, a kind this table does not know and names by language
/// come back as the source gave them; the unknown kind reads as none.
#[test]
fn what_any_source_gives_comes_back() {
    let langs = Strings::of(["en", "Seoul Station", "ja", "ソウル駅"]);
    let station = Place { name: "서울역", langs: Langs::all(&langs), kind: kind_of("subway"), id: "ChIJ3S-JXmauEmsRUcIaWtf4MzE" };
    let page = PoiPage::of_places(0, [(point(1, 0, 0, 0), station)], true);
    let back = decode(&encode(&page).unwrap()).unwrap();
    assert_eq!(back, page);
    assert_eq!(back.places.get(0), station);
    assert_eq!(decode(&lanes(&[1], &[1], &[0, 1], &[0], &["a kind of tomorrow"], &["a"])).unwrap().places.get(0).kind, 0);
}

/// A partial count stays a floor at every level over it.
#[test]
fn a_floor_stays_a_floor_as_it_folds() {
    let some = PoiPage::of_places(0, [(point(0, 0, 0, 0), called("A", "1"))], false);
    let all = PoiPage::of_places(0, [(point(0, 0, 0, 0), called("A", "1"))], true);
    assert_eq!(decode(&encode(&some).unwrap()).unwrap(), some);
    let p = LevelPage::over(1, leaf().page);
    let empty = PoiPage::default();
    for (leaf_page, partial) in [(&some, true), (&all, false)] {
        let up = PoiPage::fold_page(p, |k| Some(if k == leaf() { leaf_page } else { &empty })).unwrap();
        assert_eq!(up.cells[0].partial, partial);
        assert_eq!(up.settles(p, leaf()), !partial);
    }
}
