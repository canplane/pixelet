// pxl-engine/content.rs

//! gpu-portable: the rules the engine asks of the content it draws, the one
//! trait a content implements: where its caves take the ground, its strata,
//! and how far its decor reaches, which the light's box reads. A world type
//! carries its content as an associated type of the trait it implements
//! where those rules are asked (`ground::Resident`); a function that reads
//! no world takes it as a type parameter. The values a
//! content's palette takes are the engine's value space (`value`).

#[cfg(test)]
use crate::value::{UNKNOWN, VOID};
use crate::Zoom;

/// The rules a content's world follows, each answered the same on the CPU
/// and on the GPU.
pub trait Content {
    /// The share of the cube `c` at `zoom` its caves take, in
    /// `answer::FULL`'s units, where the ground may give one (`ground::node`):
    /// the mean of its leaves', so a coarse cell holds what they do and
    /// `FULL` is a cave all through.
    fn hollow(node: (Zoom, [i32; 3])) -> u32;

    /// What the ground is in cell `z` of a column of `cell_zoom` whose palette
    /// index is `palette_idx` and whose top is `top` cells: by depth under
    /// its top, `value::VOID` over it, `value::UNKNOWN` under its bedrock and
    /// for the wall all through.
    fn by_depth(cell_zoom: Zoom, palette_idx: u32, top: i32, z: i32) -> u32;

    /// The bedrock's top under a face at `face` leaf cells, in leaf cells:
    /// the lowest a cube may reach there.
    fn bedrock(face: i32) -> i32;

    /// Cell `z` past the service area: the Earth's ground there.
    fn earth(z: i32) -> u32;
}

/// A content for the engine's own tests, no content's: no caves, a column
/// its face's entry all the way down, a bedrock 64 under it or sea level,
/// and past the area bedrock under sea level.
#[cfg(test)]
pub(crate) struct Sample;

#[cfg(test)]
impl Sample {
    /// A ground entry the tests lay.
    pub(crate) const GROUND: u32 = 66;
}

#[cfg(test)]
impl Content for Sample {
    fn hollow(_: (Zoom, [i32; 3])) -> u32 {
        0
    }
    fn by_depth(_: Zoom, palette_idx: u32, top: i32, z: i32) -> u32 {
        if z >= top {
            VOID
        } else {
            palette_idx
        }
    }
    fn bedrock(face: i32) -> i32 {
        face.min(crate::SEA_LEVEL) - 64
    }
    fn earth(z: i32) -> u32 {
        if z < crate::SEA_LEVEL {
            UNKNOWN
        } else {
            VOID
        }
    }
}
