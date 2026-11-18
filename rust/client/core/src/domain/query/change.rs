// pxl-core/domain/query/change.rs

//! Where the ground's answers moved since the frame before: the ground
//! tiles and clipmap bricks whose answer moved, by identity, which fixes
//! their contents, and the boxes where objects were written. An answer is of
//! a cell at a size, and the world's alone, so a reader that keeps one
//! (`Memo`) asks again only when the world changes in what it read or the
//! eye sets another size for a cell it read. Its queries report what they
//! read (`Read`): the rect, the cell zooms and whether each read was exact.
//! A change reaches it where it touches that rect at a zoom it read
//! (`Read::reaches`); the eye's part is the size (`Read::holds`), as a voxel
//! found once is drawn until the eye's distance asks another zoom there.

use super::detail::grid_zoom;
use crate::domain::eye;
use crate::domain::layer::Base as _;
use crate::domain::layer::object::store;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use pxl_engine::clipmap;
use pxl_global::Global;
use pxl_spec::brick;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{BOUND, nearest, nearest_box};
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::Quadrant;

#[derive(Default)]
struct ChangeLog {
    ground: Vec<Quadrant>,
    damage: Vec<Aabb>,
}

static CHANGES: Global<ChangeLog> = Global::new();

fn changes() -> &'static mut ChangeLog {
    CHANGES.get(ChangeLog::default)
}

/// Takes the frame's changes from the ground's tile cache and the object
/// store, replacing the last frame's. Each ground node appears once, however
/// many times it moved. True when anything moved.
pub(crate) fn take() -> bool {
    let c = changes();
    IndexedTerrainTile::tile_cache().take_changed(&mut c.ground);
    c.ground.sort_unstable();
    c.ground.dedup();
    store::take_damage(&mut c.damage);
    !c.ground.is_empty() || !c.damage.is_empty()
}

/// The damage: the boxes where objects were written, as the last take found
/// them.
pub(crate) fn damage() -> &'static [Aabb] {
    &changes().damage
}

/// Whether a change touches the rect over `xy`, whose corners are in units
/// with X on any lap. A change is a moved ground tile, a written object, or
/// one of `layer`'s tiles that the reader also reads and whose answer moved.
///
/// The changes are taken once a frame, in its visible phase after its
/// arrivals are admitted, and held until the next take. What moves between
/// frames waits in the tile cache and the store for that take. So a
/// reader that asks every frame, in its effects or its interaction
/// phase, sees each change exactly once.
pub(crate) fn within(xy: [[f64; 2]; 2], layer: &[Quadrant]) -> bool {
    touched(xy, layer, |_| true)
}

/// `within`, of the ground nodes `counted` picks.
fn touched(xy: [[f64; 2]; 2], layer: &[Quadrant], counted: impl Fn(&Quadrant) -> bool) -> bool {
    let rect = Aabb {
        min: [xy[0][0], xy[0][1], BOUND.min[2]],
        max: [xy[1][0], xy[1][1], BOUND.max[2]],
    };
    let about = (rect.min[0] + rect.max[0]) / 2.0;
    let c = changes();
    let mut regions = c
        .ground
        .iter()
        .filter(|t| counted(t))
        .chain(layer)
        .map(|t| t.region())
        .chain(c.damage.iter().copied());
    regions.any(|r| nearest_box(&r, about).shares(rect))
}

/// A size the eye set for a read: the camera grid's zoom at a point's
/// distance from it (query/detail.rs `grid_zoom`), no finer than `finest`, the
/// finest the world holds where it was asked, with the point.
#[derive(Clone, Copy, PartialEq)]
struct Size {
    xy: [f64; 2],
    z: Option<f64>,
    finest: Zoom,
    zoom: Zoom,
}

/// What a read took of the eye. A place's read takes the sizes the eye set
/// for it, and is the world's answer at those sizes wherever the eye
/// stands. A ray across what the eye sees asks each node at its own
/// distance from the eye, so its answer is of the eye where it stood, none
/// where there was none.
#[derive(Clone, PartialEq)]
enum Taken {
    Sizes(Vec<Size>),
    Eye(Option<V3>),
}

