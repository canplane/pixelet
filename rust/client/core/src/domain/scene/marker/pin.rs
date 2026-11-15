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

use super::{Brick, Instance, Key, MarkerHit, surface};
use crate::domain::layer::object::asset::Asset;
use crate::domain::preview::Draft;
use crate::domain::query::{World, cell_zoom_at};
use crate::domain::scene::marker::design::Design;
use pxl_render::look::marker::{CNT_FILL, CNT_INK, PILL_FILL, PILL_INK};
use pxl_spec::asset::marker::{marker_cell_of, marker_shows};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{cube, point};
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;

/// A pin's cell (`marker_cell_of`): its zoom and place.
type Cell = (Zoom, [i32; 3]);

/// A pin read from a drawn brick: its pill, where it stands, and its base.
/// It also keeps what the last run showed: the camera grid's cell zoom at its
/// base (`grid`), its pin's cell or None where it did not show (`cell`), and
/// where its cell's count stood, with the cell that was then (`crowd`).
#[derive(Clone)]
pub(super) struct Pin {
    pub(super) brick: Brick,
    at: Octant,
    design: Design,
    pos: Q3,
    base: V3,
    grid: Zoom,
    cell: Option<Cell>,
    crowd: Option<(Cell, Q3)>,
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
                grid: 0,
                cell: None,
                crowd: None,
            });
        }
    });
}

/// Writes into `out` what `pins` show, seen from `eye`. The draft shows
/// first, wherever it is. Then each pin shows where the cell at its own point
/// shows a marker (`marker_shows`). The pins of one pin's cell, the draft
/// left out, show together: a single pin shows as itself, and more show as
/// their count, which a tap takes to where the count stands. A pin's cell is
/// finer than its brick and lies inside it, so a brick's pins are counted
/// among themselves alone.
pub(super) fn show(pins: &mut [Pin], eye: V3, draft: Option<&Draft>, out: &mut Vec<Instance>) {
    let drafted = draft.map(|d| d.at);
    out.extend(draft.map(|d| one(d.at, pill(&d.text), pin_at(d.at).0)));
    for p in pins.iter_mut() {
        p.grid = cell_zoom_at(eye, p.base);
        p.cell = marker_shows(p.grid).then(|| marker_cell_of(p.grid, p.at));
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
        // Where the count stands is asked once for each cell it stands for,
        // and kept with the cell's pins.
        let at = match run.iter().find_map(|p| p.crowd.filter(|c| c.0 == cell)) {
            Some((_, at)) => at,
            None => crowd_at(cell, run[0].at.zoom(), GROUND.read_zoom(run[0].grid)),
        };
        run.iter_mut().for_each(|p| p.crowd = Some((cell, at)));
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
/// middle, half a pin of zoom `pin` above the first thing a ray down meets
/// within the cell at `detail`, or above the cell's floor where it meets
/// nothing.
fn crowd_at((zoom, at): Cell, pin: Zoom, detail: Zoom) -> Q3 {
    let c = cube(zoom, at);
    let [x, y, _] = c.center();
    let top = surface(&point(x, y, c.min[2]), (c.max[2], c.min[2]), detail).unwrap_or(c.min[2]);
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
