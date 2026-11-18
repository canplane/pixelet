// pxl-core/domain/scene/marker/pin.rs

//! The pins people put up, objects of the marker kind as the object store
//! holds them, drawn as scene markers. Each pin shows its text on a pill at
//! its own cell. Where more than one pin stands in a pin's cell
//! (`marker_cell_zoom`), their count shows instead, at the cell's middle, on
//! a pill with the pin's colors turned over. That is the pins' own rule
//! (`asset/marker.json5`). The draft shows wherever it is. A pin is read with
//! the brick it stands in, and it is shown at the camera grid's cell at its
//! own point, so whether it shows and how it is counted follow the zoom
//! drawn there.

use core::hash::Hasher;

use super::{Brick, Instance, Key, MarkerHit};
use crate::domain::layer::object::asset::Asset;
use crate::domain::preview::Draft;
use crate::domain::query::change::Memo;
use crate::domain::query::{World, cell_zoom_drawn};
use crate::domain::scene::marker::design::Design;
use pxl_render::look::marker::{CNT_FILL, CNT_INK, PILL_FILL, PILL_INK};
use pxl_spec::asset::marker::{marker_cell_of, marker_shows};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{cube, point};
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::layer::octant::Octant;

/// A pin's cell (`marker_cell_of`): its zoom and place.
type Cell = (Zoom, [i32; 3]);

/// A pin read from a drawn brick: its pill, where it stands, and its base.
/// It also keeps what the last run showed: its pin's cell or None where it
/// did not show (`cell`), and where its cell's count stands, kept by the
/// cell (`crowd`).
#[derive(Clone)]
pub(super) struct Pin {
    pub(super) brick: Brick,
    at: Octant,
    design: Design,
    pos: Q3,
    base: V3,
    cell: Option<Cell>,
    crowd: Memo<Cell, Q3>,
}

/// The longest a one-line pin's text runs and still reads as a name, a
/// product choice. It counts code points, so a character outside the BMP is
/// never cut in half.
const SHOWN_CHARS: usize = 12;

/// A pin's text, cut to `SHOWN_CHARS`, on its pill.
fn pill(text: &str) -> Design {
    let mut shown: String = text.chars().take(SHOWN_CHARS).collect();
    if text.chars().count() > SHOWN_CHARS {
        shown.push('…');
    }
    Design::Pill {
        text: shown.into(),
        fill: PILL_FILL,
        ink: PILL_INK,
    }
}

/// The count of `n` pins in one pin's cell, on a pin's pill in the
/// count's colors.
fn cnt(n: usize) -> Design {
    Design::Pill {
        text: n.to_string().into(),
        fill: CNT_FILL,
        ink: CNT_INK,
    }
}

/// One pin's instance. A tap on it goes to the pin's entry, and the tool held
/// decides what to do with it.
fn one(at: Octant, design: Design, pos: Q3) -> Instance {
    Instance {
        key: Key::Pin(at),
        design,
        pos,
        tap: Some(MarkerHit::Pin(at, pos)),
    }
}

impl Pin {
    /// The brick of `zoom` its base stands in.
    pub(super) fn brick_at(&self, zoom: Zoom) -> Brick {
        let place = [0, 1].map(|k| libm::floor(self.base[k] / units(zoom)) as i32);
        (zoom, place)
    }
}

/// Reads the pins put up over `brick`, whose columns are `bounds`. A pin
/// belongs to the brick its base is in, so no pin is read by two bricks.
pub(super) fn read(brick: Brick, bounds: &Aabb, pins: &mut Vec<Pin>) {
    let world = World::now();
    world.placed_in(bounds, &mut |e| {
        if e.asset() != Asset::Marker {
            return;
        }
        let (pos, base) = pin_at(e.id());
        if (0..3).all(|k| (bounds.min[k]..bounds.max[k]).contains(&base[k])) {
            pins.push(Pin {
                brick,
                at: e.id(),
                design: pill(world.text(e).unwrap_or_default()),
                pos,
                base,
                cell: None,
                crowd: Memo::NONE,
            });
        }
    });
}

/// Writes into `out` what `pins` show. The draft shows first, wherever it
/// is. Then each pin shows where the cell drawn at its own point shows a
/// marker (`marker_shows`). The pins of one pin's cell, the draft left out,
/// show together: a single pin shows as itself, and more show as their
/// count, which a tap takes to where the count stands. A pin's cell is finer
/// than its brick and lies inside it, so a brick's pins are counted among
/// themselves alone.
pub(super) fn show(pins: &mut [Pin], draft: Option<&Draft>, out: &mut Vec<Instance>) {
    let drafted = draft.map(|d| d.at);
    out.extend(draft.map(|d| one(d.at, pill(&d.text), pin_at(d.at).0)));
    for p in pins.iter_mut() {
        p.cell = cell_zoom_drawn(p.base)
            .filter(|&grid| marker_shows(grid))
            .map(|grid| marker_cell_of(grid, p.at));
    }
    pins.sort_unstable_by_key(|p| (p.brick, p.cell));
    for run in pins.chunk_by_mut(|a, b| a.brick == b.brick && a.cell == b.cell) {
        let Some(cell) = run[0].cell else { continue };
        let mut left = run.iter().filter(|p| Some(p.at) != drafted);
        let Some(first) = left.next() else { continue };
        let n = 1 + left.count();
        if n == 1 {
            out.push(one(first.at, first.design.clone(), first.pos));
            continue;
        }
        // Where the count stands is kept by its cell and the size the eye
        // sets there, with each of the cell's pins. It is asked here and not
        // under the entry's budget, as a place's surface is: a count is named
        // by its cell, so nothing shown before could stand for it meanwhile.
        let mut crowd = core::mem::take(&mut run[0].crowd);
        let at = crowd.of(cell, || crowd_at(cell, run[0].at.zoom()));
        run.iter_mut().for_each(|p| p.crowd = crowd.clone());
        out.push(Instance {
            key: Key::PinCell(cell),
            design: cnt(n),
            pos: at,
            tap: Some(MarkerHit::To(at)),
        });
    }
}

/// `draft` as arrangement.rs `Input` compares it: its entry, and its text by
/// the text's hash, so a frame where nothing changed allocates nothing.
pub(super) fn draft_key(draft: Option<&Draft>) -> Option<(Octant, u64)> {
    draft.map(|d| {
        let mut h = fnv::FnvHasher::default();
        h.write(d.text.as_bytes());
        (d.at, h.finish())
    })
}

/// Where the count of the pin's cell `(zoom, at)` stands: at the cell's
/// middle, half a pin of zoom `pin` above the floor under the cell's top
/// where that lies within the cell, or above the cell's floor.
fn crowd_at((zoom, at): Cell, pin: Zoom) -> Q3 {
    let c = cube(zoom, at);
    let [x, y, _] = c.center();
    let top = World::now()
        .floor_under(&point(x, y, c.max[2]))
        .val()
        .flatten()
        .filter(|top| (c.min[2]..=c.max[2]).contains(top))
        .unwrap_or(c.min[2]);
    point(x, y, top + units(pin) / 2.0)
}

/// Where a pin stands, and its base. It stands at its own cell's center, half
/// a cell above the floor it stands on, which is where the dot under its pill
/// is drawn.
fn pin_at(at: Octant) -> (Q3, V3) {
    let c = cube(at.zoom(), at.at());
    let [x, y, z] = c.center();
    (point(x, y, z), [x, y, c.min[2]])
}
