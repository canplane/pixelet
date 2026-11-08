// pxl-core/domain/scene/frame.rs

//! The frame the scene is drawn in. What is drawn is measured about the
//! anchor, the point at sea level under the eye.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;

#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The point at sea level under the eye, in world coordinates as f64.
    pub anchor: V3,
    /// The eye in world coordinates as f64. It decides what is listed, but no
    /// draw's position or receiver box depends on it (pxl-render gpu/draw.rs
    /// `Position`, `ReceiverBox`), so a draw does not change when only the eye
    /// moves.
    pub eye: V3,
    /// The view frustum, measured about the anchor.
    pub frustum: Frustum,
    pub camera: V3,
    /// How many of the display's own pixels one unit spans at one unit from
    /// the eye, in the middle of the view. It tells what a motion there is
    /// worth on the screen. It is zero when there is no display.
    pub px_per_unit: f64,
    pub delta_s: f64,
}

impl Frame {
    /// Whether a sphere of `radius` about `at`, in world coordinates, may
    /// show. It serves a round bound, such as a model's about its origin or a
    /// mark's reach about its point; a box goes to `sees_box`.
    pub fn sees(&self, at: V3, radius: f64) -> bool {
        let p = self.world_of(at);
        self.holds_camera(p, radius) || self.frustum.sees(p, radius)
    }

    /// Whether the box from `min` to `max`, in world coordinates, may show.
    pub fn sees_box(&self, min: V3, max: V3) -> bool {
        let lo = self.world_of(min);
        let hi: V3 = core::array::from_fn(|k| lo[k] + max[k] - min[k]);
        let middle: V3 = core::array::from_fn(|k| (lo[k] + hi[k]) / 2.0);
        let half = v3::len(&core::array::from_fn(|k| (hi[k] - lo[k]) / 2.0));
        self.holds_camera(middle, half) || self.frustum.sees_box(lo, hi)
    }

    /// Whether the sphere of `radius` about `p`, measured about the anchor,
    /// holds the camera. A bound that holds the camera always shows, whatever
    /// the frustum says.
    fn holds_camera(&self, p: V3, radius: f64) -> bool {
        let c = self.camera;
        v3::len(&[c[0] - p[0], c[1] - p[1], c[2] - p[2]]) <= radius
    }

    /// `at` measured about the anchor. X is taken on the anchor's lap of the
    /// world, which wraps in X, so a point across the seam comes out near
    /// the anchor.
    pub fn world_of(&self, at: V3) -> V3 {
        let x = world::nearest(at[0], self.anchor[0]);
        [
            x - self.anchor[0],
            at[1] - self.anchor[1],
            at[2] - self.anchor[2],
        ]
    }
}
