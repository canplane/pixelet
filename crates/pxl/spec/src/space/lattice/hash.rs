// pxl-spec/space/lattice/hash.rs

//! The lattice address hash: Teschner's large primes decorrelate the axes;
//! a fourth folds in a tier so a parent cannot collide with its child.

const P1: i32 = 73856093;
const P2: i32 = 19349663;
const P3: i32 = 83492791;
const P4: i32 = 32452843;

pub fn hash3(x: i32, y: i32, z: i32) -> u32 {
    (x.wrapping_mul(P1) ^ y.wrapping_mul(P2) ^ z.wrapping_mul(P3)) as u32
}

pub fn hash4(x: i32, y: i32, z: i32, tier: u32) -> u32 {
    (x.wrapping_mul(P1) ^ y.wrapping_mul(P2) ^ z.wrapping_mul(P3) ^ (tier as i32).wrapping_mul(P4)) as u32
}
