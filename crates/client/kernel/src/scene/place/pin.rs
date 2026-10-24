// pxl-kernel/scene/place/pin.rs

//! The markers someone put up, as the object store holds them: each its
//! words on a pill at its own cell, or where more than one stands in a
//! marker's cell (`marker_cell`), their count at the cell's middle on a
//! marker's pill in its colors turned over, the markers' own rule
//! (`block/marker.json`); the one waiting for its words wherever it is.

use core::hash::Hasher;

use super::{surface, Instance, Key, Went};
use crate::billboard::badge::Label;
use crate::chunk::Chunk;
use crate::layer::object::preview::draft;
use crate::query::World;
use pxl_render::look::badge::{COUNT_FILL, COUNT_INK, PILL_FILL, PILL_INK};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::block::object::{marker_cell_of, marker_shows, object_value_is_marker};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

/// What holds a marker's instance: its entry, or the marker's cell its
/// count stands for.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Held {
    One(EntryAt),
    Cell((Zoom, [i32; 3])),
}

/// A marker read from a drawn chunk, in a run a marker's cell: its pill,
/// where it stands, and where the cell holds more than one, where their
/// count stands and the count.
#[derive(Clone)]
pub(super) struct Pin {
    pub(super) chunk: Chunk,
    at: EntryAt,
    label: Label,
    stands: Q3,
    cell: (Zoom, [i32; 3]),
    count: Option<(Q3, Label)>,
}

/// A product choice: the length a one-line badge still reads as a name at.
/// Code points, so a non-BMP character is never cut in half.
const SHOWN_CHARS: usize = 12;

/// A marker's words, cut to `SHOWN_CHARS`, on its pill.
fn pill(words: &str) -> Label {
    let mut shown: String = words.chars().take(SHOWN_CHARS).collect();
    if words.chars().count() > SHOWN_CHARS {
        shown.push('…');
    }
    Label::Pill {
        words: shown.into(),
        fill: PILL_FILL,
        ink: PILL_INK,
    }
}

/// The count of `n` markers in one marker's cell, on a marker's pill in the
/// count's colors.
fn count(n: usize) -> Label {
    Label::Pill {
        words: n.to_string().into(),
        fill: COUNT_FILL,
        ink: COUNT_INK,
    }
}

/// One marker, taken to its entry, which the tool held decides what to do
/// with.
fn one(at: EntryAt, label: Label, stands: Q3) -> Instance {
    Instance {
        key: Key::Pin(Held::One(at)),
        label,
        stands,
        tap: Some(Went::Marker(at, stands)),
    }
}

/// The markers put up in `chunk`, by the chunk their base is in, a run a
/// marker's cell: a cell nests in its chunk, so no run is another chunk's too.
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
        let (stands, at) = marker_at(e.id());
        if (0..3).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) {
            pins.push(Pin {
                chunk,
                at: e.id(),
                label: pill(world.words(e).unwrap_or_default()),
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
/// cell's markers but that one: one is itself, more their count, which a
/// tap takes to where it stands.
pub(super) fn shown(pins: &[Pin]) -> impl Iterator<Item = Instance> + '_ {
    let drafted = draft().map(|d| d.at);
    let waiting = draft().map(|d| one(d.at, pill(&d.words), marker_at(d.at).0));
    waiting.into_iter().chain(pins.chunk_by(|a, b| a.cell == b.cell).filter_map(move |run| {
        let mut left = run.iter().filter(|p| Some(p.at) != drafted);
        let first = left.next()?;
        let n = 1 + left.count();
        Some(match &first.count {
            Some((at, counted)) if n > 1 => Instance {
                key: Key::Pin(Held::Cell(first.cell)),
                label: if n == run.len() { counted.clone() } else { count(n) },
                stands: *at,
                tap: Some(Went::To(*at)),
            },
            _ => one(first.at, first.label.clone(), first.stands),
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

/// Where the count of a cell's markers stands: at its middle, half a marker
/// at `marker` over what is drawn there first within it, or over its floor.
fn crowd_at(zoom: Zoom, at: [i32; 3], marker: Zoom, detail: Zoom) -> Q3 {
    let c = cube(zoom, at);
    let [x, y, _] = c.center();
    let top = surface(&point(x, y, c.min[2]), (c.max[2], c.min[2]), detail).unwrap_or(c.min[2]);
    point(x, y, top + units(marker) / 2.0)
}

/// A marker at its own cell's center, half a cell over the floor it stands
/// on, where the dot under its pill is drawn, and its base.
fn marker_at(at: EntryAt) -> (Q3, V3) {
    let c = cube(at.zoom(), at.at());
    let [x, y, z] = c.center();
    (point(x, y, z), [x, y, c.min[2]])
}
