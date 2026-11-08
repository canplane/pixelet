// WebGPU has no generateMipmap, so each level is drawn in a pass of its own. A pixel takes one
// linear sample at the middle of the 2×2 texels above it, which averages them.

@group(0) @binding(0) var above: texture_2d<f32>;
@group(0) @binding(1) var above_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
  o.uv = vec2f(p[vi].x * 0.5 + 0.5, 0.5 - p[vi].y * 0.5);
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return textureSample(above, above_sampler, i.uv);
}
