// Draws the beacon as a metal that glows in its color, lit as a character is
// (shade.wgsl). location_beacon_place.wgsl places it.

@vertex
fn vs(@location(0) pos: vec3f, @location(1) normal: vec3f, @builtin(instance_index) instance: u32) -> VertexOut {
  draw_at(instance);
  return lit_vertex(beacon_world(pos), vec2f(0.0), spin_z(normal, location_spin()), location_night());
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let color = select(LOCATION_BEACON_DAY, LOCATION_BEACON_NIGHT, i.draw > 0.5);
  let glow = select(LOCATION_BEACON_DAY_GLOW, LOCATION_BEACON_NIGHT_GLOW, i.draw > 0.5);
  let n = normalize(i.normal);
  let seen = lit_air(i);
  let lit = shade_metal(color, LOCATION_BEACON_METALNESS, LOCATION_BEACON_ROUGHNESS, n, i.world_pos, shadow_visibility(i.world_pos, n), 1.0, seen, lit_light(i));
  return vec4f(tone(lit + color * glow * seen.transmittance), 1.0);
}
