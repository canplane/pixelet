// Grade, vignette, encode, in that order.

fn vignette(frag: vec2f) -> f32 {
  return vignette_of(frag, frame_viewport(), frame_vignette());
}

fn finish(color: vec3f, frag: vec2f) -> vec3f {
  return to_srgb(grade(color) * vignette(frag));
}
