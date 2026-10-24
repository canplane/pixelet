// pxl-kernel/scene/place/places.rs

//! The places the place tiles hold, as Carto draws them at the zoom of
//! Carto's the view is at where they stand: a dot or an icon in its
//! category's fill, named from its category's label zoom. A place stands at
//! its ground leaf cell's center on the surface a ray down meets first, and
//! a tap on it goes there.

use super::{surface, Instance, Key, Went};
use crate::billboard::badge::Label;
use crate::chunk::Chunk;
use crate::layer::place::carto_zoom;
use crate::query::World;
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, LABEL_ZOOM, TEXT};
use pxl_spec::bake::tile::place::{stands, symbol_of, Placed, PLACE_FIRST, PLACE_LATTICE};
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, tile_rows};

/// The places standing in `chunk`, read where its cells are tiles' cells,
/// each read from the tile that says it exactly; of them what Carto shows
/// at the zoom those cells are drawn at, as it shows them (`point_of`).
pub(super) fn read(chunk: Chunk, found: &mut Vec<(Chunk, Instance)>) {
    let zoom = carto_zoom(chunk.cell_zoom());
    if zoom < PLACE_FIRST as i32 {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(chunk.cell_zoom());
    let (z, column) = chunk.column();
    // The tile over the column, or its tiles under it: `n` a side from the first.
    let ([x0, y0], n) = if t <= z {
        (ancestor_of(column, z - t), 1)
    } else {
        (first_descendant_of(column, t - z), 1 << (t - z))
    };
    let tiles = (y0..y0 + n).filter(|y| tile_rows(t).contains(y)).flat_map(|y| (x0..x0 + n).map(move |x| tile_at(t, [x, y])));
    let (h0, h1) = chunk.z_range();
    let detail = GROUND.read_zoom(chunk.cell_zoom());
    World::now().places_in(tiles, &mut |src, place, h| {
        let Some(label) = point_of(&h, zoom) else { return };
        let ground = stands(src, h.at);
        if Chunk::holding(chunk.zoom(), &ground.mid()).column() != chunk.named().column() {
            return;
        }
        // Counted only by the chunk the top of what stands there is in, so a
        // column's several chunks do not repeat it.
        let Some(top) = surface(&ground.mid(), (REACH.max[2], REACH.min[2]), detail) else { return };
        if top - 1.0 < h0 || top - 1.0 >= h1 {
            return;
        }
        let stands = ground.center(top);
        found.push((
            chunk,
            Instance {
                key: Key::Place(src, place),
                label,
                stands,
                tap: Some(Went::To(stands)),
            },
        ));
    });
}

/// A place as Carto shows it where its zoom is `zoom`, if it does: from the
/// zoom it is shown from, as a dot or its icon by its category's zooms,
/// named from its category's label zoom, its name wrapped where it is laid
/// out (`Label::spaced`).
fn point_of(h: &Placed, zoom: i32) -> Option<Label> {
    let k = usize::from(h.place.category.checked_sub(1)?);
    let from = |z: u8| z != 0 && zoom >= i32::from(z);
    let (icon, dot) = (from(ICON_ZOOM[k]), from(DOT_ZOOM[k]));
    if zoom < h.from as i32 || !(icon || dot) {
        return None;
    }
    let named = from(LABEL_ZOOM[k]) && !h.place.name.is_empty();
    Some(Label::Point {
        name: named.then(|| h.place.name.into()),
        wrap: 0,
        icon,
        symbol: symbol_of(h.place.category),
        fill: FILL[k],
        ink: TEXT[k],
    })
}
