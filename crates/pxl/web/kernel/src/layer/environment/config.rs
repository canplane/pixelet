// pxl-kernel/layer/environment/config.rs

/// The band that hides where the ground ends, and the far clip, as shares
/// of the reach.
///
/// How far the ground is worth drawing is no longer a number here. It is
/// read off the world: the planet turns away at the horizon, and before
/// that the aerosol near the ground takes a surface's contrast over the
/// visual range, and the nearer of the two wins (environment/fog.rs,
/// issue/20260923-the-ground-runs-to-the-horizon.md). What the rings hold
/// is a second end — a window of levels sized for `.fixed` — and the band
/// ends the ground at whichever comes first, so ground fades out instead
/// of being cut off while it is still in view: into the sky's own light
/// along the same ray, which the sky pass draws past it (atmos_view.wgsl),
/// so the edge is no seam. The haze in front of the band is the air's
/// (look/atmosphere.rs, aerial perspective), not this band's.
///
/// The three values below are what the band and the planes were tuned at,
/// kept as the ratios they always were: the fade is FOG_NEAR/FOG_FAR of
/// the reach, the far plane FAR_CLIP/FOG_FAR of it, and the near plane
/// opens out as the reach grows past FOG_FAR so a zoom out is a change of
/// scale and nothing else.
///
/// What is drawn at which level is measured from the eye (world/pyramid/
/// domain.rs `Domain`), so the band ends the ground at the nearer of the
/// reach and how far the pyramid's top draws, and nothing the band leaves
/// visible is outside what the view draws: a root is any top-level chunk
/// nearer than that.
pub mod render_cfg {
    /// 1.5 × FOG_FAR: the far plane a reach of FOG_FAR asks for, so the band
    /// completes inside the frustum rather than being cut by it. Read as that
    /// ratio, not as a distance.
    pub const FAR_CLIP: f64 = 9216.0;
    /// 0.75 × FOG_FAR: where the fade starts, as a share of wherever the
    /// ground ends. Narrow, because it only hides the edge — the depth in
    /// front of it is the air's own haze.
    pub const FOG_NEAR: f64 = 4608.0;
    /// The reach the two above were tuned against, and what the near plane's
    /// scale is measured in: five levels from the leaves draw step 2^5, which
    /// the rule wants out to 192 · 2^5 (pxl-spec space/lod.rs's `level`).
    pub const FOG_FAR: f64 = 6144.0;
}

/// Minimum-brightness floor shared by every lit surface — terrain and
/// placed cells, decor, actors — through the one shading source
/// (crates/pxl/web/render/src/frame/shader/shade.wgsl, `frame.floor`, carried
/// in the theme through state.rs). One shared constant, so terrain
/// and objects cannot dim apart. Tune this one number to retune every
/// surface's night floor at once.
pub mod light_cfg {
    /// Judged on a real device, not a desktop monitor — a darker floor that read
    /// as convincing night on a monitor was simply too dark to see the world on
    /// a phone screen.
    ///
    /// Raising this alone flattens the scene, since it lifts every face by the
    /// same amount and erases the shading that carries depth. It moves together
    /// with the moon's directional light, which adds
    /// brightness that still differs face to face.
    pub const FLOOR: f64 = 0.6;
}

/// Fresnel/rim highlight, applied uniformly to everything lit — terrain,
/// placed objects, decor and actors alike, since one shading source lights
/// them (crates/pxl/web/render/src/frame/shader/shade.wgsl) — a thin bright edge on
/// faces that graze the view direction, the classic "toy block catching
/// light at its silhouette" cue. Originally object-only (meant to help
/// placed objects read as distinct from terrain cells sharing the same
/// block palette), but a shared render pipeline shouldn't treat one
/// geometry role differently from another for the same shader term, so it
/// applies everywhere rim_light is on. The falloff's exponent is the
/// shader's; this is the linear scale on it, the one that controls peak
/// brightness, written into the frame uniform.
pub mod rim_cfg {
    pub const INTENSITY: f64 = 0.1;
}
