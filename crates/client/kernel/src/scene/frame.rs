// pxl-kernel/scene/frame.rs

//! The frame the scene is drawn in: where the eye is, what it sees, and how
//! long since the last.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::globe::Globe;
use pxl_spec::space::coord::v3::{self, V3};

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
    pub globe: Option<Globe>,
}

impl Frame {
    /// Whether a sphere about `at`, in the world's doubles, may show.
    pub fn sees(&self, at: V3, radius: f64) -> bool {
        let [x, y, z] = self.world_of(at);
        // Cull where the renderer draws it, on the globe; a flat-world cull leaves
        // holes at the horizon.
        let [x, y, z] = match self.globe {
            Some(globe) => globe.place([x, y, z]),
            None => [x, y, z],
        };
        let c = self.camera;
        // A sphere containing the camera is always visible.
        if v3::len(&[c[0] - x, c[1] - y, c[2] - z]) <= radius {
            return true;
        }
        // Horizon cull (Cozzi & Ring): a point is above the eye's horizon while
        // E·P >= r², from the planet's center, widened by the sphere's radius.
        self.frustum.sees([x, y, z], radius)
            && self.globe.is_none_or(|globe| {
                let r = globe.radius_u();
                let (e, p) = ([c[0], c[1], c[2] + r], [x, y, z + r]);
                v3::dot(&e, &p) + radius * v3::len(&e) >= r * r
            })
    }

    /// A point about the anchor, its X on the anchor's lap.
    pub fn world_of(&self, at: V3) -> V3 {
        let x = pxl_spec::space::coord::world::nearest(at[0], self.anchor[0]);
        [x - self.anchor[0], at[1] - self.anchor[1], at[2] - self.anchor[2]]
    }
}
