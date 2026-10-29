// Decor instances too small on screen for their model, drawn as quads turned
// to the eye about the world's up, each instance read from the decor's
// instances (instance.wgsl).

@group(2) @binding(0) var sprites: texture_2d_array<f32>;
@group(2) @binding(1) var sprite_sampler: sampler;
@group(2) @binding(2) var<uniform> fits: array<vec4f, 64>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @builtin(vertex_index) k: u32,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) normal: vec3f,
  @location(2) world_pos: vec3f,
  @location(3) through: f32,
  @location(4) scattered: vec3f,
  @location(5) @interpolate(flat) fill: vec3f,
  @location(6) @interpolate(flat) sun: vec3f,
  @location(7) @interpolate(flat) other: vec3f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let i = instances[draw_idx() + v.i];
  var o: VertexOut;
  let base = vec3f(lane(0u, i), lane(1u, i), lane(2u, i)) + draw_translation();
  let scale = lane(4u, i);
  let fit = fits[draw_layer()];
  let corner = vec2f(select(-1.0, 1.0, (v.k & 1u) != 0u), select(-1.0, 1.0, (v.k & 2u) != 0u));
  let up = vec3f(0.0, 0.0, 1.0);
  let to_eye = frame_camera_pos() - base;
  let flat = vec3f(to_eye.xy, 0.0);
  let facing = select(vec3f(0.0, -1.0, 0.0), normalize(flat), dot(flat, flat) > 1e-8);
  let side = cross(up, facing);
  let local = side * (fit.x + corner.x * fit.z) * scale + up * (fit.y + corner.y * fit.z) * scale;
  let world = base + local;
  o.clip = frame_view_proj() * vec4f(world, 1.0);
  o.uv = vec2f(0.5 + 0.5 * corner.x, 0.5 - 0.5 * corner.y);
  o.normal = facing;
  o.world_pos = world;
  let seen = seen_from_eye(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let sprite = textureSample(sprites, sprite_sampler, i.uv, draw_layer());
  if (sprite.a < 0.5) {
    discard;
  }
  let n = normalize(i.normal);
  let color = shade(sprite.rgb, n, i.world_pos, sun_lit(i.world_pos, n), screen_open(i.clip.xy), Seen(i.through, i.scattered, Light(i.sun, i.other, i.fill)));
  return vec4f(tone(color), draw_opacity());
}
