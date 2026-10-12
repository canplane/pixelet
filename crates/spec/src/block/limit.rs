// pxl-spec/block/limit.rs

//! Where the world ends, one place for every limit an edit and a column
//! meet: the world is open from `BEDROCK_DEPTH` under the ground or sea
//! level, whichever is lower, to `HEADROOM` over the ground or sea level,
//! whichever is higher, in units — Minecraft's −64 to 320 on its own blocks,
//! held from sea level here so a shore below it is not buried. Under the floor
//! is bedrock, the unknown the world holds no more of
//! (`spec/spec/block/cube.json` `unknown`), and the service area is walled by
//! it from inside: a leaf column with ground beside one past the area is
//! bedrock up to its own face, a block of the leaves as any is, so the wall's
//! outer face stands on the area's edge at every zoom (`pxl-spec
//! block/sample.rs` `wall_off`).

pub const BEDROCK_DEPTH: i32 = 64;
pub const HEADROOM: f64 = 128.0;

/// The lowest a block may reach under ground whose face is at `ele`, in
/// units: the bedrock's top.
pub fn floor(ele: f64) -> f64 {
    ele.min(0.0) - f64::from(BEDROCK_DEPTH)
}

/// `floor` in a column whose face is at `ele`, both in cells `per_u` a unit.
pub fn bedrock(ele: i32, per_u: i32) -> i32 {
    (floor(f64::from(ele) / f64::from(per_u)) * f64::from(per_u)) as i32
}

/// The highest a block may reach over ground whose face is at `ele`, in units.
pub fn ceiling(ele: f64) -> f64 {
    ele.max(0.0) + HEADROOM
}

/// Whether a leaf column is the area's edge or past it: it or one of the eight
/// about it is past, `past` asked of each by its offset in leaf columns. The
/// wall is laid on the edge's ground, and a walker keeps off the edge, so it
/// neither leaves the area nor stands on the wall.
pub fn at_edge(past: impl Fn(i64, i64) -> bool) -> bool {
    (-1..=1).any(|dy| (-1..=1).any(|dx| past(dx, dy)))
}