/// What the queries read for a memo's ask (`Memo::of`): the rect read, as
/// one box with its corners in units and X on the lap of the first cell
/// read, where a ray reads the cells it passes at the coarsest zoom it is
/// answered at and a cell or a surface its cell; the cell zooms read, from
/// the coarsest to the finest, and whether every read was exact, answered by
/// the ground held at the zoom asked with nothing standing in for it; and
/// what they took of the eye.
#[derive(Clone)]
struct Read {
    rect: Option<[[f64; 2]; 2]>,
    cell_zoom_range: Option<[Zoom; 2]>,
    exact: bool,
    taken: Taken,
}

impl Default for Read {
    fn default() -> Self {
        Read {
            rect: None,
            cell_zoom_range: None,
            exact: true,
            taken: Taken::Sizes(Vec::new()),
        }
    }
}

impl Read {
    /// Whether a change of the ground node `t` may change what was read. A
    /// read at a cell zoom reads the ground's cells of that zoom, off the
    /// node of that zoom or, where it is not held, off a coarser one
    /// standing in; it reads nothing finer. So a node whose cells are finer
    /// than every zoom read changes nothing, and neither does one whose
    /// cells are coarser than every zoom read where every read was exact: a
    /// node's own cells never change, so a coarser one over an exact read
    /// changes only what is held. A node's cells are its tile's or its
    /// brick's, and it counts as either.
    fn reaches(&self, t: &Quadrant) -> bool {
        let Some([coarsest, finest]) = self.cell_zoom_range else {
            return true;
        };
        let cells = [GROUND.tile_cell_shamt(), brick::BRICK_SHIFT].map(|shift| t.zoom + shift);
        let finer = cells[0].min(cells[1]) > clipmap::clip_zoom(finest);
        let coarser = cells[0].max(cells[1]) < clipmap::clip_zoom(coarsest);
        !(finer || self.exact && coarser)
    }

    /// Whether the eye `now` is given what this read was given, the world
    /// unchanged: it sets each size again, or, for a read of the eye where it
    /// stood, stands there.
    fn holds(&self, now: Option<V3>) -> bool {
        match (&self.taken, now) {
            (Taken::Eye(eye), _) => *eye == now,
            (Taken::Sizes(sizes), Some(eye)) => sizes
                .iter()
                .all(|s| grid_zoom(eye, s.xy, s.z).min(s.finest) == s.zoom),
            (Taken::Sizes(sizes), None) => sizes.is_empty(),
        }
    }
}

/// The read of the memo asking now, none outside a memo's ask: what is
/// reported then is kept by no one.
static READ: Global<Option<Read>> = Global::new();

fn read() -> &'static mut Option<Read> {
    READ.get(|| None)
}

/// What `ask` answers, with what its queries read.
fn reads<A>(ask: impl FnOnce() -> A) -> (A, Read) {
    let outer = read().replace(Read::default());
    let a = ask();
    (a, core::mem::replace(read(), outer).unwrap_or_default())
}

/// Reports a read of the cells that the box from `a` to `b` lies in, at the
/// cell zooms from `coarsest` to `finest`, the rect being the coarsest's
/// cells, and whether it was `exact`: answered by the ground held at each
/// zoom asked, nothing coarser standing in and nothing lacked.
pub(super) fn report_read(a: [f64; 2], b: [f64; 2], [coarsest, finest]: [Zoom; 2], exact: bool) {
    let side = units(coarsest);
    let [lo, hi] = [
        [0, 1].map(|k| a[k].min(b[k])),
        [0, 1].map(|k| a[k].max(b[k])),
    ];
    join([
        lo.map(|v| libm::floor(v / side) * side),
        hi.map(|v| (libm::floor(v / side) + 1.0) * side),
    ]);
    join_zooms([coarsest, finest], exact);
}

/// Joins to what was read the cell zooms a read was made at and whether it
/// was exact.
fn join_zooms([coarsest, finest]: [Zoom; 2], exact: bool) {
    let Some(r) = read() else { return };
    r.cell_zoom_range = Some(r.cell_zoom_range.map_or([coarsest, finest], |[c, f]| {
        [c.min(coarsest), f.max(finest)]
    }));
    r.exact &= exact;
}

/// Reports the rect `cells` as read, moved onto the lap of the first
/// cell read.
fn join(cells: [[f64; 2]; 2]) {
    let Some(r) = read() else { return };
    r.rect = Some(match r.rect {
        None => cells,
        Some(c) => {
            let dx = nearest(cells[0][0], c[0][0]) - cells[0][0];
            [
                [c[0][0].min(cells[0][0] + dx), c[0][1].min(cells[0][1])],
                [c[1][0].max(cells[1][0] + dx), c[1][1].max(cells[1][1])],
            ]
        }
    });
}

