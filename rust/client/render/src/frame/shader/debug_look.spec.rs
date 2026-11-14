// pxl-render/frame/shader/debug_look.spec.rs
//
// GENERATED from spec/client/render/frame/shader/debug_look.json5 (xpute-gen data.rs)
//
// What the renderer's two debug views are drawn in, so the screen and the
// UI's legend of them read one look: the camera gizmo's grid and the tints
// of the cells the rays stop on (camera_grid.wgsl, screen_shade.wgsl
// `cell_share`), and the ray steps' ramp (screen_shade.wgsl). Each 0xrrggbb in
// sRGB.

/// The camera grid's lines.
pub mod grid {
    pub const RGB: u32 = 16777215;
    pub const ALPHA: f32 = 0.25;
}

/// The tint on a cell the rays stop on, by the solid angle its face shows the
/// eye over a grid cell's, in this order, each up to its `most`.
pub mod share {
    /// Within one grid cell.
    pub mod finer {
        pub const RGB: u32 = 5871103;
        pub const MOST: f32 = 1.0;
    }
    /// Within one to four.
    pub mod within {
        pub const RGB: u32 = 5895036;
        pub const MOST: f32 = 4.0;
    }
    /// Past four.
    pub mod coarser {
        pub const RGB: u32 = 16739417;
    }
}

/// The steps a pixel's ray took on its way from the eye, on a log scale.
pub mod steps {
    /// The steps the ramp runs up to, the same on every screen so two compare.
    pub const MOST: f32 = 1024.0;
    /// A pixel that walked nothing, apart from the ramp's every color.
    pub const NONE: u32 = 8421504;
    /// Google's Turbo, dark blue at none through green and yellow to dark red at
    /// `most`, at even stops a pixel's color runs straight between.
    pub const RAMP: [u32; 17] = [
        2299675, 4800175, 4483822, 3315191, 2538977, 2743739, 4256658, 6749549, 9894480, 13036347,
        15650861, 16755492, 16744477, 15619093, 13184268, 10555906, 9440512,
    ];
}
