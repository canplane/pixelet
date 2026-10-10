// pxl-geo-bake/terrain/absent.rs

//! What a terrain tile the set holds no object for is. A leaf inside the
//! coverage is left unbaked only when nothing is drawn in it, so it is open
//! sea; a shard past it is its coast (`coast.rs`), and a tile finer than a
//! shard the coast's cells it lies in. A coarser tile is the fold of the four
//! under it, as every stored tile is.

use std::borrow::Cow;
use std::collections::HashMap;

use pxl_spec::bake::format::{Ground, Occupancy, Terrain, ELE_LANE};
use pxl_spec::cell::cube_types::{ground, sentinel};
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::Tile;

use crate::coverage::Coverage;
use crate::pyramid::{Absent, Layer};

pub struct Absence {
    coverage: Coverage,
    coast: HashMap<Tile, Terrain>,
    sea: Terrain,
    void: Terrain,
}

/// A tile of only a surface at sea level: no decks, and no child under it.
pub fn surface(ground: Ground) -> Terrain {
    let mut t = Terrain::leaf(vec![0; ELE_LANE], ground, Default::default());
    t.index.occupancy = Occupancy::ZERO;
    t
}

fn uniform(look: u8) -> Terrain {
    surface(Ground {
        cells: vec![look; (TILE_CELLS * TILE_CELLS) as usize].into(),
    })
}

impl Absence {
    /// `coast`: each shard past the coverage, as `coast::read` gives it.
    pub fn new(coverage: Coverage, coast: HashMap<Tile, Terrain>) -> Absence {
        Absence {
            coverage,
            coast,
            sea: uniform(ground::water),
            void: uniform(sentinel::void),
        }
    }

    /// A tile finer than its shard: each cell the shard's cell it lies in.
    fn within(&self, shard: &Terrain, p: Tile) -> Terrain {
        let cells = (0..TILE_CELLS * TILE_CELLS).map(|i| shard.ground.cells[GROUND.cell_of(SHARD_ZOOM, &GROUND.cell_at(p, i).base()).index() as usize]);
        surface(Ground {
            cells: cells.collect::<Vec<_>>().into(),
        })
    }
}

impl Absent<Terrain> for Absence {
    fn at(&self, p: Tile) -> Cow<'_, Terrain> {
        if self.coverage.holds(p.zoom, p.tile) {
            Cow::Borrowed(&self.sea)
        } else if p.zoom >= SHARD_ZOOM {
            let shard = Tile::over(SHARD_ZOOM, p.first_leaf());
            match self.coast.get(&shard) {
                Some(t) if p == shard => Cow::Borrowed(t),
                Some(t) => Cow::Owned(self.within(t, p)),
                // Past every top tile the coverage meets: nothing stored folds from it.
                None => Cow::Borrowed(&self.void),
            }
        } else {
            let children = p.children().map(|c| self.at(c));
            Cow::Owned(Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).expect("four children fold"))
        }
    }
}

#[cfg(test)]
#[path = "absent.test.rs"]
mod test;
