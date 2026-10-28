// pxl-render/frame/shader/screen_finish.wgsl

// The world's color, toned and blended linear by its passes, encoded onto
// the canvas once (finish.wgsl `finish`), a texel a pixel, its alpha kept for
// a surface that shows through where nothing was drawn.

@group(0) @binding(1) var world: texture_2d<f32>;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
  return o;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let c = textureLoad(world, vec2i(v.clip.xy), 0);
  return vec4f(finish(c.rgb, v.clip.xy), c.a);
}
