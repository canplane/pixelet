// pxl-render/frame/shader/screen_show.wgsl

// Shows the finished picture, which is kept at the canvas's size, on the canvas, a texel a
// pixel (pass/finish.rs). The canvas's texture is a new one each frame, so a frame not drawn
// whole shows what an earlier frame finished wherever it does not finish the canvas again.

@group(0) @binding(1) var shown: texture_2d<f32>;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = screen_triangle_corner(vi);
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  return o;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return textureLoad(shown, vec2i(v.clip.xy), 0);
}
