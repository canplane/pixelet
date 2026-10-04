// Eye adaptation to what the view shows: the sky and the Earth past the
// terrain, unexposed, on a grid over the screen, their mean luminance read as
// the illuminance that lights the Earth's mean albedo to it, against a sun
// overhead at sea level (1 after white balance). The mean, not the log mean:
// a sunlit limb over a night Earth closes the eye as a camera's meter does.

const METER_TAPS: i32 = 8;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let fill = sky_fill();
  var sum = 0.0;
  for (var i = 0; i < METER_TAPS; i++) {
    for (var j = 0; j < METER_TAPS; j++) {
      let ndc = (vec2f(f32(i), f32(j)) + 0.5) / f32(METER_TAPS) * 2.0 - 1.0;
      let dir = normalize(sky_ray() * vec3f(ndc, 1.0));
      sum += dot(beyond(air_along(dir), fill), LUMA);
    }
  }
  let light = sum / f32(METER_TAPS * METER_TAPS) * PI / atmos_ground_albedo();
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos_adaptation()), 1.0, atmos_max_adaptation());
  return vec4f(atmos_exposure() * adapted, 0.0, 0.0, 1.0);
}
