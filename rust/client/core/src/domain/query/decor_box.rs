// pxl-core/domain/query/decor_box.rs

//! A decor thing is struck as the block it is: by the box it claims, not by
//! the model drawn in that box.

use pxl_spec::asset::decor::DECORS;
use pxl_spec::space::coord::v3::V3;
use xpute_core::global::Global;

use crate::domain::layer::instance::Instance;
use crate::domain::query::decor::claim;

/// The farthest any decor's box in decor.json reaches from its min corner,
/// however it turns. It is computed once.
pub fn box_extent() -> f64 {
    static EXTENT: Global<f64> = Global::new();
    *EXTENT.get(|| DECORS.iter().map(|d| d.box_extent()).fold(0.0, f64::max))
}

/// Where a ray from `o` along `d` enters `i`'s box, if that is nearer than
/// `limit`, and the axis it enters across (`Aabb::enters`).
pub fn intersect(i: &Instance, o: V3, d: V3, limit: f64) -> Option<(f64, usize)> {
    claim(i)?.enters(o, d).filter(|&(t, _)| t < limit)
}
