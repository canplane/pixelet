// pxl-engine/param.rs

//! wgsl-port: the values this crate sets for what the world is and how it is observed and
//! drawn. Each value is one literal with its reason beside it, in a module for its subject.
//! What follows from them is computed by name where it is used.

pub mod beam {
    /// The samples on a side of a camera grid cell. They form the lattice of stops of which a
    /// pixel takes the nearest (beam.rs `SAMPLE_NODE`).
    pub const SAMPLE: u32 = 4;

    /// Samples past the screen on each side, so a cell seen at the screen's edge
    /// is one a sample stopped at.
    pub const NEAR: u32 = 2;

    /// The margin a beam ray's radius takes over a cube's, `√6 · s`.
    pub(crate) const RADIUS_SLACK: f32 = 1.01;
}

pub mod raster {
    /// How far a face is grown in its own plane, in drawn pixels at each corner's distance, so
    /// that a pixel whose middle lies on the face's edge is not lost to the rasterizer's rounding.
    /// The face stays in its plane, so a pixel's depth is still where its ray meets the face. A
    /// pixel that only the growth covers fails the fragment's exact test (pxl-render raster.wgsl
    /// `fs`), so the growth costs fragments but changes no visible sample.
    pub const GROWTH: f32 = 0.25;
}

pub mod light {
    /// The slope by which a caster's depth in the shadow map is pushed back, like a rasterizer's
    /// polygon offset: the caster's greatest slope, in depth a texel (light.rs `slope`), times
    /// this. The raster casters' depth pipelines apply it (pxl-render record.rs `depth_only`) and
    /// the world's cast adds it (pxl-render light.wgsl `light_cs`), so the two kinds of caster
    /// shadow alike. A receiver compares each tap at its own plane's depth there (pxl-render
    /// shadow.wgsl), so the push need cover only the texels a tap filters, one texel each way
    /// along the steeper axis; the receiver's one texel of normal offset covers the other axis.
    /// Comparing each tap at the receiver's own depth instead would make a step one unit high
    /// shadow the ground below it in stripes a texel apart, because the taps up the slope see
    /// past the step.
    pub const SLOPE_BIAS: f32 = 1.0;

    /// The shadow map's slots from one sample to the next, across and down (light.rs
    /// `samples_at`).
    pub const SAMPLE: u32 = 4;

    /// The spacing of the receiver's 3×3 taps in the shadow map, in texels (pxl-render
    /// shadow.wgsl `shadow_visibility`). The filtered edge then spans about four texels, half a
    /// unit where a texel is 1/8 u. Taps one texel apart would cut each cell's shadow out as a
    /// hard square.
    pub const SPREAD: f32 = 1.5;
}
