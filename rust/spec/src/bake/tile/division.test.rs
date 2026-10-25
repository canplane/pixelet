// pxl-spec/bake/tile/division.test.rs

use super::*;

fn division(subtype: Subtype, prominence: u8) -> Division {
    Division {
        id: String::new(),
        subtype,
        name: String::new(),
        names: Vec::new(),
        parent: NONE,
        prominence,
        area: true,
    }
}

/// A web map's zoom as the cell zoom whose cell is its pixel.
fn web(zoom: i32) -> Zoom {
    cell_zoom_of(zoom)
}

#[test]
fn a_prominence_places_the_name_by_fives_from_the_top_down_to_ten() {
    assert_eq!(division(Subtype::Locality, 100).shown_from(), web(1));
    assert_eq!(division(Subtype::Locality, 96).shown_from(), web(1));
    assert_eq!(division(Subtype::Locality, 95).shown_from(), web(2));
    assert_eq!(division(Subtype::Locality, 60).shown_from(), web(9));
    assert_eq!(division(Subtype::Locality, 1).shown_from(), web(10));
}

#[test]
fn without_a_prominence_the_subtype_places_the_name() {
    assert_eq!(division(Subtype::Country, 0).shown_from(), web(0));
    assert_eq!(division(Subtype::Dependency, 0).shown_from(), web(0));
    assert_eq!(division(Subtype::Region, 0).shown_from(), web(4));
    assert_eq!(division(Subtype::County, 0).shown_from(), web(8));
    assert_eq!(division(Subtype::Neighborhood, 0).shown_from(), web(10));
}
