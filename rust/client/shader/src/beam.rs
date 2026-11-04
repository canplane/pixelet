// pxl-shader/beam.rs

//! The beam rays (pxl-engine `beam`): where each cell's rays start off them
//! (`cell_cs`), cast for the eye's camera grid (the corners' walks and lines
//! by hand, pxl-render beam.wgsl, and the shadow map's, light_beam.wgsl,
//! light_cell.wgsl); and the words a line and a node are held in, every
//! observer's.

use spirv_std::glam::UVec3;
use spirv_std::spirv;

use crate::eye::Eye;
use crate::pyramid::{pyramid_at, pyramid_most, pyramid_put};
use crate::observer::{self, Observer};
use crate::light::Head;

/// The beam rays' buffer, the cast's own for a frame (pxl-engine
/// `beam`): the lines, a corner each, the cells' starts and the samples'
/// hits, as the frame's `Grid` lays them (`Grid::size`), or as the light's
/// list does (`light::list_words`) where the pass casts for the map; a
/// runtime array as `Pyramid` is.
pub(crate) type Beams = spirv_std::RuntimeArray<u32>;

/// Where lines, nodes and samples' words lie (pxl-engine
/// `beam::LINE_WORDS`, `beam::NODE_WORDS`, `beam::SAMPLE_WORDS`): the eye's
/// in the beam rays' buffer; the light's in its list's (pxl-engine
/// `light::line`, `light::sample`).
pub(crate) trait Words {
    fn word(&self, k: u32) -> u32;
    fn set(&mut self, k: u32, v: u32);
    /// Word `k` raised to `v` where it is less, whoever else raises it.
    fn most(&mut self, k: u32, v: u32);
}

impl Words for Beams {
    fn word(&self, k: u32) -> u32 {
        pyramid_at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pyramid_put(self, k, v)
    }

    fn most(&mut self, k: u32, v: u32) {
        pyramid_most(self, k, v)
    }
}

impl Words for [u32] {
    fn word(&self, k: u32) -> u32 {
        pxl_engine::at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pxl_engine::fill::put(self, k, v)
    }

    fn most(&mut self, k: u32, v: u32) {
        crate::most(self, k, v)
    }
}

/// Where each cell's rays start (pxl-engine `beam::CELL_WORDS`), a thread a
/// cell of the screen's lattice (observer.rs `cell`, `beam::Grid::span`),
/// past the lines in the beam rays' buffer (`Grid::cell_at`); the shadow
/// map's is pxl-render light_cell.wgsl.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cell_cs"))]
pub fn cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] _head: &Head,
) {
    cell_by(eye, id, beams);
}

/// Cell `id` of `o`'s lattice, where it has one.
fn cell_by<O: Observer>(o: &O, id: UVec3, beams: &mut Beams) {
    let [across, down] = o.cells();
    let c = o.take(&*beams, [id.x, id.y]);
    if c[0] >= across || c[1] >= down {
        return;
    }
    observer::cell(o, beams, c);
}
