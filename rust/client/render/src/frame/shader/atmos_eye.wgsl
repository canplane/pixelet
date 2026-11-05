// The exposure pass's texel, in stops: an opened night eye is past a half float's range.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;

// The texel as this invocation read it, once: every light and the sky take it.
var<private> eye_texel: vec4f;
var<private> eye_texel_read: bool;

fn eye_texel_word() -> vec4f {
  if (!eye_texel_read) {
    eye_texel = textureLoad(exposure_texel, vec2i(0, 0), 0);
    eye_texel_read = true;
  }
  return eye_texel;
}

fn exposure() -> f32 {
  return exp2(eye_texel_word().r);
}

// The table's white, as the exposure pass wrote it beside the stops.
fn atmos_white() -> vec3f {
  return eye_texel_word().gba;
}
