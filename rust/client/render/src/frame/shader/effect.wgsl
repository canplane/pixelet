// pxl-render/frame/shader/effect.wgsl

// The share of the effects that shows on a drawn surface `d` units from the eye.
// The effects are a surface's lines, the occlusion of its corners and of the
// screen, the shadows and the rim. They all reach to one distance (pxl-spec
// space/layer/lattice.rs `LEAF_REACH`) and fade alike before it.
fn effect_share(d: f32) -> f32 {
  return 1.0 - smoothstep(EFFECT_FADE_START * LEAF_REACH, LEAF_REACH, d);
}
