// pxl-kernel/param/scene.rs

//! What is drawn beside the chunks, and how it is lit.

// ---- lighting ----

/// Fresnel rim highlight, applied to everything lit since one shader lights
/// it all, on the shader's linear falloff: how much a rim brightens a face at
/// its edge, as a share of its own light — the day's rim of 0.7 of the sky's
/// light, the sky a fifth of a day's light on a face of albedo 0.3.
pub const RIM_INTENSITY: f64 = 0.5;

/// Edge shading where a cube's edge meets the air: its darkening, and its
/// width in CSS pixels, one line alike near or far, large or small.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 2.0;

/// Corner occlusion's darkening at its deepest, written where the setting
/// asks for it; the shader cannot see a setting.
pub const AO_STRENGTH: f64 = 0.2;

// ---- places ----

/// What reading one chunk's markers and places costs, measured: 3.75 ms over
/// 63 chunks at E120, places on, a ray down a place (2026-10-17).
pub const PLACE_CHUNK_MS: f64 = 0.06;
/// The share of a frame reading the chunks a move enters may take.
pub const PLACE_READ_MS: f64 = 1.0;
/// The chunks a frame reads, nearest the eye first; the rest a later frame.
pub const PLACE_CHUNKS_READ: usize = (PLACE_READ_MS / PLACE_CHUNK_MS) as usize;
