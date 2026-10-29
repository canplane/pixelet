// pxl-kernel/domain/scene/frame.rs

//! The frame the scene is drawn in: where the eye is, what it sees, and how
//! long since the last.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;

#[derive(Clone, Copy, Default)]
pub struct Frame {
    /// The sea under the eye, in the world's doubles; what is drawn is measured about it.
    pub anchor: V3,
    /// Draw translations are taken relative to the eye.
    pub eye: V3,
    /// About the anchor.
    pub frustum: Frustum,
    pub camera: V3,
    pub delta_s: f64,
}

impl Frame {
    /// Whether a sphere about `at`, in the world's doubles, may show: for a
    /// bound that is round, a model's about its origin or a mark's reach
    /// about its point. A box is `sees_box`'s.
    pub fn sees(&self, at: V3, radius: f64) -> bool {
        let p = self.world_of(at);
        self.holds_camera(p, radius) || self.frustum.sees(p, radius)
    }

    /// Whether the box from `min` to `max`, in the world's doubles, may show.
    pub fn sees_box(&self, min: V3, max: V3) -> bool {
        let lo = self.world_of(min);
        let hi: V3 = core::array::from_fn(|k| lo[k] + max[k] - min[k]);
        let middle: V3 = core::array::from_fn(|k| (lo[k] + hi[k]) / 2.0);
        let half = v3::len(&core::array::from_fn(|k| (hi[k] - lo[k]) / 2.0));
        self.holds_camera(middle, half) || self.frustum.sees_box(lo, hi)
    }

    /// Whether what stands `radius` about the point `p`, about the anchor,
    /// holds the camera, and is so always seen.
    fn holds_camera(&self, p: V3, radius: f64) -> bool {
        let c = self.camera;
        v3::len(&[c[0] - p[0], c[1] - p[1], c[2] - p[2]]) <= radius
    }

    /// A point about the anchor, its X on the anchor's lap.
    pub fn world_of(&self, at: V3) -> V3 {
        let x = world::nearest(at[0], self.anchor[0]);
        [
            x - self.anchor[0],
            at[1] - self.anchor[1],
            at[2] - self.anchor[2],
        ]
    }
}
