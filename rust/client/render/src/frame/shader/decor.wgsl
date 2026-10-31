// Decor: instanced variant models, placed as decor_place.wgsl lays them.

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) transmittance: f32,
  @location(4) scattered: vec3f,
  @location(5) @interpolate(flat) fill: vec3f,
  @location(6) @interpolate(flat) sun: vec3f,
  @location(7) @interpolate(flat) other: vec3f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let p = placed(v);
  var o: VertexOut;
  o.clip = jittered(frame_view_proj() * vec4f(p.world, 1.0));
  o.uv = v.uv;
  o.normal = place_up(v.normal, p.spin);
  o.world_pos = p.world;
  let seen = seen_from_eye(p.world);
  o.transmittance = seen.transmittance;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, Seen(i.transmittance, i.scattered, Light(i.sun, i.other, i.fill)));
  return vec4f(tone(color), draw_opacity());
}
