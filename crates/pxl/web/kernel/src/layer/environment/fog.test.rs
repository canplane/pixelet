// pxl-kernel/layer/environment/fog.test.rs

use super::*;
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};

/// Where the scale leaves 1: the distance the values were tuned at.
const TUNED: f64 = render_cfg::FOG_FAR / render_cfg::FOG_PER_DIST;

#[test]
fn the_view_holds_its_reach_near_and_scales_it_with_the_distance_past() {
    // Up to where they were tuned, the values are render_cfg's as they stand.
    for dist in [MIN_DIST, 16.0, TUNED] {
        assert_eq!(fog_far(dist), render_cfg::FOG_FAR);
        assert_eq!(far_clip(dist), render_cfg::FAR_CLIP);
    }
    // Past it, all of them in proportion to the distance.
    let far = MAX_DIST;
    assert_eq!(fog_far(far), render_cfg::FOG_PER_DIST * far);
    assert_eq!(far_clip(far) / near_clip(far), render_cfg::FAR_CLIP / camera_cfg::projection::NEAR);
}
