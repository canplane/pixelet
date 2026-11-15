// pxl-spec/bake/tile/division.test.rs

use super::*;

fn division(kind: Subtype, prominence: u8) -> Division<'static> {
    Division {
        id: "",
        kind,
        name: "",
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
fn prominence_zoom() {
    assert_eq!(division(Subtype::Locality, 100).shown_from(), web(1));
    assert_eq!(division(Subtype::Locality, 96).shown_from(), web(1));
    assert_eq!(division(Subtype::Locality, 95).shown_from(), web(2));
    assert_eq!(division(Subtype::Locality, 60).shown_from(), web(9));
    assert_eq!(division(Subtype::Locality, 1).shown_from(), web(10));
}

#[test]
fn subtype_zoom() {
    assert_eq!(division(Subtype::Country, 0).shown_from(), web(0));
    assert_eq!(division(Subtype::Dependency, 0).shown_from(), web(0));
    assert_eq!(division(Subtype::Region, 0).shown_from(), web(4));
    assert_eq!(division(Subtype::County, 0).shown_from(), web(8));
    assert_eq!(division(Subtype::Neighborhood, 0).shown_from(), web(10));
}

/// A tile holds every name whole and every chain however long, as the bake
/// writes them: one with a long name and a chain longer than there are
/// subtypes reads back as written.
#[test]
fn long_name_and_chain_read() {
    let long = "가".repeat(1000);
    let depth = 2 * Subtype::ALL.len() as u32;
    let chain = (0..depth).map(|k| Division {
        name: &long,
        parent: if k == 0 { NONE } else { k - 1 },
        ..division(Subtype::Locality, 0)
    });
    let tile = DivisionTile::of(vec![depth - 1; CELL_CNT].into(), chain, Occupancy::LEAF);
    let read = decode(&encode(&tile).unwrap()).unwrap();
    assert_eq!(read.name_at(0).count(), depth as usize);
    assert!(read.name_at(0).all(|d| d.name == long));
}
