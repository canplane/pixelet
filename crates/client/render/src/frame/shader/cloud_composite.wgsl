// The clouds marched at half the screen's size, laid over it at its own: the
// depth test here keeps a ridge's edge as sharp as the world drew it.

@group(0) @binding(0) var cloud_half: texture_2d<f32>;
@group(0) @binding(1) var cloud_half_sampler: sampler;

struct CompositeOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> CompositeOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: CompositeOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  return o;
}

// The march writes its color times its opacity, so the filter blends a cloud's
// edge with nothing rather than with black.
@fragment
fn fs(v: CompositeOut) -> @location(0) vec4f {
  let c = textureSampleLevel(cloud_half, cloud_half_sampler, v.uv, 0.0);
  if (c.a <= 0.002) {
    discard;
  }
  return vec4f(c.rgb / c.a, c.a);
}
