// pxl-engine/content.rs

//! wgsl-port: the rules the engine asks of the content it draws, as the one trait a
//! content implements: how its caves hollow the ground, its strata, its bedrock, and the
//! Earth's ground past the service area. A world type carries its content as an associated
//! type of the trait through which those rules are asked (`ground::Resident`); a function that
//! reads no world takes the content as a type parameter. The values in a content's palette
//! are pxl-spec's value space (`value`).

#[cfg(test)]
use pxl_spec::asset::cube::Sentinel;
#[cfg(test)]
use pxl_spec::space::coord::world::SEA_LEVEL;
use pxl_spec::space::coord::zoom::Zoom;

/// The rules a content's world follows, each answered the same on the CPU
/// and on the GPU.
pub trait Content {
    /// The share of the cube at `node` that its caves take, in units of `share::FULL`, asked
    /// where the ground may give the cube a cave (`ground::node`). It is the mean over the cube's
    /// leaves, so a coarse cell holds what its leaves hold, and `FULL` is a cave all through.
    fn hollow(node: (Zoom, [i32; 3])) -> u32;

    /// What the ground is in cell `z` over a cell of the plane at `cell_zoom` whose asset id is
    /// `asset_id` and whose elevation is `ele_cells` cells. Under it the ground goes by depth; above
    /// it is `Sentinel::Void`; under the bedrock, and all through the wall, it is
    /// `Sentinel::Unknown`.
    fn by_depth(cell_zoom: Zoom, asset_id: u32, ele_cells: i32, z: i32) -> u32;

    /// The top of the bedrock under ground `ele` leaf cells high, in leaf cells: the lowest a
    /// cube may reach there.
    fn bedrock(ele: i32) -> i32;

    /// Cell `z` past the service area, where the Earth's ground stands.
    fn earth(z: i32) -> u32;
}

/// A content for the engine's own tests. It has no caves, and the ground is its face's asset
/// all the way down. The bedrock lies 64 cells under the elevation or under sea level, whichever is
/// lower, and past the area everything under sea level is unknown.
#[cfg(test)]
pub(crate) struct Sample;

#[cfg(test)]
impl Sample {
    /// A ground cube the tests lay.
    pub(crate) const GROUND: u32 = 66;
}

#[cfg(test)]
impl Content for Sample {
    fn hollow(_: (Zoom, [i32; 3])) -> u32 {
        0
    }
    fn by_depth(_: Zoom, asset_id: u32, ele_cells: i32, z: i32) -> u32 {
        if z >= ele_cells {
            Sentinel::Void as u32
        } else {
            asset_id
        }
    }
    fn bedrock(ele: i32) -> i32 {
        ele.min(SEA_LEVEL) - 64
    }
    fn earth(z: i32) -> u32 {
        if z < SEA_LEVEL {
            Sentinel::Unknown as u32
        } else {
            Sentinel::Void as u32
        }
    }
}
