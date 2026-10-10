// pxl-render/mesh/paint/column.rs

//! A chunk's blocks as runs to draw, a cell a column across with heights in
//! the sample's cells. A column is a whole run from the floor to the ground:
//! a part-filled cell would have to be drawn as something inside the cell,
//! and in flat places the whole zoom turned to cubes standing apart.

use pxl_spec::bake::format::GroundId;
use pxl_spec::block::color;
use pxl_spec::block::cube::{sentinel, Ground};
use pxl_spec::block::object::Cube;
use pxl_spec::block::sample::{ChunkSample, Refused};
use pxl_spec::block::stand::{compose, Block, Stand};
use std::sync::LazyLock;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::mesh::paint::object::PAINTER;
use crate::mesh::paint::palette::{rgb255, LAYER_IDX};
use crate::mesh::run::RunWriter;
use crate::mesh::word::{pack_material, Side, NO_LAYER};

/// Each byte's material word and layer: a look's, or unknown's for a byte
/// that is none.
pub struct Materials {
    looks: [Option<(u32, u8)>; 256],
    unknown: (u32, u8),
}

pub static MATERIALS: LazyLock<Materials> = LazyLock::new(Materials::of_palette);

impl Materials {
    /// What a cell draws as: a byte that is no look as unknown.
    pub fn drawn(&self, cell: GroundId) -> (u32, u8) {
        self.looks[cell as usize].unwrap_or(self.unknown)
    }

    fn of_palette() -> Materials {
        let mut looks = [None; 256];
        for g in Ground::ALL {
            let rgb = color::ground(g as u8).map_or([0; 3], |s| rgb255(s.rgb));
            looks[g as usize] = Some((pack_material(Side::BASE, rgb), LAYER_IDX(Cube::Ground(g)) as u8));
        }
        let rgb = color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb));
        Materials {
            looks,
            unknown: (pack_material(Side::BASE, rgb), NO_LAYER as u8),
        }
    }
}

/// What a block that fills its cell is drawn as.
fn material(block: Block) -> (u32, u8) {
    match block {
        Block::Ground(g) => MATERIALS.drawn(g),
        Block::Placed(state) => PAINTER.material_of(state).or_bug(Errno::ENOTRECOVERABLE),
    }
}

/// The runs of the grid's `rows`, from `stands` (`stands_of`); `next` is the
/// first stand of the first row and is left at the next row's.
pub fn write_rows(direct: &mut RunWriter, sample: &ChunkSample, stands: &[(u32, Stand)], rows: core::ops::Range<u32>, next: &mut usize) -> Result<(), Refused> {
    for i in rows.start * sample.span..rows.end * sample.span {
        direct.column(i);
        let first = *next;
        while *next < stands.len() && stands[*next].0 == i {
            *next += 1;
        }
        compose(sample, &stands[first..*next], i as usize, sample.floor, |lo, hi, block| match block.fills() {
            true => {
                let (m, l) = material(block);
                direct.run(lo, hi, m, l)
            }
            false => Ok(()),
        })?;
    }
    Ok(())
}
