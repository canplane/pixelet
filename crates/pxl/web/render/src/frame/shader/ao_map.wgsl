// The AO byte applied as N8AO composited it, before the grade.

@group(0) @binding(7) var ao_map: texture_2d<f32>;

// #1a1006 in linear light: N8AO's `color`.
const AO_COLOR: vec3f = vec3f(0.01033, 0.00518, 0.00182);

fn screen_ao(color: vec3f, frag: vec2f) -> vec3f {
  if (frame_screen_ao() < 0.5) {
    return color;
  }
  let occlusion = textureLoad(ao_map, vec2i(frag), 0).r;
  return mix(color, AO_COLOR * color, 1.0 - occlusion);
}
