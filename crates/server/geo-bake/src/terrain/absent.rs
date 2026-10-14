// pxl-geo-bake/terrain/absent.rs

//! What a terrain tile the set holds no object for is. A leaf inside a shard
//! the set holds (`Run::baked`) is left unbaked only when nothing is drawn in
//! it, so it is open sea; past them nothing is known, sea or land, so it is unknown,
//! which gives no surface and leaves the planet's picture under it to be
//! seen. A coarser tile is the fold of the four under it, as every stored
//! tile is.

use std::borrow::Cow;

use pxl_spec::bake::tile::terrain::{Ground, TerrainTile, HEIGHT_LANE};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::block::cube::{ground, sentinel};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::tile::Tile;

use crate::coverage::Coverage;
use crate::pyramid::{Absent, Base};

pub struct Absence {
    coverage: Coverage,
    sea: TerrainTile,
    unknown: TerrainTile,
}

/// A tile of one look at sea level: no lines, and no child under it.
fn uniform(look: u8) -> TerrainTile {
    let mut t = TerrainTile::leaf(
        vec![0; HEIGHT_LANE],
        Ground {
            cells: vec![look; (TILE_CELLS * TILE_CELLS) as usize].into(),
        },
        Default::default(),
    );
    t.occupancy = Occupancy::ZERO;
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

impl Absent<TerrainTile> for Absence {
    fn at(&self, p: Tile) -> Cow<'_, TerrainTile> {
        if self.coverage.holds(p.zoom, p.at) {
            Cow::Borrowed(&self.sea)
        } else if !self.coverage.meets(p.zoom, p.at) {
            Cow::Borrowed(&self.unknown)
        } else {
            let children = p.children().map(|c| self.at(c));
            Cow::Owned(TerrainTile::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).expect("four children fold"))
        }
    }
}

#[cfg(test)]
#[path = "absent.test.rs"]
mod test;
