// pxl-render/frame/shader/effect.wgsl

// How much of the effects on a drawn surface `d` units from the eye shows,
// `reach` their reach (pxl-kernel domain/scene/stage.rs `STAGE_AT::EFFECT_REACH`):
// its lines, its corners' and the screen's occlusion and the shadows, all
// to one boundary, faded alike before it.
fn effect_share(d: f32, reach: f32) -> f32 {
  return 1.0 - smoothstep(EFFECT_FADE_START * reach, reach, d);
}
