// pxl-core/domain/query/change.rs

//! Where the ground's answers moved since the frame turn before: the ground
//! tiles whose answer moved, by identity, which fixes their contents, and the
//! boxes where objects were written. A reader that keeps an answer asks again
//! only when one of these touches the columns it read, which its queries
//! report (`Footprint`). A change so reaches only the readers of the ground
//! it touches.

use super::world::World;
use crate::domain::layer::object::store;
use crate::domain::layer::Base as _;
use core::cell::Cell;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::{nearest, nearest_box, BOUNDS};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;

#[derive(Default)]
struct ChangeLog {
    ground: Vec<Quadrant>,
    written: Vec<Aabb>,
}

static CHANGES: Global<ChangeLog> = Global::new();

fn changes() -> &'static mut ChangeLog {
    CHANGES.get(ChangeLog::default)
}

/// Takes the frame turn's changes from the ground's tile cache and the object
/// store, replacing the last turn's. Each ground tile appears once, however
/// many of its bricks or entries moved. True when anything moved.
pub(crate) fn take() -> bool {
    let c = changes();
    TerrainTile::tile_cache().take_changed(&mut c.ground);
    c.ground.sort_unstable();
    c.ground.dedup();
    store::take_written(&mut c.written);
    !c.ground.is_empty() || !c.written.is_empty()
}

/// The boxes where objects were written, as the last take found them.
pub fn written_boxes() -> &'static [Aabb] {
    &changes().written
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
    let columns = Aabb {
        min: [xy[0][0], xy[0][1], BOUNDS.min[2]],
        max: [xy[1][0], xy[1][1], BOUNDS.max[2]],
    };
    let about = (columns.min[0] + columns.max[0]) / 2.0;
    let c = changes();
    let mut regions = c
        .ground
        .iter()
        .chain(layer)
        .map(|t| t.region())
        .chain(c.written.iter().copied());
    regions.any(|r| nearest_box(&r, about).shares(columns))
}

/// The columns a reader's queries read, as one box with its corners in units
/// and X on the lap of the first column read. A ray reads the cells it
/// passes at the coarsest zoom it is answered at; a surface or a column
/// surface reads its cell (`World::reading`).
#[derive(Default)]
pub struct Footprint(Cell<Option<[[f64; 2]; 2]>>);

impl Footprint {
    /// Adds the cells of `zoom` that the box from `a` to `b` lies in.
    pub(super) fn read(&self, a: [f64; 2], b: [f64; 2], zoom: Zoom) {
        let side = units(zoom);
        let [lo, hi] = [
            [0, 1].map(|k| a[k].min(b[k])),
            [0, 1].map(|k| a[k].max(b[k])),
        ];
        self.join([
            lo.map(|v| libm::floor(v / side) * side),
            hi.map(|v| (libm::floor(v / side) + 1.0) * side),
        ]);
    }

    /// Adds the columns `cells`, moved onto the lap of the first column read.
    pub(super) fn join(&self, cells: [[f64; 2]; 2]) {
        let joined = match self.0.get() {
            None => cells,
            Some(c) => {
                let dx = nearest(cells[0][0], c[0][0]) - cells[0][0];
                [
                    [c[0][0].min(cells[0][0] + dx), c[0][1].min(cells[0][1])],
                    [c[1][0].max(cells[1][0] + dx), c[1][1].max(cells[1][1])],
                ]
            }
        };
        self.0.set(Some(joined));
    }

    /// The box of the columns read. None when nothing was read.
    pub fn get(&self) -> Option<[[f64; 2]; 2]> {
        self.0.get()
    }
}

/// An answer a step got from the world, kept with the key it was asked for
/// and the columns it read (`World::reading`), their corners in units. It is
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

    /// The answer for `of`: the kept one, or else what `ask` gets from
    /// `world`, whose queries report what it read. What it read is reported
    /// to `world` as well, so a memo kept inside another memo's ask lets that
    /// one go with it.
    pub fn of(&mut self, world: World<'_>, of: K, ask: impl FnOnce(World<'_>) -> A) -> A {
        if let Some((k, columns, a)) = &self.0 {
            if *k == of {
                world.read_columns(*columns);
                return *a;
            }
        }
        let read = Footprint::default();
        let a = ask(world.reading(&read));
        self.0 = read.get().map(|columns| {
            world.read_columns(columns);
            (of, columns, a)
        });
        a
    }
}
