// The eye, adapted to the light on the land under it: an up-facing face of
// albedo one, against the same at sea level under a sun overhead. A fall from
// noon shows as SHOWN_FALL of it down to NIGHT_STOPS under; the eye takes back
// the rest. In stops, which an opened night eye needs.

fn noon() -> f32 {
  let r = atmos_bottom_radius();
  return dot((sun_transmittance(r, 1.0) + sky_ground(r, 1.0).rgb) * atmos_white(), LUMA) / PI;
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let up = frame_up();
  let land = dot(surface_light(up, up, 1.0, light_unexposed(land_radius(), up)), LUMA);
  let stops = log2(max(land, 1e-30) / noon());
  let shown = clamp(stops * atmos_shown_fall(), -atmos_night_stops(), 0.0);
  return vec4f(log2(atmos_exposure()) + shown - stops, 0.0, 0.0, 1.0);
}
