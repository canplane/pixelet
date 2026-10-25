// pxl-kernel/domain/scene/marker/pin.rs

//! The pins someone put up, as the object store holds them: each its words
//! on a pill at its own cell, or where more than one stands in a pin's cell
//! (`marker_cell`), their count at the cell's middle on a pin's pill in its
//! colors turned over, the pins' own rule (`block/marker.json`); the one
//! waiting for its words wherever it is.

use core::hash::Hasher;

use super::arrangement::{surface, Instance, Key, Went};
use crate::domain::chunk::Chunk;
use crate::domain::layer::object::preview::draft;
use crate::domain::query::World;
use crate::domain::scene::marker::design::Design;
use pxl_render::look::marker::{COUNT_FILL, COUNT_INK, PILL_FILL, PILL_INK};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::block::object::{marker_cell_of, marker_shows, object_value_is_marker};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

/// What holds a pin's instance: its entry, or the pin's cell its
/// count stands for.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Held {
    One(EntryAt),
    Cell((Zoom, [i32; 3])),
}

/// A pin read from a drawn chunk, in a run a pin's cell: its pill,
/// where it stands, and where the cell holds more than one, where their
/// count stands and the count.
#[derive(Clone)]
pub(super) struct Pin {
    pub(super) chunk: Chunk,
    at: EntryAt,
    design: Design,
    stands: Q3,
    cell: (Zoom, [i32; 3]),
    count: Option<(Q3, Design)>,
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
fn count(n: usize) -> Design {
    Design::Pill {
        words: n.to_string().into(),
        fill: COUNT_FILL,
        ink: COUNT_INK,
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

/// The pins put up in `chunk`, by the chunk their base is in, a run a
/// pin's cell: a cell nests in its chunk, so no run is another chunk's too.
pub(super) fn read(chunk: Chunk, pins: &mut Vec<Pin>) {
    let cell_zoom = chunk.cell_zoom();
    if !marker_shows(cell_zoom) {
        return;
    }
    let region = chunk.region();
    let world = World::now();
    let from = pins.len();
    world.placed_in(&region, &mut |e| {
        if !object_value_is_marker(e.state) {
            return;
        }
        let (stands, at) = pin_at(e.id());
        if (0..3).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) {
            pins.push(Pin {
                chunk,
                at: e.id(),
                design: pill(world.words(e).unwrap_or_default()),
                stands,
                cell: marker_cell_of(cell_zoom, e.id()),
                count: None,
            });
        }
    });
    let detail = GROUND.read_zoom(cell_zoom);
    let mine = &mut pins[from..];
    mine.sort_unstable_by_key(|p| p.cell);
    for run in mine.chunk_by_mut(|a, b| a.cell == b.cell).filter(|run| run.len() > 1) {
        let ((z, at), one) = (run[0].cell, run[0].at);
        let counted = Some((crowd_at(z, at, one.zoom(), detail), count(run.len())));
        run.iter_mut().for_each(|p| p.count.clone_from(&counted));
    }
}

/// What `pins` show: the one waiting for its words, wherever it is, then a
/// cell's pins but that one: one is itself, more their count, which a
/// tap takes to where it stands.
pub(super) fn shown(pins: &[Pin]) -> impl Iterator<Item = Instance> + '_ {
    let drafted = draft().map(|d| d.at);
    let waiting = draft().map(|d| one(d.at, pill(&d.words), pin_at(d.at).0));
    waiting.into_iter().chain(pins.chunk_by(|a, b| a.cell == b.cell).filter_map(move |run| {
        let mut left = run.iter().filter(|p| Some(p.at) != drafted);
        let first = left.next()?;
        let n = 1 + left.count();
        Some(match &first.count {
            Some((at, counted)) if n > 1 => Instance {
                key: Key::Pin(Held::Cell(first.cell)),
                design: if n == run.len() { counted.clone() } else { count(n) },
                stands: *at,
                tap: Some(Went::To(*at)),
            },
            _ => one(first.at, first.design.clone(), first.stands),
        })
    }))
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

/// Where the count of a cell's pins stands: at its middle, half a pin at
/// `pin` over what is drawn there first within it, or over its floor.
fn crowd_at(zoom: Zoom, at: [i32; 3], pin: Zoom, detail: Zoom) -> Q3 {
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
