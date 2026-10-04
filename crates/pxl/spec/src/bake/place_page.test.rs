// pxl-spec/bake/place_page.test.rs

use super::*;

fn division(rank: PlaceRank, name: &str, en: &str) -> Division {
    Division {
        rank,
        name: name.to_string(),
        names: vec![("en".to_string(), en.to_string())],
    }
}

/// Dictionary places 경기도 → 성남시 → 수내동, and a group-local 백현동.
fn dict() -> Vec<Named> {
    vec![
        division(PlaceRank::Region, "경기도", "Gyeonggi-do"),
        division(PlaceRank::County, "성남시", "Seongnam-si"),
        division(PlaceRank::Neighborhood, "수내동", "Sunae-dong"),
    ]
}

fn group() -> PlaceGroup {
    let mut pages = vec![NO_LABEL; GROUP_PAGES];
    pages[0] = 0;
    pages[GROUP_PAGES - 1] = 1;
    PlaceGroup {
        labels: vec![vec![0, 1, 2], vec![0, 1, APPROX]],
        pages,
        locals: vec![named(PlaceRank::Neighborhood, "백현동", "Baekhyeon-dong")],
    }
}

fn names_at<'a>(g: &PlaceGroupView<'a>, slot: usize, dict: &PlacesView<'a>, lang: &str) -> Vec<&'a str> {
    g.place_of(slot, dict).expect("a page with a label").map(|p| p.in_language(lang)).collect()
}

#[test]
fn a_dictionary_survives_the_wire() {
    let pkt = encode_place_dict(&dict()).expect("encodes");
    let view = open_place_dict(&pkt).expect("opens");
    assert_eq!(view.len(), 3);
    let at = |i| view.at(i).expect("a place the dictionary has");
    assert_eq!((at(0).rank, at(0).name), (PlaceRank::Region, "경기도"));
    assert_eq!(at(1).in_language("en"), "Seongnam-si");
    assert_eq!(at(2).rank, PlaceRank::Neighborhood);
    assert!(view.at(3).is_none(), "and nothing past its end");
}

#[test]
fn a_dictionary_of_nothing_survives_it_too() {
    let pkt = encode_place_dict(&[]).expect("encodes");
    assert!(open_place_dict(&pkt).expect("opens").is_empty());
}

#[test]
fn a_page_reads_as_the_place_its_label_names() {
    let (gp, dp) = (encode_place_group(&group()).expect("encodes"), encode_place_dict(&dict()).expect("encodes"));
    let (g, d) = (open_place_group(&gp).expect("opens"), open_place_dict(&dp).expect("opens"));
    assert_eq!(names_at(&g, 0, &d, "ko"), ["경기도", "성남시", "수내동"], "the provider's own name where the language has none");
    assert_eq!(names_at(&g, 0, &d, "en"), ["Gyeonggi-do", "Seongnam-si", "Sunae-dong"]);
    assert!(g.place_of(0, &d).expect("a page with a label").all(|p| !p.approx));
}

#[test]
fn a_part_a_nearby_place_named_comes_from_the_groups_own_and_says_it_is_one() {
    let (gp, dp) = (encode_place_group(&group()).expect("encodes"), encode_place_dict(&dict()).expect("encodes"));
    let (g, d) = (open_place_group(&gp).expect("opens"), open_place_dict(&dp).expect("opens"));
    let parts: Vec<_> = g.place_of(GROUP_PAGES - 1, &d).expect("a page with a label").collect();
    assert_eq!(parts.iter().map(|p| p.approx).collect::<Vec<_>>(), [false, false, true]);
    // `LOCAL` 0 is the group's first place, not the dictionary's 경기도.
    assert_eq!(parts[2].name, "백현동");
    assert_eq!(parts[2].in_language("en"), "Baekhyeon-dong");
}

#[test]
fn a_page_outside_every_place_the_provider_knows_has_none() {
    let (gp, dp) = (encode_place_group(&group()).expect("encodes"), encode_place_dict(&dict()).expect("encodes"));
    let (g, d) = (open_place_group(&gp).expect("opens"), open_place_dict(&dp).expect("opens"));
    assert!(g.place_of(1, &d).is_none());
    assert!(g.place_of(GROUP_PAGES, &d).is_none(), "and so does a slot no page has");
}

#[test]
fn a_leafs_slot_is_the_low_bits_of_where_it_is() {
    assert_eq!(leaf_slot(0, 0), 0);
    assert_eq!(leaf_slot(1, 0), 1);
    assert_eq!(leaf_slot(0, 1), 1 << GROUP_SHAMT);
    assert_eq!(leaf_slot(15, 15), GROUP_PAGES - 1);
    // A page of the next group lands in the same slot as its own group's first.
    assert_eq!(leaf_slot(16, 16), 0);
    // A negative coordinate is a slot like any other, not a wrap.
    assert_eq!(leaf_slot(-1, -1), GROUP_PAGES - 1);
}

#[test]
fn a_group_that_is_not_the_pages_under_one_is_refused() {
    let pkt = encode_place_group(&PlaceGroup {
        labels: vec![vec![0]],
        pages: vec![0; GROUP_PAGES - 1],
        locals: vec![],
    })
    .expect("encodes");
    assert!(open_place_group(&pkt).is_err());
}

/// A group's lanes written directly, since `encode_place_group` cannot make an inconsistent one.
fn group_pkt(starts: &[u32], flat: &[u32], pages: &[u16], locals: &[Named]) -> Vec<u8> {
    let (ranks, names) = place_lanes(locals);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(starts)).u32_array(Some(flat)).u16_array(Some(pages));
        write_places(b, &ranks, &names);
    }));
    encoder().encode(&t, TreeEncoderOptions::default()).expect("encodes")
}

#[test]
fn a_label_running_past_the_numbers_is_refused() {
    // A truncated body is refused at open.
    let pkt = group_pkt(&[0, 9], &[0], &[NO_LABEL; GROUP_PAGES], &[]);
    assert!(open_place_group(&pkt).is_err());
    // A label whose numbers run backward would read as a negative length.
    let pkt = group_pkt(&[0, 2, 1], &[0, 1], &[NO_LABEL; GROUP_PAGES], &[]);
    assert!(open_place_group(&pkt).is_err());
}

#[test]
fn a_run_of_places_whose_lanes_disagree_is_refused() {
    // A rank with no name.
    let named = [named(PlaceRank::Region, "경기도", "Gyeonggi-do")];
    let (_, names) = place_lanes(&named);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_places(b, &[PlaceRank::Region as u8, PlaceRank::County as u8], &names)));
    let pkt = encoder().encode(&t, TreeEncoderOptions::default()).expect("encodes");
    assert!(open_place_dict(&pkt).is_err());
}

/// A local place given as exact reads as exact.
#[test]
fn a_groups_own_place_is_not_an_approximation_unless_said() {
    let mut g = group();
    g.pages[1] = 2;
    let (gp, dp) = (encode_place_group(&g).expect("encodes"), encode_place_dict(&dict()).expect("encodes"));
    let (g, d) = (open_place_group(&gp).expect("opens"), open_place_dict(&dp).expect("opens"));
    let parts: Vec<_> = g.place_of(1, &d).expect("a page with a label").collect();
    assert_eq!(parts.iter().map(|p| (p.name, p.approx)).collect::<Vec<_>>(), [("경기도", false), ("성남시", false), ("백현동", false)]);
}
