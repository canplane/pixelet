// pxl-spec/bake/tile/place.test.rs

use super::*;
use crate::space::layer::lattice::Cells;

fn leaf() -> Tile {
    Tile::new(LEAF_ZOOM, [6, 4])
}

/// The middle of ground leaf cell (x, y) of `leaf()`.
fn point(x: u32, y: u32) -> Q3 {
    let first = GROUND.cell_at(leaf(), 0);
    first.up(0).inner_at(GROUND, LEAF_ZOOM, [x, y]).mid()
}

fn called<'a>(name: &'a str, category: &str, id: &'a str) -> Place<'a> {
    Place {
        name,
        category: category_of(category),
        id,
    }
}

fn shown_at(tile: &PlaceTile, zoom: Zoom) -> Vec<&str> {
    tile.iter()
        .filter(|(_, h)| h.from <= zoom)
        .map(|(_, h)| h.place.name)
        .collect()
}

#[test]
fn a_tile_comes_back_as_it_went() {
    let tile = PlaceTile::leaf(
        leaf(),
        [
            (point(1, 2), called("Zürich HB", "train_station", "3")),
            (point(40, 7), called("Cafe", "cafe", "9")),
            (point(41, 7), called("", "hospital", "4")),
        ],
    );
    assert_eq!(decode(&encode(&tile).unwrap()).unwrap(), tile);
}

#[test]
fn a_kind_carto_draws_no_point_for_is_not_held() {
    let tile = PlaceTile::leaf(
        leaf(),
        [(point(1, 2), called("School", "elementary_school", "1"))],
    );
    assert!(tile.is_empty());
}

#[test]
fn a_cell_keeps_its_first_by_the_zoom_its_kind_shows_from_then_id() {
    // Sixteen leaf cells apart: one place's cell through Carto's z17, apart from z18.
    let tile = PlaceTile::leaf(
        leaf(),
        [
            (point(8, 10), called("Cafe", "cafe", "1")),
            (point(24, 10), called("Hospital", "hospital", "9")),
        ],
    );
    assert_eq!(shown_at(&tile, 15), ["Hospital"]);
    // The cafe, shown from z17, never takes the hospital's room, though its id is less.
    assert_eq!(shown_at(&tile, 17), ["Hospital"]);
    assert_eq!(shown_at(&tile, 18), ["Cafe", "Hospital"]);
}

#[test]
fn a_parent_is_what_its_children_show_from_its_zoom_on() {
    let child = PlaceTile::leaf(
        leaf(),
        [
            (point(10, 10), called("Peak", "mountain", "1")),
            (point(300, 10), called("Cafe", "cafe", "2")),
        ],
    );
    let empty = PlaceTile::default();
    let p = leaf().parent();
    let mut children = [&empty; 4];
    children[leaf().slot()] = &child;
    let parent = fold(p, children);
    assert_eq!(shown_at(&parent, p.zoom), ["Peak"]);
    // The cafe shows only deeper, so the leaf stays a tile under it.
    assert!(parent.occupancy.has_child(leaf().slot()));
    assert!(!parent.occupancy.has_child((leaf().slot() + 1) % 4));
}

#[test]
fn every_kind_carto_draws_an_icon_for_ships_its_symbol() {
    for category in 1..=CATEGORIES.len() as u16 {
        if ICON_ZOOM[usize::from(category - 1)] != 0 {
            assert!(
                symbol_of(category).is_some(),
                "{} has no symbol shipped",
                CATEGORIES[usize::from(category - 1)]
            );
        }
    }
}

#[test]
fn no_grid_cell_reads_a_tile_coarser_than_the_coarsest() {
    use crate::space::layer::tile::TOP_ZOOM;
    const {
        assert!(
            crate::space::layer::lattice::coarsest(GROUND) == TOP_ZOOM,
            "the ground is read up to the pyramid's top"
        )
    };
    const {
        assert!(
            crate::space::layer::lattice::coarsest(PLACE_LATTICE) > TOP_ZOOM,
            "places are read no coarser than their icon's cell allows"
        )
    };
}
