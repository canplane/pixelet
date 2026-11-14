// pxl-core/spec/gizmo.spec.rs
//
// GENERATED from spec/client/core/domain/scene/gizmo/gizmo.json5 (xpute-gen data.rs)
//
// What each line of the world gizmo is drawn in, so the lines and the
// UI's legend of them read one look: the frame's marks about the pivot
// (domain/scene/gizmo/frame.rs) and the coverage's over the ground
// (domain/scene/gizmo/coverage.rs). Each rgb is sRGB's 0xrrggbb, with its
// opacity, and dashed where a dashed line draws it: a ring of the frame, a
// stretch of the coverage.

/// About the pivot, opaque.
pub mod frame {
    /// The pivot and the ground cell it floors to.
    pub mod pivot {
        pub const RGB: u32 = 0;
    }
    /// The level ring.
    pub mod level {
        pub const RGB: u32 = 0;
        pub const DASHED: bool = true;
    }
    /// The upright ring facing the screen, and the view's forward on it.
    pub mod forward {
        pub const RGB: u32 = 16711680;
        pub const DASHED: bool = true;
    }
    /// The upright ring through north, and north on it.
    pub mod north {
        pub const RGB: u32 = 65280;
        pub const DASHED: bool = true;
    }
    /// The point straight up from the pivot, where the two upright rings cross.
    pub mod up {
        pub const RGB: u32 = 255;
    }
}

/// Over the ground, in the order a stretch shared by several takes the last: a drawn piece faint under what marks something wrong.
pub mod coverage {
    pub mod drawn {
        pub const RGB: u32 = 16777215;
        pub const ALPHA: f32 = 0.4;
        pub const DASHED: bool = false;
    }
    /// A drawn piece read from the floor's tile alone.
    pub mod drawn_from_floor {
        pub const RGB: u32 = 65535;
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = false;
    }
    /// A goal piece with a tile on its line, its brick not yet filled.
    pub mod unbuilt {
        pub const RGB: u32 = 16776960;
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    /// A goal piece only the floor's tile can build.
    pub mod floor_only {
        pub const RGB: u32 = 16711935;
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    /// A goal piece with no tile on its line.
    pub mod no_tile {
        pub const RGB: u32 = 16711680;
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    /// A drawn piece's tile.
    pub mod tile {
        pub const RGB: u32 = 16749824;
        pub const ALPHA: f32 = 0.9;
        pub const DASHED: bool = false;
    }
    /// A drawn piece's shard, the unit the world's data is kept in.
    pub mod shard {
        pub const RGB: u32 = 8191868;
        pub const ALPHA: f32 = 1.0;
        pub const DASHED: bool = false;
    }
}
