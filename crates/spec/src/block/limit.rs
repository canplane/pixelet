// pxl-spec/block/limit.rs

//! Where the world ends, one place for every limit an edit and a column
//! meet: the world is open from `BEDROCK_DEPTH` under the ground or sea
//! level, whichever is lower, to `HEADROOM` over the ground or sea level,
//! whichever is higher, in units — Minecraft's −64 to 320 on its own blocks,
//! held from sea level here so a shore below it is not buried. Under the floor
//! is bedrock, the unknown the world holds no more of
//! (`spec/spec/block/cube.json` `unknown`), and the service area is walled by
//! it: a column with no ground beside one with ground is bedrock up to that
//! ground's face (`wall`), one cell thick at every zoom.

pub const BEDROCK_DEPTH: i32 = 64;
pub const HEADROOM: f64 = 128.0;

/// The bedrock's top in a column whose face is at `ele`, both in cells
/// `per_u` a unit.
pub const fn bedrock(ele: i32, per_u: i32) -> i32 {
    (if ele < 0 { ele } else { 0 }) - BEDROCK_DEPTH * per_u
}

/// The highest a block may reach over ground whose face is at `ele`, in units.
pub fn ceiling(ele: f64) -> f64 {
    ele.max(0.0) + HEADROOM
}

/// The face of the wall in a column with no ground, from the faces of the
/// columns beside it that have ground: the highest, so nothing steps over it.
/// None where none has ground, and the column stays open.
pub fn wall(beside: impl Iterator<Item = i32>) -> Option<i32> {
    beside.max()
}
