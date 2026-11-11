// pxl-core/domain/query/change.rs

//! Where the ground's answers moved since the frame turn before: the ground
//! tiles and clipmap bricks whose answer moved, by identity, which fixes their
//! contents, and the boxes where objects were written. A reader that keeps an answer asks again
//! only when one of these touches the columns it read, which its queries
//! report (`ReadRect`). A change so reaches only the readers of the ground
//! it touches.

use crate::domain::layer::object::store;
use crate::domain::layer::Base as _;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::brick;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::{nearest, nearest_box, BOUNDS};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;

#[derive(Default)]
struct ChangeLog {
    ground: Vec<Quadrant>,
    damage: Vec<Aabb>,
}

static CHANGES: Global<ChangeLog> = Global::new();

fn changes() -> &'static mut ChangeLog {
    CHANGES.get(ChangeLog::default)
}

/// Takes the frame turn's changes from the ground's tile cache and the object
/// store, replacing the last turn's. Each ground node appears once, however
/// many times it moved. True when anything moved.
pub(crate) fn take() -> bool {
    let c = changes();
    TerrainTile::tile_cache().take_changed(&mut c.ground);
    c.ground.sort_unstable();
    c.ground.dedup();
    store::take_damage(&mut c.damage);
    !c.ground.is_empty() || !c.damage.is_empty()
}

/// The damage: the boxes where objects were written, as the last take found
/// them.
pub fn damage() -> &'static [Aabb] {
    &changes().damage
}

/// Whether a change touches the columns over `xy`, whose corners are in units
/// with X on any lap. A change is a moved ground tile, a written object, or
/// one of `layer`'s tiles that the reader also reads and whose answer moved.
///
/// The changes are taken once a frame turn, in its visible phase after its
/// arrivals are admitted, and held until the next take. What moves between
/// frame turns waits in the tile cache and the store for that take. So a
/// reader that asks every frame turn, in its effects or its interaction
/// phase, sees each change exactly once.
pub fn within(xy: [[f64; 2]; 2], layer: &[Quadrant]) -> bool {
    touched(xy, layer, |_| true)
}

/// Whether the ground node `t` is at the zoom of the leaf cells' tile or of
/// their clip level's brick. A coarser level's brick that shares the tile's
/// zoom counts too.
fn at_leaf(t: &Quadrant) -> bool {
    let cell_zoom = GROUND.leaf_cell_zoom();
    t.zoom == cell_zoom - GROUND.tile_cell_shamt() || t.zoom == brick::bricks_zoom(cell_zoom)
}

/// `within`, of the ground nodes `counted` picks.
fn touched(xy: [[f64; 2]; 2], layer: &[Quadrant], counted: impl Fn(&Quadrant) -> bool) -> bool {
    let columns = Aabb {
        min: [xy[0][0], xy[0][1], BOUNDS.min[2]],
        max: [xy[1][0], xy[1][1], BOUNDS.max[2]],
    };
    let about = (columns.min[0] + columns.max[0]) / 2.0;
    let c = changes();
    let mut regions = c
        .ground
        .iter()
        .filter(|t| counted(t))
        .chain(layer)
        .map(|t| t.region())
        .chain(c.damage.iter().copied());
    regions.any(|r| nearest_box(&r, about).shares(columns))
}

/// The columns the queries read, as one box with its corners in units and X
/// on the lap of the first column read. A ray reads the cells it passes at
/// the coarsest zoom it is answered at; a cell or a column surface reads its
/// cell. Those queries report here, and a memo's ask takes what it
/// read from here (`Memo::of`).
#[derive(Clone, Copy, Default)]
struct ReadRect(Option<[[f64; 2]; 2]>);

static READ_RECT: Global<ReadRect> = Global::new();

fn read_rect() -> &'static mut ReadRect {
    READ_RECT.get(ReadRect::default)
}

/// Reports the cells of `zoom` that the box from `a` to `b` lies in as read.
pub(super) fn report_read(a: [f64; 2], b: [f64; 2], zoom: Zoom) {
    let side = units(zoom);
    let [lo, hi] = [
        [0, 1].map(|k| a[k].min(b[k])),
        [0, 1].map(|k| a[k].max(b[k])),
    ];
    join([
        lo.map(|v| libm::floor(v / side) * side),
        hi.map(|v| (libm::floor(v / side) + 1.0) * side),
    ]);
}

/// Reports the columns `cells` as read, moved onto the lap of the first
/// column read.
fn join(cells: [[f64; 2]; 2]) {
    let f = read_rect();
    f.0 = Some(match f.0 {
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

/// An answer a step got from the world, kept with the key it was asked for
/// and the columns it read (`ReadRect`), their corners in units. It is
/// asked again only when the key changes, or when a change to the ground or
/// the objects touched those columns (`forget_touched`). An answer that read
/// nothing is not kept, since no change would ever tell it to go.
pub struct Memo<K, A>(Option<(K, [[f64; 2]; 2], A)>);

impl<K, A> Default for Memo<K, A> {
    fn default() -> Self {
        Memo(None)
    }
}

impl<K: PartialEq, A: Copy> Memo<K, A> {
    pub const NONE: Memo<K, A> = Memo(None);

    /// Drops the answer when a change touched its columns. The caller calls
    /// this every frame turn, so that it sees each change once (`within`).
    pub fn forget_touched(&mut self) {
        if self
            .0
            .as_ref()
            .is_some_and(|&(_, columns, _)| within(columns, &[]))
        {
            self.0 = None;
        }
    }

    /// `forget_touched`, for an answer of the ground at its leaf. `exact` says
    /// of the kept answer whether every read of it was exact. A base tile
    /// never changes, so a coarser tile or brick over the columns of an exact
    /// answer changes only what is held, and only the leaf's own nodes and
    /// what objects wrote drop it. An answer not exact goes as in
    /// `forget_touched`, since a coarser node can be what answered it.
    pub fn forget_touched_at_leaf(&mut self, exact: impl Fn(&A) -> bool) {
        if self
            .0
            .as_ref()
            .is_some_and(|(_, columns, a)| match exact(a) {
                true => touched(*columns, &[], at_leaf),
                false => within(*columns, &[]),
            })
        {
            self.0 = None;
        }
    }

    /// The answer for `of`: the kept one, or else what `ask` gets, with the
    /// columns its queries read. What it read is reported
    /// as read here too, so a memo kept inside another memo's ask lets that
    /// one go with it.
    pub fn of(&mut self, of: K, ask: impl FnOnce() -> A) -> A {
        if let Some((k, columns, a)) = &self.0 {
            if *k == of {
                join(*columns);
                return *a;
            }
        }
        let outer = core::mem::take(read_rect());
        let a = ask();
        let read = core::mem::replace(read_rect(), outer);
        self.0 = read.0.map(|columns| {
            join(columns);
            (of, columns, a)
        });
        a
    }
}

#[cfg(test)]
#[path = "change.test.rs"]
mod test;
