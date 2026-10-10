// pxl-kernel/param/scene.rs

//! What is drawn beside the chunks, and how it is lit.

// ---- lighting ----

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all, on the shader's linear falloff: how much a rim brightens a face at
/// its edge, as a share of its own light — the day's rim of 0.7 of the sky's
/// light, the sky a fifth of a day's light on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// Edge shading on a cube's silhouette: its darkening, and its width in CSS
/// pixels, one line drawn round every cube alike, near or far, large or small.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 2.0;

/// Corner occlusion's darkening, written where the setting asks for it; the
/// shader cannot see a setting.
pub const AO_STRENGTH: f64 = 0.2;
