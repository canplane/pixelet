// pxl-render/frame/shader/screen_show.wgsl

// The finished picture kept at the canvas's size shown on the canvas, a
// texel a pixel (gpu/finish.rs): the canvas's texture is a new one each
// frame, so what a region frame did not finish again is shown from what an
// earlier frame did.

@group(0) @binding(1) var shown: texture_2d<f32>;

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
  return textureLoad(shown, vec2i(v.clip.xy), 0);
}
