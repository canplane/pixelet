// Decor's impostor sprite: a variant's model drawn once from the side into
// its layer of the sprite atlas (gpu/pass/impostor.rs), its color as its skin
// gives it and its coverage as alpha, which the layer is cleared to none of.
// glTF's Y is up and its +Z faces the viewer; the fit puts the model's side
// in the layer, its foot at the bottom, and its depth between the planes.

// The fit: the model's center across and up, its half side, its half depth.
@group(0) @binding(0) var<uniform> fit_words: array<vec4f, 1>;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VertexIn {
  @location(0) position: vec3f,
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
  o.clip = vec4f((v.position.x - f.x) / f.z, (v.position.y - f.y) / f.z, 0.5 - 0.5 * v.position.z / f.w, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(textureSample(skin, skin_sampler, i.uv).rgb, 1.0);
}
