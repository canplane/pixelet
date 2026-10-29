// pxl-kernel/domain/scene/marker/pin.rs

//! The pins someone put up, as the object store holds them: each its words
//! on a pill at its own cell, or where more than one stands in a pin's cell
//! (`marker_cell`), their count at the cell's middle on a pin's pill in its
//! colors turned over, the pins' own rule (`asset/marker.json`); the one
//! waiting for its words wherever it is. A pin is read with the brick it
//! stands in and shown at the camera grid's cell where it stands, its own
//! point's, so it shows and is counted as the zoom drawn there says.

use core::hash::Hasher;

use super::arrangement::{surface, Brick, Instance, Key, Went};
use crate::domain::layer::object::preview::draft;
use crate::domain::query::{cell_at, World};
use crate::domain::scene::marker::design::Design;
use pxl_render::look::marker::{CNT_FILL, CNT_INK, PILL_FILL, PILL_INK};
use pxl_spec::asset::marker::{marker_cell_of, marker_shows};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::object::is_marker;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

/// A pin's cell (`marker_cell_of`): its zoom and place.
type Cell = (Zoom, [i32; 3]);

/// What holds a pin's instance: its entry, or the pin's cell its
/// count stands for.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Held {
    One(EntryAt),
    Cell(Cell),
}

/// A pin read from a drawn brick: its pill, where it stands and its base;
/// and as the last run showed it, the camera grid's cell zoom at its base
/// and its pin's cell, none where it did not show, and where its cell's
/// count stood, of the cell it was then.
#[derive(Clone)]
pub(super) struct Pin {
    pub(super) brick: Brick,
    at: EntryAt,
    design: Design,
    stands: Q3,
    base: V3,
    grid: Zoom,
    cell: Option<Cell>,
    crowd: Option<(Cell, Q3)>,
}

/// A product choice: the length a one-line pin still reads as a name at.
/// Code points, so a non-BMP character is never cut in half.
const SHOWN_CHARS: usize = 12;

/// A pin's words, cut to `SHOWN_CHARS`, on its pill.
fn pill(words: &str) -> Design {
    let mut shown: String = words.chars().take(SHOWN_CHARS).collect();
    if words.chars().count() > SHOWN_CHARS {
        shown.push('…');
    }
    Design::Pill {
        words: shown.into(),
        fill: PILL_FILL,
        ink: PILL_INK,
    }
}

/// The count of `n` pins in one pin's cell, on a pin's pill in the
/// count's colors.
fn cnt(n: usize) -> Design {
    Design::Pill {
        words: n.to_string().into(),
        fill: CNT_FILL,
        ink: CNT_INK,
    }
}

/// One pin, taken to its entry, which the tool held decides what to do
/// with.
fn one(at: EntryAt, design: Design, stands: Q3) -> Instance {
    Instance {
        key: Key::Pin(Held::One(at)),
        design,
        stands,
        tap: Some(Went::Pin(at, stands)),
    }
}

/// The pins put up over `brick`, whose columns are `columns`, by the brick
/// their base is in, so no pin is another brick's too.
pub(super) fn read(brick: Brick, columns: &Aabb, pins: &mut Vec<Pin>) {
    let world = World::now();
    world.placed_in(columns, &mut |e| {
        if !is_marker(e.state) {
            return;
        }
        let (stands, base) = pin_at(e.id());
        if (0..3).all(|k| (columns.min[k]..columns.max[k]).contains(&base[k])) {
            pins.push(Pin {
                brick,
                at: e.id(),
                design: pill(world.words(e).unwrap_or_default()),
                stands,
                base,
                grid: 0,
                cell: None,
                crowd: None,
            });
        }
    });
}

/// What `pins` show seen from `eye`, into `out`: the one waiting
/// for its words, wherever it is, then each pin where its own point's cell
/// shows a marker (`marker_shows`), a pin's cell's pins but that one
/// together: one is itself, more their count, which a tap takes to where it
/// stands. A cell lies in its brick, finer than it, so a brick's pins are
/// counted among themselves.
pub(super) fn show(pins: &mut [Pin], eye: V3, out: &mut Vec<Instance>) {
    let drafted = draft().map(|d| d.at);
    out.extend(draft().map(|d| one(d.at, pill(&d.words), pin_at(d.at).0)));
    for p in pins.iter_mut() {
        p.grid = cell_at(eye, p.base);
        p.cell = marker_shows(p.grid).then(|| marker_cell_of(p.grid, p.at));
    }
    pins.sort_unstable_by_key(|p| (p.brick, p.cell));
    for run in pins.chunk_by_mut(|a, b| a.brick == b.brick && a.cell == b.cell) {
        let Some(cell) = run[0].cell else { continue };
        let mut left = run.iter().filter(|p| Some(p.at) != drafted);
        let Some(first) = left.next() else { continue };
        let n = 1 + left.count();
        if n == 1 {
            out.push(one(first.at, first.design.clone(), first.stands));
            continue;
        }
        // Where the count stands, asked once the cell it stands for.
        let at = match run.iter().find_map(|p| p.crowd.filter(|c| c.0 == cell)) {
            Some((_, at)) => at,
            None => crowd_at(cell, run[0].at.zoom(), GROUND.read_zoom(run[0].grid)),
        };
        run.iter_mut().for_each(|p| p.crowd = Some((cell, at)));
        out.push(Instance {
            key: Key::Pin(Held::Cell(cell)),
            design: cnt(n),
            stands: at,
            tap: Some(Went::To(at)),
        });
    }
}

/// The one waiting for its words, as `Seen` compares it: its entry and its
/// words by their hash, so a settled turn asks the heap for nothing.
pub(super) fn drafted() -> Option<(EntryAt, u64)> {
    draft().map(|d| {
        let mut h = fnv::FnvHasher::default();
        h.write(d.words.as_bytes());
        (d.at, h.finish())
    })
}

/// Where the count of a pin's cell `(zoom, at)` stands: at its middle, half
/// a pin at `pin` over what is drawn there first within it at `detail`, or
/// over its floor.
fn crowd_at((zoom, at): Cell, pin: Zoom, detail: Zoom) -> Q3 {
    let c = cube(zoom, at);
    let [x, y, _] = c.center();
    let top = surface(&point(x, y, c.min[2]), (c.max[2], c.min[2]), detail).unwrap_or(c.min[2]);
    point(x, y, top + units(pin) / 2.0)
}

/// A pin at its own cell's center, half a cell over the floor it stands
/// on, where the dot under its pill is drawn, and its base.
fn pin_at(at: EntryAt) -> (Q3, V3) {
    let c = cube(at.zoom(), at.at());
    let [x, y, z] = c.center();
    (point(x, y, z), [x, y, c.min[2]])
}
