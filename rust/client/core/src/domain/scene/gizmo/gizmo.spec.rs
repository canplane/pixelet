// pxl-core/domain/scene/gizmo/gizmo.spec.rs
//
// GENERATED from spec/client/core/domain/scene/gizmo/gizmo.json (xpute-gen data.rs)
//
// What each line of the world gizmo is drawn in, so the lines and the
// UI's legend of them read one look: the frame's marks about the pivot
// (domain/scene/gizmo/frame.rs) and the coverage's over the ground
// (domain/scene/gizmo/coverage.rs). Each rgb is sRGB's #rrggbb, with its
// opacity, and dashed where a dashed line draws it: a ring of the frame, a
// stretch of the coverage.

// A table is generated whole, whether or not this crate calls an entry.
#![allow(dead_code, reason = "generated from the spec")]

pub mod frame {
    pub mod pivot {
        pub const RGB: &str = "#000000";
    }
    pub mod level {
        pub const RGB: &str = "#000000";
        pub const DASHED: bool = true;
    }
    pub mod forward {
        pub const RGB: &str = "#ff0000";
        pub const DASHED: bool = true;
    }
    pub mod north {
        pub const RGB: &str = "#00ff00";
        pub const DASHED: bool = true;
    }
    pub mod up {
        pub const RGB: &str = "#0000ff";
    }
}

pub mod coverage {
    pub mod drawn {
        pub const RGB: &str = "#ffffff";
        pub const ALPHA: f32 = 0.4;
        pub const DASHED: bool = false;
    }
    pub mod drawn_from_floor {
        pub const RGB: &str = "#00ffff";
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = false;
    }
    pub mod unbuilt {
        pub const RGB: &str = "#ffff00";
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    pub mod floor_only {
        pub const RGB: &str = "#ff00ff";
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    pub mod no_tile {
        pub const RGB: &str = "#ff0000";
        pub const ALPHA: f32 = 0.6;
        pub const DASHED: bool = true;
    }
    pub mod tile {
        pub const RGB: &str = "#ff9500";
        pub const ALPHA: f32 = 0.9;
        pub const DASHED: bool = false;
    }
    pub mod shard {
        pub const RGB: &str = "#7cff7c";
        pub const ALPHA: f32 = 1.0;
        pub const DASHED: bool = false;
    }
}
