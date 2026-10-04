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

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all; the linear scale on the shader's falloff.
pub mod rim_cfg {
    /// How much a rim brightens a face at its edge, as a share of its own light:
    /// the day's rim of 0.7 of the sky's light, the sky a fifth of a day's light
    /// on a face of albedo 0.3.
    pub const INTENSITY: f64 = 0.5;
}

/// Edge shading on a cube's silhouette: its darkening and its width as a share of a face.
pub mod edge_cfg {
    pub const STRENGTH: f64 = 0.3;
    pub const WIDTH: f64 = 0.03;
}

/// Corner occlusion's darkening, written where the setting asks for it; the
/// shader cannot see a setting.
pub mod ao_cfg {
    pub const STRENGTH: f64 = 0.2;
}
