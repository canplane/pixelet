// pxl-spec/bake/division_tile.test.rs

use super::*;

fn division(subtype: Subtype, name: &str, en: &str, parent: u32) -> Division {
    Division {
        subtype,
        name: name.to_string(),
        names: vec![("en".to_string(), en.to_string())],
        parent,
    }
}

/// 경기도 ← 성남시 ← 수내동, and a group's own 백현동 under 성남시.
fn dict() -> Vec<Division> {
    vec![
        division(Subtype::Region, "경기도", "Gyeonggi-do", NONE),
        division(Subtype::County, "성남시", "Seongnam-si", 0),
        division(Subtype::Locality, "수내동", "Sunae-dong", 1),
    ]
}

fn group() -> DivisionGroup {
    let mut tiles = vec![NONE; GROUP_TILES];
    tiles[0] = 2;
    tiles[1] = LOCAL | APPROX;
    tiles[2] = 1;
    DivisionGroup {
        tiles,
        locals: vec![division(Subtype::Locality, "백현동", "Baekhyeon-dong", 1)],
    }
}

fn opened<'a>(g: &'a [u8], d: &'a [u8]) -> (DivisionGroupView<'a>, DivisionsView<'a>) {
    (open_division_group(g).expect("opens"), open_division_dict(d).expect("opens"))
}

fn names_at<'a>(g: &DivisionGroupView<'a>, slot: usize, dict: &DivisionsView<'a>, lang: &str) -> Vec<&'a str> {
    g.name_of(slot, dict).expect("a tile in a division").map(|p| p.in_language(lang)).collect()
}

#[test]
fn a_dictionary_survives_the_wire() {
    let pkt = encode_division_dict(&dict()).expect("encodes");
    let view = open_division_dict(&pkt).expect("opens");
    assert_eq!(view.len(), 3);
    let at = |i| view.at(i).expect("a division the dictionary has");
    assert_eq!((at(0).subtype, at(0).name, at(0).parent), (Subtype::Region, "경기도", NONE));
    assert_eq!((at(1).in_language("en"), at(1).parent), ("Seongnam-si", 0));
    assert!(view.at(3).is_none(), "and nothing past its end");
    assert!(open_division_dict(&encode_division_dict(&[]).expect("encodes")).expect("opens").is_empty());
}

#[test]
fn a_tile_reads_as_its_division_and_every_one_above_it() {
    let (gp, dp) = (encode_division_group(&group()).expect("encodes"), encode_division_dict(&dict()).expect("encodes"));
    let (g, d) = opened(&gp, &dp);
    assert_eq!(names_at(&g, 0, &d, "ko"), ["경기도", "성남시", "수내동"], "the provider's own name where the language has none");
    assert_eq!(names_at(&g, 0, &d, "en"), ["Gyeonggi-do", "Seongnam-si", "Sunae-dong"]);
    assert_eq!(names_at(&g, 2, &d, "ko"), ["경기도", "성남시"], "a tile may be in a broad division only");
    assert!(g.name_of(3, &d).is_none(), "a tile in none has no name");
}

#[test]
fn a_groups_own_division_hangs_under_the_dictionarys_and_says_it_is_near() {
    let (gp, dp) = (encode_division_group(&group()).expect("encodes"), encode_division_dict(&dict()).expect("encodes"));
    let (g, d) = opened(&gp, &dp);
    let parts: Vec<(&str, bool)> = g.name_of(1, &d).expect("named").map(|p| (p.name, p.approx)).collect();
    assert_eq!(parts, [("경기도", false), ("성남시", false), ("백현동", true)], "only the nearest is approximate");
}

#[test]
fn a_dictionary_whose_walk_up_leaves_it_or_loops_is_refused() {
    let past = [division(Subtype::County, "성남시", "Seongnam-si", 5)];
    assert!(open_division_dict(&encode_division_dict(&past).expect("encodes")).is_err());
    let looped = [division(Subtype::Region, "경기도", "Gyeonggi-do", 1), division(Subtype::County, "성남시", "Seongnam-si", 0)];
    assert!(open_division_dict(&encode_division_dict(&looped).expect("encodes")).is_err());
}

#[test]
fn a_group_that_is_not_the_tiles_under_one_or_names_past_its_own_is_refused() {
    let mut short = group();
    short.tiles.pop();
    assert!(open_division_group(&encode_division_group(&short).expect("encodes")).is_err());
    let mut past = group();
    past.tiles[5] = LOCAL | 1;
    assert!(open_division_group(&encode_division_group(&past).expect("encodes")).is_err());
}

