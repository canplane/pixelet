// The beacon lit as a character is (shade.wgsl), a metal that glows its color,
// placed as location_beacon_place.wgsl lays it.

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
  @location(1) world_pos: vec3f,
  @location(2) transmittance: f32,
  @location(3) scattered: vec3f,
  @location(4) @interpolate(flat) fill: vec3f,
  @location(5) @interpolate(flat) sun: vec3f,
  @location(6) @interpolate(flat) other: vec3f,
  // The draw's, which the fragment reads no draw for.
  @location(7) @interpolate(flat) night: f32,
};

@vertex
fn vs(@location(0) pos: vec3f, @location(1) normal: vec3f, @builtin(instance_index) instance: u32) -> VertexOut {
  draw_at(instance);
  var o: VertexOut;
  o.night = location_night();
  let world = beacon_world(pos);
  o.clip = jittered(frame_view_proj() * vec4f(world, 1.0));
  o.normal = spin_z(normal, location_spin());
  o.world_pos = world;
  let seen = seen_from_eye(world);
  o.transmittance = seen.transmittance;
  o.scattered = seen.scattered;
  let l = light_of(world);
  o.sun = l.sun;
  o.other = l.other;
  o.fill = l.fill;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let color = select(LOCATION_BEACON_DAY, LOCATION_BEACON_NIGHT, i.night > 0.5);
  let glow = select(LOCATION_BEACON_DAY_GLOW, LOCATION_BEACON_NIGHT_GLOW, i.night > 0.5);
  let n = normalize(i.normal);
  let seen = Air(i.transmittance, i.scattered);
  let lit = shade_metal(color, LOCATION_BEACON_METALNESS, LOCATION_BEACON_ROUGHNESS, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, seen, Light(i.sun, i.other, i.fill));
  return vec4f(tone(lit + color * glow * seen.transmittance), 1.0);
}
