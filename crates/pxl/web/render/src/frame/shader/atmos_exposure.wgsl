// Eye adaptation to the light the land under the eye is lit by: an up-facing
// face of albedo one there, before the eye, against the same face at sea level
// under a sun overhead. The land is lit by that same light, so how bright it
// shows is set here, wherever the eye is and whatever it looks at: a fall from
// noon shows as SHOWN_FALL of it, down to NIGHT_STOPS under, and the eye takes
// back the rest. Written in stops, which an opened night eye needs.

fn noon() -> f32 {
  let r = atmos_bottom_radius();
  return dot((sun_transmittance(r, 1.0) + sky_ground(r, 1.0).rgb) * atmos_white(), LUMA) / PI;
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let up = frame_up();
  let land = dot(surface_light(up, 1.0, light_unexposed(land_radius(), up)), LUMA);
  let stops = log2(max(land, 1e-30) / noon());
  let shown = clamp(stops * atmos_shown_fall(), -atmos_night_stops(), 0.0);
  return vec4f(log2(atmos_exposure()) + shown - stops, shown, 0.0, 1.0);
}
