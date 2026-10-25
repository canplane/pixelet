// The exposure pass's texel, in stops: an opened night eye is past a half float's range.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;

fn exposure() -> f32 {
  return exp2(textureLoad(exposure_texel, vec2i(0, 0), 0).r);
}

// The table's white, as the exposure pass wrote it beside the stops.
fn atmos_white() -> vec3f {
  return textureLoad(exposure_texel, vec2i(0, 0), 0).gba;
}
