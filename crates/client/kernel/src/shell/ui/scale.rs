// pxl-kernel/shell/ui/scale.rs

//! The view's scale at the map center, as the meters a logical pixel spans.

use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::pvf3;

use crate::control::camera::controller::{dist, map_center, units_per_px_at};

/// 0 where the camera stands at the center: no distance, no scale.
pub fn m_per_px() -> f64 {
    let y_u = map_center().map_or(0.0, |c| pvf3::to_real(pvf3::Y(&c)));
    units_per_px_at(dist()) / epsg3857::u_per_m(y_u)
}
