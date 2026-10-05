// The eye the frame is shown through: the exposure pass's one texel, its
// exposure in stops (an opened night eye is past a half float's range) and
// how far under noon it shows the land.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;

fn exposure() -> f32 {
  return exp2(textureLoad(exposure_texel, vec2i(0, 0), 0).r);
}

// From day, none, to night, one: how far the shown land has gone toward the night.
fn night_share() -> f32 {
  return clamp(-textureLoad(exposure_texel, vec2i(0, 0), 0).g / atmos_night_stops(), 0.0, 1.0);
}
