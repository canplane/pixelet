// pxl-core/domain/scene/marker/place.rs

//! The place layer's markers, read from its place tiles. Each place a tile
//! holds is drawn as Carto draws it at the Carto zoom that matches the camera
//! grid's cell at the place's own point: a dot or an icon in its category's
//! fill, named from its category's label zoom on. A place stands at the
//! center of its ground leaf cell, on the first surface a ray down meets at
//! the detail drawn there, and a tap on it goes there.

use pxl_spec::space::layer::quadrant::Quadrant;
use std::rc::Rc;

use super::arrangement::{cell_zoom, surface, Brick, Instance, Key, MarkerHit};
use crate::domain::layer::place::carto_zoom;
use crate::domain::query::{cell_zoom_at, World};
use crate::domain::scene::marker::design::Design;
use pxl_engine::wrap_idx;
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, INK, LABEL_ZOOM};
use pxl_spec::bake::tile::place::{stands, symbol_of, PLACE_FIRST, PLACE_LATTICE};
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::BOUNDS;
use pxl_spec::space::coord::zoom::{idx_of, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::{quad_rows, Cell};

/// A place read from a drawn brick: what Carto shows of it, the ground leaf
/// cell it stands in, and the finest camera grid cell its brick draws, which
/// its tile was read at (`finest`). `top` keeps the surface it stood on when
/// last asked, with the detail it was asked at.
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

/// Reads the places standing in `brick`, from the tiles whose cells are its
/// cells at its finest zoom, each from the tile that places it exactly. Only
/// the places Carto shows at the zoom those cells are drawn at are read
/// (`point_of`). A place not shown there is shown at no coarser zoom either.
pub(super) fn read(brick: Brick, found: &mut Vec<Place>) {
    let finest = cell_zoom(brick);
    let zoom = carto_zoom(finest);
    if zoom < PLACE_FIRST as i32 {
        return;
    }
    let t = PLACE_LATTICE.read_zoom(finest);
    let (z, column) = brick;
    // The tile over the column, or the tiles under it: `n` of them along each
    // side, from the first.
    let ([x0, y0], n) = if t <= z {
        (ancestor_of(column, z - t), 1)
    } else {
        (first_descendant_of(column, t - z), 1 << (t - z))
    };
    let tiles = (y0..y0 + n)
        .filter(|y| quad_rows(t).contains(y))
        .flat_map(|y| (x0..x0 + n).map(move |x| Quadrant::new(t, [x, y])));
    // The brick's column as the world names it, wrapped once round the
    // world.
    let named = [wrap_idx(z, column[0]), column[1]];
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

/// Writes into `out` the places `read` read, seen from `eye`. Each shows as
/// Carto shows it at the zoom of the camera grid's cell where it stands, no
/// finer than its tile was read at, and it stands on what is drawn there.
pub(super) fn show(read: &mut [Place], eye: V3, out: &mut Vec<Instance>) {
    for p in read {
        // Its cell is the grid's cell at what it stands on. That surface is
        // the one last asked, or, on the first ask, the one at its brick's
        // finest detail.
        let first = match p.top {
            Some((_, top)) => top,
            None => top_at(p, GROUND.read_zoom(p.finest)),
        };
        let [x, y, _] = q3::unpack(&p.ground.mid());
        let cell_zoom = cell_zoom_at(eye, [x, y, first.unwrap_or(0.0)]).min(p.finest);
        let Some(design) = point_of(p.category, p.from, p.name.as_ref(), carto_zoom(cell_zoom))
        else {
            continue;
        };
        let Some(top) = top_at(p, GROUND.read_zoom(cell_zoom)) else {
            continue;
        };
        let pos = p.ground.center(top);
        out.push(Instance {
            key: p.key,
            design,
            pos,
            tap: Some(MarkerHit::To(pos)),
        });
    }
}

/// The height at which a ray down over `p`'s cell first meets what is drawn
/// at `detail`, or None where nothing there is known. The answer is kept with
/// the detail it was asked at.
fn top_at(p: &mut Place, detail: Zoom) -> Option<f64> {
    match p.top {
        Some((at, top)) if at == detail => top,
        _ => {
            let top = surface(&p.ground.mid(), (BOUNDS.max[2], BOUNDS.min[2]), detail);
            p.top = Some((detail, top));
            top
        }
    }
}

/// A place of `category`, shown from zoom `from` on, as Carto shows it at
/// `zoom`, or None where Carto does not show it. It shows as a dot or as its
/// icon by its category's zooms, and it is named `name` from its category's
/// label zoom on. Its name is wrapped where it is laid out
/// (`Design::with_spacing`).
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
