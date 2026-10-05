// pxl-kernel/layer/environment/config.rs

pub mod render_cfg {
    /// The far clip against the reach: past it, so ground at the reach is whole.
    pub const FAR_CLIP: f64 = 1.5;
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
