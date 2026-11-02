// pxl-engine/content.rs

//! gpu-portable: the rules the engine asks of the content it draws, the one
//! trait a content implements: where its caves take the ground, its strata,
//! and how far its decor reaches, which the light's box reads. A world type
//! carries its content as an associated type of the trait it implements
//! where those rules are asked (`ground::Resident`); a function that reads
//! no world takes it as a type parameter (`light::Light::up`). The values a
//! content's palette takes are the engine's value space (`value`).

#[cfg(test)]
use crate::value::{UNKNOWN, VOID};
use crate::Zoom;

/// The rules a content's world follows, each answered the same on the CPU
/// and on the GPU.
pub trait Content {
    /// Whether the cube `c` at `zoom` is a cave's air, where the ground may
    /// take one (`ground::Cell::asset`).
    fn hollow(node: (Zoom, [i32; 3])) -> bool;

    /// Whether a cave takes all of the cube `c` at `zoom`, whatever cube
    /// under it is asked: where it does, `hollow` does of every one.
    fn all_hollow(node: (Zoom, [i32; 3])) -> bool;

    /// What the ground is in cell `z` of a column of `cell` whose palette
    /// index is `palette_idx` and whose top is `top` cells: by depth under
    /// its top, `value::VOID` over it, `value::UNKNOWN` under its bedrock and
    /// for the wall all through.
    fn by_depth(cell: Zoom, palette_idx: u32, top: i32, z: i32) -> u32;

    /// The bedrock's top under a face at `face` leaf cells, in leaf cells:
    /// the lowest a cube may reach there.
    fn bedrock(face: i32) -> i32;

    /// The highest a cube may reach over a face at `face` leaf cells, in
    /// leaf cells.
    fn ceiling(face: i32) -> i32;

    /// Cell `z` past the service area: the Earth's ground there.
    fn earth(z: i32) -> u32;

    /// The tallest decor and the widest, in unit cells: as high over what it
    /// stands on as anything it generates reaches, and as far across
    /// (`light::Light::up`, `Light::ring`).
    const TALLEST: i32;
    const WIDEST: i32;
}

/// A content for the engine's own tests, no content's: no caves, a column
/// its face's entry all the way down, a bedrock 64 under it or sea level,
/// room 128 over it, past the area bedrock under sea level, and decor three
/// units tall and two wide.
#[cfg(test)]
pub(crate) struct Sample;

#[cfg(test)]
impl Sample {
    /// A ground entry the tests lay.
    pub(crate) const GROUND: u32 = 66;
}

#[cfg(test)]
impl Content for Sample {
    fn hollow(_: (Zoom, [i32; 3])) -> bool {
        false
    }
    fn all_hollow(_: (Zoom, [i32; 3])) -> bool {
        false
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
    fn ceiling(face: i32) -> i32 {
        face.max(crate::SEA_LEVEL) + 128
    }
    fn earth(z: i32) -> u32 {
        if z < crate::SEA_LEVEL {
            UNKNOWN
        } else {
            VOID
        }
    }
    const TALLEST: i32 = 3;
    const WIDEST: i32 = 2;
}
