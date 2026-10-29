// Decor: instanced variant models, each instance read from the decor's
// instances (instance.wgsl).

@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) through: f32,
  @location(4) scattered: vec3f,
  @location(5) @interpolate(flat) fill: vec3f,
  @location(6) @interpolate(flat) sun: vec3f,
  @location(7) @interpolate(flat) other: vec3f,
};

// One function, so the depth passes place a vertex where the lit pass does.
struct Placed {
  world: vec3f,
  spin: f32,
};

fn placed(v: VertexIn) -> Placed {
  let i = instances[draw_idx() + v.i];
  let at = vec3f(lane(0u, i), lane(1u, i), lane(2u, i));
  var p: Placed;
  p.spin = lane(3u, i);
  let scale = lane(4u, i);
  p.world = place_up(v.pos * scale, p.spin) + at + draw_translation();
  return p;
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let p = placed(v);
  var o: VertexOut;
  o.clip = jittered(frame_view_proj() * vec4f(p.world, 1.0));
  o.uv = v.uv;
  o.normal = place_up(v.normal, p.spin);
  o.world_pos = p.world;
  let seen = seen_from_eye(p.world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(placed(v).world, 1.0));
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, sun_lit(i.world_pos, n), screen_open(i.clip.xy), Seen(i.through, i.scattered, Light(i.sun, i.other, i.fill)));
  return vec4f(finish(color, i.clip.xy), draw_opacity());
}
