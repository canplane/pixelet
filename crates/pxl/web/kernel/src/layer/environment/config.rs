// pxl-kernel/layer/environment/config.rs

/// The fog band's and far clip's tuning, kept as ratios of FOG_FAR: the band
/// fades the ground into the sky's own light where the nearer of the horizon
/// and the drawn levels ends it, so the edge is no seam.
pub mod render_cfg {
    /// 1.5 × FOG_FAR, so the band completes inside the frustum.
    pub const FAR_CLIP: f64 = 9216.0;
    /// 0.75 × FOG_FAR. Narrow: it only hides the edge; the haze is the air's.
    pub const FOG_NEAR: f64 = 4608.0;
    /// Five levels from the leaves draw step 2^5, which the LOD rule wants out
    /// to 192 · 2^5.
    pub const FOG_FAR: f64 = 6144.0;
}

/// One brightness floor for every lit surface, so terrain and objects cannot
/// dim apart.
pub mod light_cfg {
    /// Judged on a phone: a floor dark enough to read as night on a monitor was
    /// too dark on a phone. Raising it alone flattens the shading; it moves with
    /// the moon's light.
    pub const FLOOR: f64 = 0.6;
}

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all; the linear scale on the shader's falloff.
pub mod rim_cfg {
    pub const INTENSITY: f64 = 0.1;
}
