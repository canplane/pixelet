// pxl-core/domain/scene/marker/place.rs

//! The place layer's markers, read from its place tiles. Each place a tile
//! holds is drawn as Carto draws it at the Carto zoom that matches the camera
//! grid's cell at the place's own point: a dot or an icon in its category's
//! fill, named from its category's label zoom on. A place stands at the
//! center of its ground leaf cell, on the surface drawn there at the size
//! the eye sets, and a tap on it goes there.

use fnv::FnvHashMap;
use pxl_spec::space::layer::lattice::Cell;
use pxl_spec::space::layer::quadrant::{Quadrant, quad_rows};
use std::rc::Rc;

use super::{Brick, Instance, Key, MarkerHit, cell_zoom};
use crate::domain::layer::place::carto_zoom;
use crate::domain::query::World;
use crate::domain::query::change::Memo;
use crate::domain::scene::marker::design::Design;
use crate::sys::sched;
use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, INK, LABEL_ZOOM};
use pxl_spec::bake::tile::place::{PLACE_FIRST, PLACE_LATTICE, stands, symbol_of};
use pxl_spec::space::coord::world::{idx_of, wrap_idx};
use pxl_spec::space::coord::zoom::{Zoom, idx_at};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};

/// A place read from a drawn brick: what Carto shows of it, the ground leaf
/// cell it stands in, and the finest camera grid cell its brick draws, which
/// its tile was read at (`finest`). `top` is what it shows by: the surface
/// drawn under it and the cell zoom the ground is drawn with there, None
/// where nothing was found. It is its own answer (`surface`), or, until
/// that is asked, that of the same place as it stood before (`hand_held`).
pub(super) struct Place {
    pub(super) brick: Brick,
    kind: u16,
    first_zoom: Zoom,
    name: Option<Rc<str>>,
    ground: Cell,
    finest: Zoom,
    top: Option<(f64, Zoom)>,
    surface: Memo<(), Option<(f64, Zoom)>>,
}

impl Place {
    /// The brick of `zoom` it stands in, on its own brick's lap of X.
    pub(super) fn brick_at(&self, zoom: Zoom) -> Brick {
        let (z, column) = self.brick;
        let lap = column[0] - wrap_idx(z, column[0]);
        let [x, y, _] = idx_of(zoom, &self.ground.mid());
        (zoom, [x + idx_at(z, lap, zoom), y])
    }
}

/// Reads the places standing in `brick`, from the tiles whose cells are its
/// cells at its finest zoom, each from the tile that places it exactly. Only
/// the places Carto shows at the zoom those cells are drawn at are read
/// (`point_of`). A place not shown there is shown at no coarser zoom either.
/// The brick is read whole or not at all: where one of its tiles is still to
/// come, nothing is found and false is returned.
pub(super) fn read(brick: Brick, found: &mut Vec<Place>) -> bool {
    let finest = cell_zoom(brick);
    let zoom = carto_zoom(finest);
    if zoom < PLACE_FIRST as i32 {
        return true;
    }
    let found_cnt = found.len();
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
    let whole = World::now().places_in(tiles, &mut |src, h| {
        if point_of(h.kind, h.first_zoom, None, zoom).is_none() {
            return;
        }
        let ground = stands(src, h.off);
        let [x, y, _] = idx_of(z, &ground.mid());
        if [x, y] != named {
            return;
        }
        found.push(Place {
            brick,
            kind: h.kind,
            first_zoom: h.first_zoom,
            name: (!h.name.is_empty()).then(|| h.name.into()),
            ground,
            finest,
            top: None,
            surface: Memo::NONE,
        });
    });
    if !whole {
        found.truncate(found_cnt);
    }
    whole
}

/// Hands each place read now, those past the first `held`, what the same
/// place stood on, so it shows as it did until its own is asked.
pub(super) fn hand_held(read: &mut [Place], held: usize) {
    let (before, now) = read.split_at_mut(held);
    if before.is_empty() || now.is_empty() {
        return;
    }
    let was: FnvHashMap<Cell, (f64, Zoom)> = before
        .iter()
        .filter_map(|p| Some((p.ground, p.top?)))
        .collect();
    for p in now {
        if let Some(&top) = was.get(&p.ground) {
            p.top = Some(top);
        }
    }
}

/// Writes into `out` the places `read` read. Each shows as Carto shows it at
/// the zoom of the cell the ground is drawn with under it, no finer than its
/// tile was read at, and it stands on what is drawn there: a marker is
/// tested against the world's depth at its anchor (pxl-render
/// pass/marker.rs), so one standing on a finer ground than is drawn would be
/// hidden wherever the drawn ground is higher. A place's surface is asked
/// again only when the eye sets another size for it (`Memo`), under the
/// entry's budget (sys/sched.rs `run`), a place showing by what it holds
/// until its own is asked; one with nothing to show by is asked first.
/// Returns whether the budget stopped the asks before the list's end.
pub(super) fn show(read: &mut [Place], out: &mut Vec<Instance>) -> bool {
    let mut due: Vec<_> = read.iter_mut().filter(|p| !p.surface.keeps(&())).collect();
    due.sort_by_key(|p| p.top.is_some());
    let run = sched::run(due, |p| {
        let at = p.ground.mid();
        p.top = p
            .surface
            .of((), || World::now().surface_drawn(&at).val().flatten());
        true
    });
    for p in read {
        let Some((top, drawn)) = p.top else { continue };
        let carto = carto_zoom(drawn.min(p.finest));
        let Some(design) = point_of(p.kind, p.first_zoom, p.name.as_ref(), carto) else {
            continue;
        };
        let pos = p.ground.center(top);
        out.push(Instance {
            key: Key::Place(p.ground),
            design,
            pos,
            tap: Some(MarkerHit::To(pos)),
        });
    }
    run.stopped_early
}

/// A place of `kind`, shown from zoom `first_zoom` on, as Carto shows it at
/// `zoom`, or None where Carto does not show it. It shows as a dot or as its
/// icon by its category's zooms, and it is named `name` from its category's
/// label zoom on. Its name is wrapped where it is laid out
/// (`Design::with_spacing`).
fn point_of(kind: u16, first_zoom: Zoom, name: Option<&Rc<str>>, zoom: i32) -> Option<Design> {
    let k = usize::from(kind.checked_sub(1)?);
    let shows = |z: u8| z != 0 && zoom >= i32::from(z);
    let (icon, dot) = (shows(ICON_ZOOM[k]), shows(DOT_ZOOM[k]));
    if zoom < first_zoom as i32 || !(icon || dot) {
        return None;
    }
    Some(Design::Point {
        name: name.filter(|_| shows(LABEL_ZOOM[k])).cloned(),
        wrap: 0,
        icon,
        symbol: symbol_of(kind),
        fill: FILL[k],
        ink: INK[k],
    })
}

#[cfg(test)]
#[path = "place.test.rs"]
mod test;
