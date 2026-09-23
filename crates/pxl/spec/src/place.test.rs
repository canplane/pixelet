// pxl-spec/place.test.rs

use super::*;

fn part(rank: PlaceRank, name: &str) -> PlacePart {
    PlacePart::new(rank, name.to_string(), BTreeMap::new())
}

/// 성남시 분당구 수내동, as the divisions give it.
fn sunae() -> Place {
    Place {
        parts: vec![part(PlaceRank::Region, "경기도"), part(PlaceRank::County, "성남시"), part(PlaceRank::Neighbourhood, "수내동")],
    }
}

#[test]
fn a_reader_cuts_the_parts_at_the_rank_its_zoom_asks_for() {
    let place = sunae();
    let names = |deepest| place.to(deepest).map(|p| p.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names(PlaceRank::Building), ["경기도", "성남시", "수내동"]);
    assert_eq!(names(PlaceRank::County), ["경기도", "성남시"]);
    assert_eq!(names(PlaceRank::Region), ["경기도"]);
    assert!(names(PlaceRank::Country).is_empty(), "nothing in Korea is ranked as a country");
}

#[test]
fn a_rung_is_the_number_it_is_stored_as() {
    for rank in PlaceRank::ALL {
        assert_eq!(PlaceRank::of(rank as u8), Some(rank));
    }
    // The gaps are the point: a provider with a level between two of ours
    // has a number to be given, and no existing rung moves to make room.
    assert_eq!(PlaceRank::of(5), None);
    assert_eq!(PlaceRank::of(255), None);
}

#[test]
fn a_name_falls_back_to_the_providers_own_where_the_language_has_none() {
    let mut names = BTreeMap::new();
    names.insert("en".to_string(), "Bundang-gu".to_string());
    let p = PlacePart::new(PlaceRank::County, "분당구".to_string(), names);
    assert_eq!(p.in_language("en"), "Bundang-gu");
    assert_eq!(p.in_language("ko"), "분당구", "the provider's own name, not nothing");
    assert_eq!(p.in_language("ja"), "분당구");
}

#[test]
fn an_approximate_part_says_so() {
    assert!(!part(PlaceRank::Neighbourhood, "수내동").approx);
    assert!(part(PlaceRank::Neighbourhood, "수내동").approx().approx);
}

#[test]
fn a_zoom_asks_for_the_rung_its_scale_shows() {
    let at = PlaceRank::at_zoom;
    assert_eq!(at(3.0), PlaceRank::Country);
    assert_eq!(at(6.0), PlaceRank::Region);
    assert_eq!(at(11.0), PlaceRank::City);
    assert_eq!(at(14.0), PlaceRank::Neighbourhood, "where a 동 is named");
    assert_eq!(at(19.0), PlaceRank::Building);
    // The whole ladder rather than none of it, so a broken zoom shows a
    // label rather than nothing.
    assert_eq!(at(f64::NAN), PlaceRank::Building);
    assert_eq!(at(f64::NEG_INFINITY), PlaceRank::Country);
}
