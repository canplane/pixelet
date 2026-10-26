// The screen's depth alone, for the occlusion's prepass (gpu/ao.rs): each
// pixel's face where its ray met it (screen_ray.wgsl `pixel_ray`), the
// depth the lit pass draws it at less the step it lays it back by, so the
// ground darkens where it folds as what is drawn on it does.

@fragment
fn fs(v: VertexOut) -> @builtin(frag_depth) f32 {
  let p = pixel_ray(v.clip.xy);
  if (!p.met || (eye_word(EYE_STEPS) & EYE_SHOW_STEPS) != 0u) {
    discard;
  }
  let clip = frame_view_proj() * vec4f(p.h.at, 1.0);
  return clip.z / clip.w;
}
