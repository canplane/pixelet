// pxl-kernel/shell/ui/compass.rs

//! What the compass's press does.

/// Nothing while following the heading, which would turn it straight back.
pub fn compass_reset() {
    if crate::control::camera::store::camera_store().is_follow_mode {
        return;
    }
    crate::control::camera::controller::reset_azimuth();
}
