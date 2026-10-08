// pxl-geo-bake/terrain/absent.rs

//! What a terrain tile the set holds no object for is. A leaf inside the
//! coverage is left unbaked only when nothing is drawn in it, so it is open
//! sea; past the coverage nothing is known, sea or land, so it is unknown,
//! which gives no surface and leaves the planet's picture under it to be
//! seen. A coarser tile is the fold of the four under it, as every stored
//! tile is.

use std::borrow::Cow;

use pxl_spec::bake::format::{Ground, Occupancy, Terrain, ELE_LANE};
use pxl_spec::cell::cube_types::{ground, sentinel};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::tile::Tile;

use crate::coverage::Coverage;
use crate::pyramid::{Absent, Layer};

pub struct Absence {
    coverage: Coverage,
    sea: Terrain,
    unknown: Terrain,
}

/// A tile of one look at sea level: no decks, and no child under it.
fn uniform(look: u8) -> Terrain {
    let mut t = Terrain::leaf(
        vec![0; ELE_LANE],
        Ground {
            cells: vec![look; (TILE_CELLS * TILE_CELLS) as usize].into(),
        },
        Default::default(),
    );
    t.index.occupancy = Occupancy::ZERO;
    t
}

impl Absence {
    pub fn new(coverage: Coverage) -> Absence {
        Absence {
            coverage,
            sea: uniform(ground::water),
            unknown: uniform(sentinel::unknown),
        }
    }
}

impl Absent<Terrain> for Absence {
    fn at(&self, p: Tile) -> Cow<'_, Terrain> {
        if self.coverage.holds(p.zoom, p.tile) {
            Cow::Borrowed(&self.sea)
        } else if !self.coverage.meets(p.zoom, p.tile) {
            Cow::Borrowed(&self.unknown)
        } else {
            let children = p.children().map(|c| self.at(c));
            Cow::Owned(Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).expect("four children fold"))
        }
    }
}

#[cfg(test)]
#[path = "absent.test.rs"]
mod test;
