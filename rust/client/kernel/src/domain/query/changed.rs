// pxl-kernel/domain/query/changed.rs

//! Where the ground's answers moved since the frame turn before: the ground
//! tiles whose answer moved (their identities, which fix their contents) and
//! where the objects were written. A reader that stands on what it asked
//! asks again only where one of them touches the columns it read, which its
//! queries report (`Columns`), so a change stops at the ground it touches.

use super::world::World;
use crate::domain::layer::Base as _;
use core::cell::Cell;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::{nearest, nearest_box, REACH};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

#[derive(Default)]
struct Changed {
    ground: Vec<Tile>,
    written: Vec<Aabb>,
}

static CHANGED: Global<Changed> = Global::new();

fn changed() -> &'static mut Changed {
    CHANGED.get(Changed::default)
}

/// The frame turn's changes, taken from the ground's residency and the
/// object store in place of the last turn's; true where any moved.
pub(crate) fn take() -> bool {
    let c = changed();
    TerrainTile::residency().take_moved(&mut c.ground);
    crate::domain::layer::object::store::take_written(&mut c.written);
    !c.ground.is_empty() || !c.written.is_empty()
}

/// Where the objects were written, as the last take found them.
pub fn written() -> &'static [Aabb] {
    &changed().written
}

/// Whether a ground change, or one of `layer`'s tiles a reader also reads
/// whose answer moved, touches the columns over `xy`, its corners in units,
/// X on any lap. Taken once a frame turn, in its visible phase, after its
/// arrivals are admitted, and held until the next; what moves between frame
/// turns waits in the residency and the store for the next take. So a
/// reader asking every frame turn, in its effects or its interaction phase,
/// sees each change exactly once.
pub fn within(xy: [[f64; 2]; 2], layer: &[Tile]) -> bool {
    let columns = Aabb {
        min: [xy[0][0], xy[0][1], REACH.min[2]],
        max: [xy[1][0], xy[1][1], REACH.max[2]],
    };
    let about = (columns.min[0] + columns.max[0]) / 2.0;
    let c = changed();
    let mut regions = c
        .ground
        .iter()
        .chain(layer)
        .map(|t| t.region())
        .chain(c.written.iter().copied());
    regions.any(|r| nearest_box(&r, about).shares(columns))
}

/// The columns a reader's queries read, their corners in units, X on the
/// lap of the first: each ray the cells it passes at the coarsest zoom it
/// is answered at, each surface and footing its cell (`World::reading`).
#[derive(Default)]
pub struct Columns(Cell<Option<[[f64; 2]; 2]>>);

impl Columns {
    /// The cells of `zoom` the box from `a` to `b` lies in, read.
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

    /// The columns `cells` read too, put on the lap of the first.
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

    /// None where nothing was read.
    pub fn get(&self) -> Option<[[f64; 2]; 2]> {
        self.0.get()
    }
}

/// What a step asked the world, kept with what it asked it of and the
/// columns its answer read, as its queries report them (`World::reading`),
/// their corners in units: asked again only where what it asks of moved,
/// or a change to the ground or the objects touched those columns (`kept`).
/// An answer that read nothing is not kept, as nothing says when it goes.
pub struct Asked<K, A>(Option<(K, [[f64; 2]; 2], A)>);

impl<K, A> Default for Asked<K, A> {
    fn default() -> Self {
        Asked(None)
    }
}

impl<K: PartialEq, A: Copy> Asked<K, A> {
    pub const NONE: Asked<K, A> = Asked(None);

    /// Let go where a change touched its columns: looked at every frame
    /// turn, so it sees each change once (`within`).
    pub fn kept(&mut self) {
        if self
            .0
            .as_ref()
            .is_some_and(|&(_, columns, _)| within(columns, &[]))
        {
            self.0 = None;
        }
    }

    /// The answer to `of`: the one kept, else `ask`'s of `world`, whose
    /// queries say what it read. What it read is read of `world` too, so an
    /// answer kept inside another's ask lets that one go with it.
    pub fn of(&mut self, world: World<'_>, of: K, ask: impl FnOnce(World<'_>) -> A) -> A {
        if let Some((k, columns, a)) = &self.0 {
            if *k == of {
                world.read_columns(*columns);
                return *a;
            }
        }
        let read = Columns::default();
        let a = ask(world.reading(&read));
        self.0 = read.get().map(|columns| {
            world.read_columns(columns);
            (of, columns, a)
        });
        a
    }
}
