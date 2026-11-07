// pxl-render/frame/shader/effect.wgsl

// How much of the effects on a drawn surface `d` units from the eye shows: its
// lines, its corners' and the screen's occlusion, the shadows and the rim, all to one
// boundary (pxl-spec space/layer/lattice.rs `EFFECT_REACH`), faded alike
// before it.
fn effect_share(d: f32) -> f32 {
  return 1.0 - smoothstep(EFFECT_FADE_START * EFFECT_REACH, EFFECT_REACH, d);
}
