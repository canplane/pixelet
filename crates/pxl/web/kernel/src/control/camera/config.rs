// pxl-kernel/control/camera/config.rs

//! Camera projection and interaction limits.

/// Default tilt angle in degrees from the ground plane.
pub const DEFAULT_TILT_DEG: f64 = 30.0;

pub mod camera_cfg {
    pub mod projection {
        pub const FOV: f64 = 50.0;
        pub const NEAR: f64 = 0.1;
    }
    pub mod view {

        /// 4 * 4.
        pub const DEFAULT_DIST: f64 = 16.0;
        pub const DEFAULT_AZIMUTH: f64 = 0.0;
        /// 0 = straight down from above, PI / 2 = horizontal. (90 - DEFAULT_TILT_DEG) * PI / 180.
        pub const DEFAULT_POLAR: f64 = 1.0471975511965976;
        /// Picked against the scale bar's own reading, not as a raw distance
        /// number with no felt meaning: at a 1080px viewport and Seoul's
        /// latitude, a bar of at most 56px reads total_meters ≈ 0.092 * dist
        /// before its 1/2/5 rounding (see shell/ui/scale_bar.rs), giving
        /// MIN_DIST ≈ 0.18m — a felt-right target, not an exact contract (it
        /// shifts with real screen size and latitude, as the scale bar's own
        /// reading does). 1 * 2.
        pub const MIN_DIST: f64 = 2.0;
        /// As far out as the terrain goes: looking straight down from here the
        /// ground wants step 2^11, which level 10, the pyramid's top
        /// (TOP_LEVEL), draws with its cells averaged 2×2 — about 24 km
        /// on the scale bar. Past it no level is coarse enough. The band and
        /// the clip planes follow the reach from up here, which is the planet's
        /// own edge, so every zoom out to here shows the same octaves of
        /// ground. 1024 * 2^8.
        pub const MAX_DIST: f64 = 262144.0;
    }
    pub mod free {
        /// How high the free camera's eye stands over the ground it walks
        /// on, in units. A knob, set by eye and derived from nothing: no
        /// relation to the lattice, the orbit's pivot or anything else
        /// fixes it.
        pub const EYE_HEIGHT_U: f64 = 1.5;
    }
    pub mod input {
        /// How much of the viewport a vertical drag has to cover to sweep the
        /// whole tilt range. Stated against the screen rather than as a rad/px
        /// rate, because that is the only form comparable to azimuth: azimuth
        /// has no gain at all, it tracks the cursor's angle around screen center
        /// 1:1 (arcball — control/camera/input.rs's do_rotate).
        ///
        /// A fixed rad/px rate is what makes the two diverge: -0.012 at a 1080px
        /// reference puts the whole 90-degree range inside about a tenth of a
        /// screen height, roughly six times azimuth's angular speed for the same
        /// finger travel. Both damp toward their target at the same rate, so a
        /// polar settle after release would cover six times the distance and read
        /// as the gesture still being held.
        pub const TILT_SWEEP_FRACTION: f64 = 0.5;
        pub const DOLLY_GAIN: f64 = 0.0022;
        /// azimuth is derived from atan2, unstable at small radius (Δangle ≈
        /// tangential_jitter / radius — a few px of jitter swings the angle
        /// wildly up close). Below this radius, azimuth tracking freezes instead
        /// of following it (control/camera/input.rs's do_rotate and update_pinch).
        pub const MIN_ROTATE_RADIUS_PX: f64 = 24.0;
        pub const MIN_TWIST_DIST_PX: f64 = 24.0;
        /// Once a rotate-drag's total displacement passes this radius, whichever
        /// axis dominates locks in for the rest of the gesture
        /// (control/camera/input.rs's do_rotate). Without the lock a visually
        /// "straight down" drag also sweeps azimuth unless it starts exactly on
        /// the vertical centerline, which reads as a large unrelated spin once
        /// polar has saturated against the ground-collision clamp. Smaller than MIN_ROTATE_RADIUS_PX on purpose — this needs only
        /// enough displacement to tell vertical from horizontal, not a stable
        /// angle reading.
        pub const AXIS_LOCK_THRESHOLD_PX: f64 = 12.0;
        /// A tap is a press whose total displacement from pointerdown stays
        /// inside this (control/camera/input.rs's tap_start_x/y). Gating on any
        /// nonzero pointermove delta instead would swallow genuine taps — map
        /// recenter, object place and erase — because real pointers (trackpad
        /// clicks, high-DPI mice, touch) report a pixel or two of jitter even on
        /// a stationary click. Small enough that an intentional drag still starts
        /// panning immediately, large enough to absorb that jitter.
        pub const TAP_DRAG_THRESHOLD_PX: f64 = 4.0;
    }
    pub mod smooth {
        pub const ROT_DAMP: f64 = 10.0;
        pub const POS_DAMP: f64 = 10.0;
        pub const CAM_DAMP: f64 = 10.0;
    }
}
