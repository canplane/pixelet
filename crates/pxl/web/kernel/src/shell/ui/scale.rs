// pxl-kernel/shell/ui/scale.rs

//! The view's scale at the map center, as the meters a logical pixel spans.

use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::pvf3;

use crate::control::camera::config::camera_cfg;

/// 0 where the camera stands at the center: no distance, no scale.
pub fn m_per_px(screen_height: f64) -> f64 {
    let dist = crate::control::camera::controller::radius();
    let fov_rad = camera_cfg::projection::FOV.to_radians();
    let u_per_px = 2.0 * libm::tan(fov_rad / 2.0) * dist / screen_height;
    let y_u = crate::control::camera::controller::map_center().map_or(0.0, |c| pvf3::to_real(pvf3::Y(&c)));
    u_per_px / epsg3857::u_per_m(y_u)
}
