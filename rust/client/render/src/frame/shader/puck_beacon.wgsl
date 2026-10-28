struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
};

@vertex
fn vs(@location(0) position: vec3f, @location(1) normal: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(spin_z(position, draw_fourth()) + draw_translation(), 1.0);
  o.normal = spin_z(normal, draw_fourth());
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let color = select(PUCK_BEACON_DAY, PUCK_BEACON_NIGHT, draw_night() > 0.5);
  let glow = select(PUCK_BEACON_DAY_GLOW, PUCK_BEACON_NIGHT_GLOW, draw_night() > 0.5);
  let n = normalize(i.normal);
  // Each light through the air over the land it stands on.
  let r = land_radius();
  let sun = atmos_white() * sun_transmittance(r, atmos_sun_dir().z);
  let other = atmos_moon_color() * atmos_white() * sun_transmittance(r, atmos_moon_dir().z);
  let lit = color * (sun * max(dot(n, atmos_sun_dir()), 0.0) + other * max(dot(n, atmos_moon_dir()), 0.0)) / PI * PUCK_BEACON_LIT;
  return vec4f(finish(lit + color * glow, i.clip.xy), 1.0);
}
