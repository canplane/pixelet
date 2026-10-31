// Decor instances too small on screen for their model, drawn as quads placed
// as impostor_place.wgsl lays them.

@group(2) @binding(0) var impostors: texture_2d_array<f32>;
@group(2) @binding(1) var impostor_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) normal: vec3f,
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
  let world = p.world;
  var o: VertexOut;
  o.clip = jittered(frame_view_proj() * vec4f(world, 1.0));
  o.uv = vec2f(0.5 + 0.5 * p.corner.x, 0.5 - 0.5 * p.corner.y);
  o.normal = p.facing;
  o.world_pos = world;
  let seen = seen_from_eye(world);
  o.transmittance = seen.transmittance;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let texel = textureSample(impostors, impostor_sampler, i.uv, draw_layer());
  if (texel.a < 0.5) {
    discard;
  }
  let n = normalize(i.normal);
  let color = shade(texel.rgb, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, Seen(i.transmittance, i.scattered, Light(i.sun, i.other, i.fill)));
  return vec4f(tone(color), draw_opacity());
}
