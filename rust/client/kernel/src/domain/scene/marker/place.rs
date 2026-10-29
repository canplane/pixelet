// pxl-kernel/domain/scene/marker/place.rs

//! The place layer's markers, read from its place tiles: each place a tile
//! holds as Carto draws it at the zoom of Carto's the view is at where it
//! stands, the camera grid's cell at its own point, a dot or an icon in its
//! category's fill, named from its category's label zoom. A place stands at
//! its ground leaf cell's center on the surface a ray down meets first at
//! the detail drawn there, and a tap on it goes there.

use std::rc::Rc;

use super::arrangement::{cell_zoom, surface, Brick, Instance, Key, Went};
use crate::domain::layer::place::carto_zoom;
use crate::domain::query::{cell_at, World};
use crate::domain::scene::marker::design::Design;
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, INK, LABEL_ZOOM};
use pxl_spec::bake::tile::place::{stands, symbol_of, PLACE_FIRST, PLACE_LATTICE};
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::coord::zoom::{idx_of, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, tile_rows, Cell};
use pxl_spec_core::wrap_index;

/// A place read from a drawn brick: what Carto shows of it, the ground leaf
/// cell it stands in, the finest camera grid cell its brick draws, which
/// its tile was read at, and the surface it stood on as last asked, of the
/// detail it was asked at.
pub(super) struct Place {
    pub(super) brick: Brick,
    key: Key,
    category: u16,
    from: Zoom,
    name: Option<Rc<str>>,
    ground: Cell,
    finest: Zoom,
    top: Option<(Zoom, Option<f64>)>,
}

/// The places standing in `brick`, read where its cells are tiles' cells
/// at its finest, each read from the tile that says it exactly: those Carto
/// shows at the zoom those cells are drawn at, and so at none coarser
/// either (`point_of`).
pub(super) fn read(brick: Brick, found: &mut Vec<Place>) {
    let finest = cell_zoom(brick);
    let zoom = carto_zoom(finest);
    if zoom < PLACE_FIRST as i32 {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(finest);
    let (z, column) = brick;
    // The tile over the column, or its tiles under it: `n` a side from the first.
    let ([x0, y0], n) = if t <= z {
        (ancestor_of(column, z - t), 1)
    } else {
        (first_descendant_of(column, t - z), 1 << (t - z))
    };
    let tiles = (y0..y0 + n)
        .filter(|y| tile_rows(t).contains(y))
        .flat_map(|y| (x0..x0 + n).map(move |x| tile_at(t, [x, y])));
    // The brick as the world names it, once round the world.
    let named = [wrap_index(z, column[0]), column[1]];
    World::now().places_in(tiles, &mut |src, place, h| {
        if point_of(h.place.category, h.from, None, zoom).is_none() {
            return;
        }
        let ground = stands(src, h.at);
        let [x, y, _] = idx_of(z, &ground.mid());
        if [x, y] != named {
            return;
        }
        found.push(Place {
            brick,
            key: Key::Place(src, place),
            category: h.place.category,
            from: h.from,
            name: (!h.place.name.is_empty()).then(|| h.place.name.into()),
            ground,
            finest,
            top: None,
        });
    });
}

/// What `read` read, seen from `eye`, into `out`: each place as
/// Carto shows it at the zoom the camera grid's cell where it stands is,
/// no finer than its tile was read at, standing on what is drawn there.
pub(super) fn show(read: &mut [Place], eye: V3, out: &mut Vec<Instance>) {
    for p in read {
        // Its cell is the grid's at what it stands on: as last asked, or
        // first at its brick's finest.
        let first = match p.top {
            Some((_, top)) => top,
            None => top_at(p, GROUND.read_zoom(p.finest)),
        };
        let [x, y, _] = q3::unpack(&p.ground.mid());
        let cell = cell_at(eye, [x, y, first.unwrap_or(0.0)]).min(p.finest);
        let Some(design) = point_of(p.category, p.from, p.name.as_ref(), carto_zoom(cell)) else {
            continue;
        };
        let Some(top) = top_at(p, GROUND.read_zoom(cell)) else {
            continue;
        };
        let stands = p.ground.center(top);
        out.push(Instance {
            key: p.key,
            design,
            stands,
            tap: Some(Went::To(stands)),
        });
    }
}

/// The height a ray down over `p`'s cell first meets what is drawn at
/// `detail`, kept with the detail it was asked at; none where nothing there
/// is known.
fn top_at(p: &mut Place, detail: Zoom) -> Option<f64> {
    match p.top {
        Some((at, top)) if at == detail => top,
        _ => {
            let top = surface(&p.ground.mid(), (REACH.max[2], REACH.min[2]), detail);
            p.top = Some((detail, top));
            top
        }
    }
}

/// A place of `category`, shown from `from`, as Carto shows it where its
/// zoom is `zoom`, if it does: as a dot or its icon by its category's
/// zooms, named `name` from its category's label zoom, its name wrapped
/// where it is laid out (`Design::spaced`).
fn point_of(category: u16, from: Zoom, name: Option<&Rc<str>>, zoom: i32) -> Option<Design> {
    let k = usize::from(category.checked_sub(1)?);
    let shows = |z: u8| z != 0 && zoom >= i32::from(z);
    let (icon, dot) = (shows(ICON_ZOOM[k]), shows(DOT_ZOOM[k]));
    if zoom < from as i32 || !(icon || dot) {
        return None;
    }
    Some(Design::Point {
        name: name.filter(|_| shows(LABEL_ZOOM[k])).cloned(),
        wrap: 0,
        icon,
        symbol: symbol_of(category),
        fill: FILL[k],
        ink: INK[k],
    })
}
