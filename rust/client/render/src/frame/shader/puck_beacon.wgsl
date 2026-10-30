// The beacon lit as an actor is (shade.wgsl), a metal that glows its color.

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
  @location(1) world_pos: vec3f,
  @location(2) transmittance: f32,
  @location(3) scattered: vec3f,
  @location(4) @interpolate(flat) fill: vec3f,
  @location(5) @interpolate(flat) sun: vec3f,
  @location(6) @interpolate(flat) other: vec3f,
};

@vertex
fn vs(@location(0) pos: vec3f, @location(1) normal: vec3f) -> VertexOut {
  var o: VertexOut;
  let world = spin_z(pos, draw_fourth()) + draw_translation();
  o.clip = jittered(frame_view_proj() * vec4f(world, 1.0));
  o.normal = spin_z(normal, draw_fourth());
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
  let color = select(PUCK_BEACON_DAY, PUCK_BEACON_NIGHT, draw_night() > 0.5);
  let glow = select(PUCK_BEACON_DAY_GLOW, PUCK_BEACON_NIGHT_GLOW, draw_night() > 0.5);
  let n = normalize(i.normal);
  let seen = Seen(i.transmittance, i.scattered, Light(i.sun, i.other, i.fill));
  let lit = shade_metal(color, PUCK_BEACON_METALNESS, PUCK_BEACON_ROUGHNESS, n, i.world_pos, shadow_visibility(i.world_pos, n), screen_ao(i.clip.xy), seen);
  return vec4f(tone(lit + color * glow * seen.transmittance), 1.0);
}
