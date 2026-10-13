// pxl-kernel/layer/division.rs

//! The divisions as a layer of the pyramid, held over the focus alone: its
//! line is asked as a destination's, and the deepest tile held on it names
//! the cell. Nothing here is drawn; the name is read straight into the UI
//! snapshot.

use pxl_spec::bake::tile::division::{self, Division, DivisionTile, DIVISION_LEAF};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{index_of, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_guest::global::Global;

use crate::io::demand::{Phases, Stream};
use crate::io::failure::Failure;
use crate::layer::residency::Residency;
use crate::layer::{Asks, Base};

static DIVISION: Global<Residency<DivisionTile>> = Global::new();

impl Base for DivisionTile {
    const STREAM: Stream = Stream::division;
    const LATTICE: Lattice = GROUND;
    const ASKS: Asks = Asks::Line;
    const LEAF: Zoom = DIVISION_LEAF;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn residency() -> &'static mut Residency<DivisionTile> {
        DIVISION.get(Residency::default)
    }

    /// A handful over the focus, so held on the heap with its names decoded.
    fn receive(_record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<DivisionTile, Failure> {
        Ok(division::decode(pkt)?)
    }
}

/// The finest tile of the focus's line, which demand asks for with the line.
pub fn line(focus: Option<Q3>) -> Option<Tile> {
    focus.map(|at| {
        let [x, y, _] = index_of(LEAF_ZOOM, &at);
        Tile::over(DivisionTile::LEAF, [x, y])
    })
}

/// The divisions of the cell `at` is in, broad to narrow, from the deepest
/// tile held over it: the held parent names the point while the tile under
/// it is on its way.
pub fn named(at: &Q3, mut each: impl FnMut(&Division)) {
    let held = DivisionTile::residency();
    let Some(from) = line(Some(*at)).and_then(|p| held.drawn_from(p)) else { return };
    if let Some(tile) = held.get(from) {
        tile.name_at(GROUND.cell_of(from.zoom, at).index()).for_each(&mut each);
    }
}
