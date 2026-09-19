// xpute-core/math/vec3.rs

//! A point or a direction as three doubles, written into by the matrix
//! functions and read by whatever asked.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
