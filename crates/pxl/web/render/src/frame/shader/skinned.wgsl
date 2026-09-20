// Actors: the module's skinned draws (model.rs) against each model's mesh
// block and each actor's joint palette, both as the module wrote them. A
// vertex is skinned by its four joints and weights out of the palette,
// scaled to the model's height, turned from glTF's Y-up to this world's
// Z-up and by the actor's heading about Z, and placed; the fragment is
// decor's recipe.

struct Draw {
  translation: vec3f,
  rotation: f32,
  opacity: f32,
  scale: f32,
  _p0: f32,
  _p1: f32,
};

@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
@group(3) @binding(0) var<storage, read> palette: array<mat4x4f>;

struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let m = palette[v.joints.x] * v.weights.x + palette[v.joints.y] * v.weights.y + palette[v.joints.z] * v.weights.z + palette[v.joints.w] * v.weights.w;
  let p = (m * vec4f(v.position, 1.0)).xyz * draw.scale;
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  let world = place_up(p, draw.rotation) + draw.translation;
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place_up(n, draw.rotation);
  o.world_pos = world;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = screen_ao(shade(albedo, n, i.world_pos, 1.0 / i.clip.w, sun_lit(i.world_pos, n), vec3f(1.0)), i.clip.xy);
  return vec4f(finish(color, i.clip.xy), draw.opacity);
}
