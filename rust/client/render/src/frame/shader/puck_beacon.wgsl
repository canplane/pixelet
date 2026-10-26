// #ff1493 and #00f2ff, in linear light.
const BEACON_DAY: vec3f = vec3f(1.0, 0.006995, 0.291771);
const BEACON_NIGHT: vec3f = vec3f(0.0, 0.887923, 1.0);

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
  let color = select(BEACON_DAY, BEACON_NIGHT, draw_night() > 0.5);
  let glow = select(0.25, 1.0, draw_night() > 0.5);
  let n = normalize(i.normal);
  // Each light through the air over the land it stands on.
  let r = land_radius();
  let sun = atmos_white() * sun_transmittance(r, atmos_sun_dir().z);
  let other = atmos_moon_color() * atmos_white() * sun_transmittance(r, atmos_moon_dir().z);
  let lit = color * (sun * max(dot(n, atmos_sun_dir()), 0.0) + other * max(dot(n, atmos_moon_dir()), 0.0)) / PI * 0.2;
  return vec4f(finish(lit + color * glow, i.clip.xy), 1.0);
}
