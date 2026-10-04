// The eye the frame is shown through: the exposure pass's one texel.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;

fn exposure() -> f32 {
  return textureLoad(exposure_texel, vec2i(0, 0), 0).r;
}
