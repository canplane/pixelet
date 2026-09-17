// xpute-core/ds/lattice_hash.rs

//! One hash for an integer lattice address, shared by every structure that
//! buckets cells: the spatial hash's broadphase and the billboard lattice's
//! nodes. Teschner's three large primes decorrelate the axes before the xor,
//! so a lattice of points does not collapse onto a few buckets; a fourth
//! folds in a tier, for a table that holds several tiers of one lattice at
//! once, where a parent's address would otherwise collide with its child's.

const P1: i32 = 73856093;
const P2: i32 = 19349663;
const P3: i32 = 83492791;
const P4: i32 = 32452843;

pub fn lattice_hash3(x: i32, y: i32, z: i32) -> u32 {
    (x.wrapping_mul(P1) ^ y.wrapping_mul(P2) ^ z.wrapping_mul(P3)) as u32
}

pub fn lattice_hash4(x: i32, y: i32, z: i32, tier: u32) -> u32 {
    (x.wrapping_mul(P1) ^ y.wrapping_mul(P2) ^ z.wrapping_mul(P3) ^ (tier as i32).wrapping_mul(P4)) as u32
}

#[cfg(test)]
#[path = "lattice_hash.test.rs"]
mod test;
