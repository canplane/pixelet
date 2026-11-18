// pxl-content/lib.rs

//! Pixelet's content, the rules the CPU runs and the GPU's WGSL is written to,
//! as the engine's are (pxl-engine). `Pixelet` answers the rules the engine asks of a content
//! (pxl-engine `content::Content`) with its caves (`cave`) and its strata (`strata`). The
//! crate also holds the decor the ground holds and how it is generated (`scatter`,
//! `generation`), by the catalog's rows (pxl-spec asset/decor.spec.rs), and the clouds' field
//! (`cloud`). The core and the renderer name `Pixelet` where they hand the engine its content.

pub mod cave;
pub mod cloud;
pub mod generation;
pub mod halve;
pub mod param;
pub mod scatter;
pub mod strata;

use pxl_engine::content::Content;
use pxl_spec::space::coord::zoom::Zoom;

/// Pixelet's content: the rules its world follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixelet;

impl Content for Pixelet {
    fn hollow(node: (Zoom, [i32; 3])) -> u32 {
        cave::hollow(node)
    }
    fn by_depth(cell_zoom: Zoom, asset_id: u32, top: i32, z: i32) -> u32 {
        strata::by_depth(cell_zoom, asset_id, top, z)
    }
    fn bedrock(ele: i32) -> i32 {
        strata::bedrock(ele)
    }
    fn earth(z: i32) -> u32 {
        strata::earth(z)
    }
}
