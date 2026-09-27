// The screen-space occlusion as a lit pass reads it: the byte the upsample
// wrote for this pixel, applied as N8AO composited it — the color pulled
// toward itself times a dark warm tint by the occlusion — before the
// grade, where the post stack had it. A blank map is bound while the
// screen-space mode is off, and the uniform says not to read it.

@group(0) @binding(7) var ao_map: texture_2d<f32>;

// #1a1006 in linear light: N8AO's `color`, a contact shadow's warm dark
// rather than black.
const AO_COLOR: vec3f = vec3f(0.01033, 0.00518, 0.00182);

fn screen_ao(color: vec3f, frag: vec2f) -> vec3f {
  if (frame().screen_ao < 0.5) {
    return color;
  }
  let occlusion = textureLoad(ao_map, vec2i(frag), 0).r;
  return mix(color, AO_COLOR * color, 1.0 - occlusion);
}
