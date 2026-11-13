// pxl-render/frame/shader/debug_look.spec.rs
//
// GENERATED from spec/client/render/frame/shader/debug_look.json (xpute-gen data.rs)
//
// What the renderer's two debug views are drawn in, so the screen and the
// UI's legend of them read one look: the camera gizmo's grid and the tints
// of the cells the rays stop on (camera_grid.wgsl, screen_shade.wgsl
// `cell_share`), and the ray steps' ramp (screen_shade.wgsl). Each #rrggbb in
// sRGB.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    reason = "generated from the spec"
)]

/// The camera grid's lines.
pub mod grid {
    pub const rgb: &str = "#ffffff";
    pub const alpha: f32 = 0.25;
}

/// The tint on a cell the rays stop on, by the solid angle its face shows the
/// eye over a grid cell's, in this order, each up to its `most`.
pub mod share {
    pub mod finer {
        pub const rgb: &str = "#5995ff";
        pub const most: f32 = 1.0;
    }
    pub mod within {
        pub const rgb: &str = "#59f37c";
        pub const most: f32 = 4.0;
    }
    pub mod coarser {
        pub const rgb: &str = "#ff6c59";
    }
}

/// The steps a pixel's ray took on its way from the eye, on a log scale.
pub mod steps {
    pub const most: f32 = 1024.0;
    pub const none: &str = "#808080";
    /// Google's Turbo, dark blue at none through green and yellow to dark red at
    /// `most`, at even stops a pixel's color runs straight between.
    pub const ramp: &[&str] = &[
        "#23171b", "#493eaf", "#446aee", "#3295f7", "#26bde1", "#29ddbb", "#40f392", "#66fd6d",
        "#96fa50", "#c6eb3b", "#eed02d", "#ffab24", "#ff801d", "#ee5415", "#c92d0c", "#a11202",
        "#900d00",
    ];
}
