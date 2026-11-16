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
  let p = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return textureSample(above, above_sampler, i.uv);
}
