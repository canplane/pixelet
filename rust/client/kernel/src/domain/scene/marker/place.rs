// pxl-kernel/domain/scene/marker/place.rs

//! The place layer's markers, read from its place tiles: each place a tile
//! holds as Carto draws it at the zoom of Carto's the view is at where it
//! stands, a dot or an icon in its category's fill, named from its
//! category's label zoom. A place stands at its ground leaf cell's center on
//! the surface a ray down meets first, and a tap on it goes there.

use super::arrangement::{cell_zoom, surface, Instance, Key, Square, Went};
use crate::domain::layer::place::carto_zoom;
use crate::domain::query::World;
use crate::domain::scene::marker::design::Design;
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, INK, LABEL_ZOOM};
use pxl_spec::bake::tile::place::{stands, symbol_of, Placed, PLACE_FIRST, PLACE_LATTICE};
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::coord::zoom::index_of;
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, tile_rows};
use pxl_spec_core::wrap_index;

/// What `read` read, as the arrangement takes it.
pub(super) fn shown(read: &[(Square, Instance)]) -> impl Iterator<Item = Instance> + '_ {
    read.iter().map(|(_, s)| s.clone())
}

/// The places standing in `square`, read where its cells are tiles' cells,
/// each read from the tile that says it exactly; of them what Carto shows
/// at the zoom those cells are drawn at, as it shows them (`point_of`).
pub(super) fn read(square: Square, found: &mut Vec<(Square, Instance)>) {
    let cells = cell_zoom(square);
    let zoom = carto_zoom(cells);
    if zoom < PLACE_FIRST as i32 {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(cells);
    let (z, column) = square;
    // The tile over the column, or its tiles under it: `n` a side from the first.
    let ([x0, y0], n) = if t <= z {
        (ancestor_of(column, z - t), 1)
    } else {
        (first_descendant_of(column, t - z), 1 << (t - z))
    };
    let tiles = (y0..y0 + n).filter(|y| tile_rows(t).contains(y)).flat_map(|y| (x0..x0 + n).map(move |x| tile_at(t, [x, y])));
    let detail = GROUND.read_zoom(cells);
    // The square as the world names it, once round the world.
    let named = [wrap_index(z, column[0]), column[1]];
    World::now().places_in(tiles, &mut |src, place, h| {
        let Some(design) = point_of(&h, zoom) else { return };
        let ground = stands(src, h.at);
        let [x, y, _] = index_of(z, &ground.mid());
        if [x, y] != named {
            return;
        }
        let Some(top) = surface(&ground.mid(), (REACH.max[2], REACH.min[2]), detail) else { return };
        let stands = ground.center(top);
        found.push((
            square,
            Instance {
                key: Key::Place(src, place),
                design,
                stands,
                tap: Some(Went::To(stands)),
            },
        ));
    });
}

/// A place as Carto shows it where its zoom is `zoom`, if it does: from the
/// zoom it is shown from, as a dot or its icon by its category's zooms,
/// named from its category's label zoom, its name wrapped where it is laid
/// out (`Design::spaced`).
fn point_of(h: &Placed, zoom: i32) -> Option<Design> {
    let k = usize::from(h.place.category.checked_sub(1)?);
    let from = |z: u8| z != 0 && zoom >= i32::from(z);
    let (icon, dot) = (from(ICON_ZOOM[k]), from(DOT_ZOOM[k]));
    if zoom < h.from as i32 || !(icon || dot) {
        return None;
    }
    let named = from(LABEL_ZOOM[k]) && !h.place.name.is_empty();
    Some(Design::Point {
        name: named.then(|| h.place.name.into()),
        wrap: 0,
        icon,
        symbol: symbol_of(h.place.category),
        fill: FILL[k],
        ink: INK[k],
    })
}
