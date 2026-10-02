// pxl-geo-bake/geocode.test.rs

use super::*;

#[test]
fn a_providers_name_map_becomes_the_answers_own() {
    let common = serde_json::json!({"en": "Bundang-gu", "ko": "분당구"});
    let names = names_of(&Some(common));
    assert_eq!(names.get("en").map(String::as_str), Some("Bundang-gu"));
    assert_eq!(names.get("ko").map(String::as_str), Some("분당구"));
}

#[test]
fn anything_that_is_not_a_name_a_reader_can_be_shown_is_nothing() {
    assert!(names_of(&None).is_empty());
    assert!(names_of(&Some(serde_json::json!("분당구"))).is_empty(), "a bare string names no language");
    assert!(names_of(&Some(serde_json::json!([1, 2]))).is_empty());
    let mixed = names_of(&Some(serde_json::json!({"en": "Bundang-gu", "ko": 3})));
    assert_eq!(mixed.len(), 1, "the readable half is kept");
}

#[test]
fn a_place_survives_the_wire_and_says_nothing_it_need_not() {
    use pxl_spec::place::{Place, PlacePart, PlaceRank};

    let plain = PlacePart::new(PlaceRank::County, "분당구".to_string(), BTreeMap::new());
    let json = serde_json::to_string(&plain).expect("a part encodes");
    assert!(!json.contains("approx"), "a part that is not an approximation says nothing: {json}");
    assert!(!json.contains("names"), "an empty name map is absent rather than empty: {json}");
    assert!(serde_json::to_string(&plain.clone().approx()).expect("a part encodes").contains("approx"));

    let place = Place {
        parts: vec![
            PlacePart::new(PlaceRank::Region, "경기도".to_string(), names_of(&Some(serde_json::json!({"en": "Gyeonggi-do"})))),
            plain,
        ],
    };
    let json = serde_json::to_string(&place).expect("a place encodes");
    assert_eq!(serde_json::from_str::<Place>(&json).expect("and decodes"), place);
}
