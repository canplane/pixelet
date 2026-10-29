// pxl-kernel/domain/query/decor/strike.rs

//! A decor thing struck as the block it is: the box it claims, which its
//! model only draws.

use pxl_spec::asset::decor::DECORS;
use pxl_spec::space::coord::v3::V3;

use crate::domain::layer::instance::Instance;
use crate::domain::query::decor::claim;

/// How far any decor's box reaches from its min corner however it turns.
pub fn reach() -> f64 {
    DECORS.iter().map(|d| d.reach()).fold(0.0, f64::max)
}

/// Where a ray from `o` along `d` enters `i`'s box, nearer than `limit`,
/// and the axis it enters across (`Aabb::enters`).
pub fn strike(i: &Instance, o: V3, d: V3, limit: f64) -> Option<(f64, usize)> {
    claim(i)?.enters(o, d).filter(|&(t, _)| t < limit)
}
