// pxl-geo-bake/geocode.test.rs

use std::collections::BTreeMap;

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
            PlacePart::new(PlaceRank::Region, "경기도".to_string(), BTreeMap::from([("en".to_string(), "Gyeonggi-do".to_string())])),
            plain,
        ],
    };
    let json = serde_json::to_string(&place).expect("a place encodes");
    assert_eq!(serde_json::from_str::<Place>(&json).expect("and decodes"), place);
}
