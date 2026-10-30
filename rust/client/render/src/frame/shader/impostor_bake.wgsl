// Draws a variant's model once from the side into its atlas layer, coverage
// as alpha. glTF is Y-up with +Z toward the viewer.

// Center across and up, half side, half depth.
@group(0) @binding(0) var<uniform> fit_words: array<vec4f, 1>;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VertexIn {
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let f = fit_words[0];
  var o: VertexOut;
  o.clip = vec4f((v.pos.x - f.x) / f.z, (v.pos.y - f.y) / f.z, 0.5 - 0.5 * v.pos.z / f.w, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(textureSample(skin, skin_sampler, i.uv).rgb, 1.0);
}