/// Where the eye stands, for a ray across what the eye sees: the read is
/// then of the eye there (`Taken::Eye`).
pub(super) fn eye_at() -> Option<V3> {
    let at = eye::at();
    if let Some(r) = read() {
        r.taken = Taken::Eye(at);
    }
    at
}

/// The zoom the eye sets over `xy` at the height `z`, or with none at the
/// eye's own height (query/detail.rs `grid_zoom`). None with no eye: nothing
/// is observed, and the read is of there being none.
pub(super) fn eye_zoom(xy: [f64; 2], z: Option<f64>) -> Option<Zoom> {
    let Some(at) = eye::at() else {
        eye_at();
        return None;
    };
    Some(grid_zoom(at, xy, z))
}

/// The size a read asks over `xy` at the height `z` where the eye sets
/// `eye_zoom` (`eye_zoom`) and the world holds nothing finer than `finest`
/// there: the eye's, no finer than is held. It is what the read took of the
/// eye, kept with the answer.
pub(super) fn size(xy: [f64; 2], z: Option<f64>, eye_zoom: Zoom, finest: Zoom) -> Zoom {
    let zoom = eye_zoom.min(finest);
    if let Some(Read {
        taken: Taken::Sizes(sizes),
        ..
    }) = read()
    {
        sizes.push(Size {
            xy,
            z,
            finest,
            zoom,
        });
    }
    zoom
}

/// Reports to the memo asking now what a memo within its ask read.
fn report(inner: &Read) {
    if let Some(rect) = inner.rect {
        join(rect);
    }
    if let Some(zooms) = inner.cell_zoom_range {
        join_zooms(zooms, inner.exact);
    }
    match (&inner.taken, read()) {
        (Taken::Eye(_), _) => {
            eye_at();
        }
        (
            Taken::Sizes(sizes),
            Some(Read {
                taken: Taken::Sizes(outer),
                ..
            }),
        ) => outer.extend_from_slice(sizes),
        _ => {}
    }
}

/// An answer a step got from the world, kept with the key it was asked for,
/// and what it read (`Read`). It is asked again
/// only when the key changes, when the eye is not given what the read took
/// of it (`Read::holds`), or when the world changed in what it read
/// (`forget_touched`). An answer that read no rect is not kept, since no
/// change would ever tell it to go.
#[derive(Clone)]
pub(crate) struct Memo<K, A>(Option<Kept<K, A>>);

/// A kept answer: its key, what it read and the answer.
type Kept<K, A> = (K, Read, A);

impl<K, A> Default for Memo<K, A> {
    fn default() -> Self {
        Memo(None)
    }
}

impl<K: PartialEq, A: Copy> Memo<K, A> {
    pub(crate) const NONE: Memo<K, A> = Memo(None);

    /// Drops the answer when the world changed in what it read: an object
    /// written over its rect, or a ground node over its rect whose change
    /// may change what was read at the zooms it read (`Read::reaches`). The
    /// caller calls this every frame, so that it sees each change once
    /// (`within`).
    pub(crate) fn forget_touched(&mut self) {
        if self.0.as_ref().is_some_and(|(_, read, _)| {
            read.rect
                .is_some_and(|rect| touched(rect, &[], |t| read.reaches(t)))
        }) {
            self.0 = None;
        }
    }

    /// Whether the answer for `of` is kept as the eye now stands, so `of`
    /// would not ask.
    pub(crate) fn keeps(&self, of: &K) -> bool {
        self.0
            .as_ref()
            .is_some_and(|(k, read, _)| k == of && read.holds(eye::at()))
    }

    /// The answer for `of`: the kept one, or else what `ask` gets, with what
    /// its queries read. What it read is reported as read here too, so a memo
    /// kept inside another memo's ask lets that one go with it.
    pub(crate) fn of(&mut self, of: K, ask: impl FnOnce() -> A) -> A {
        if self.keeps(&of)
            && let Some((_, read, a)) = &self.0
        {
            report(read);
            return *a;
        }
        let (a, read) = reads(ask);
        report(&read);
        self.0 = read.rect.is_some().then_some((of, read, a));
        a
    }
}

#[cfg(test)]
#[path = "change.test.rs"]
mod test;
