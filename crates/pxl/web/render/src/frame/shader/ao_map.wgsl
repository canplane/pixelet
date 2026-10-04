// The screen-space occlusion, read where shade.wgsl weighs the light: it dims
// what lights a surface, never the air in front of it.

@group(0) @binding(7) var ao_map: texture_2d<f32>;

fn screen_open(frag: vec2f) -> f32 {
  if (frame_screen_ao() < 0.5) {
    return 1.0;
  }
  return textureLoad(ao_map, vec2i(frag), 0).r;
}
