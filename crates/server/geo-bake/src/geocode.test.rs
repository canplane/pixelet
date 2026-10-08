// pxl-geo-bake/geocode.test.rs

use std::collections::BTreeMap;

use pxl_spec::bake::division_tile::{Division, NONE};
use pxl_spec::division::{DivisionPart, Subtype};

use super::DivisionDict;

fn division(subtype: Subtype, name: &str) -> Division {
    Division {
        subtype,
        name: name.to_string(),
        names: Vec::new(),
        parent: NONE,
    }
}

#[test]
fn a_dictionary_numbers_each_parent_by_its_own_place_and_one_it_lacks_as_none() {
    let dict = DivisionDict::of(vec![
        ("c".into(), Some("r".into()), division(Subtype::County, "성남시")),
        ("r".into(), Some("k".into()), division(Subtype::Region, "경기도")),
    ]);
    assert_eq!(dict.number("r"), Some(1));
    assert_eq!(dict.divisions[0].parent, 1, "a parent read after its child is still found");
    assert_eq!(dict.divisions[1].parent, NONE, "a parent the rows lack is none");
}

#[test]
fn a_part_survives_the_wire_and_says_nothing_it_need_not() {
    let plain = DivisionPart {
        subtype: Subtype::County,
        name: "분당구".to_string(),
        names: BTreeMap::new(),
        approx: false,
    };
    let json = serde_json::to_string(&plain).expect("a part encodes");
    assert!(!json.contains("approx"), "a part that is not an approximation says nothing: {json}");
    assert!(!json.contains("names"), "an empty name map is absent rather than empty: {json}");
    assert!(json.contains("\"county\""), "a subtype is Overture's word: {json}");
    assert_eq!(serde_json::from_str::<DivisionPart>(&json).expect("and decodes"), plain);
}
