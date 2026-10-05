// pxl-geo-bake/terrain/absent.rs

//! What a terrain tile the set holds no object for is. A leaf inside the
//! coverage is left unbaked only when nothing is drawn in it, so it is open
//! sea; one past the coverage has no surface, void. A coarser tile is the fold
//! of the four under it, as every stored tile is.

use std::borrow::Cow;

use pxl_spec::bake::format::{Ground, Occupancy, Terrain, ELE_LANE};
use pxl_spec::cell::cube::cube_types::{ground, sentinel};
use pxl_spec::space::indexer::idx;
use pxl_spec::space::pyramid::{Folds, Tile, GROUND, LEAF_Z, TILE_CELLS};

use crate::coverage::Coverage;
use crate::pyramid::Absent;

pub struct Absence {
    coverage: Coverage,
    sea: Terrain,
    void: Terrain,
}

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
            void: uniform(sentinel::void),
        }
    }

    /// Each cell by whether the leaf it lies in is covered: exact where a
    /// cell lies in one leaf, where the four children's majority is that tile's too.
    fn by_cell(&self, p: Tile) -> Terrain {
        let cells = (0..TILE_CELLS * TILE_CELLS).map(|i| {
            if self.coverage.contains(idx(LEAF_Z, GROUND.cell_at(p, i).base().i)) {
                ground::water
            } else {
                sentinel::void
            }
        });
        let mut t = self.sea.clone();
        t.ground = Ground {
            cells: cells.collect::<Vec<_>>().into(),
        };
        t
    }
}

impl Absent<Terrain> for Absence {
    fn at(&self, p: Tile) -> Cow<'_, Terrain> {
        if self.coverage.holds(p.z, p.tile) {
            Cow::Borrowed(&self.sea)
        } else if !self.coverage.meets(p.z, p.tile) {
            Cow::Borrowed(&self.void)
        } else if GROUND.cell_zoom(p.z) >= LEAF_Z {
            Cow::Owned(self.by_cell(p))
        } else {
            let children = p.children().map(|c| self.at(c));
            Cow::Owned(Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).expect("four children fold"))
        }
    }
}

#[cfg(test)]
#[path = "absent.test.rs"]
mod test;
