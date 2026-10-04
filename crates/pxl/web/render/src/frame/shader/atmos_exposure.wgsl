// Eye adaptation to the light around the eye, whichever way it looks: the sky
// and the Earth past the terrain, unexposed, over the whole sphere, their mean
// luminance read as the illuminance that lights the Earth's mean albedo to it,
// against a sun overhead at sea level (1 after white balance). Metered on the
// screen instead, tilting toward a bright horizon darkened the ground stops.

const METER_TAPS: i32 = 64;
// The golden angle, which spreads the taps evenly about the axis.
const GOLDEN_ANGLE: f32 = 2.39996323;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let fill = sky_fill();
  var sum = 0.0;
  for (var i = 0; i < METER_TAPS; i++) {
    let z = 1.0 - (f32(i) + 0.5) * 2.0 / f32(METER_TAPS);
    let s = sqrt(1.0 - z * z);
    let phi = GOLDEN_ANGLE * f32(i);
    sum += dot(beyond(air_along(vec3f(s * cos(phi), s * sin(phi), z)), fill), LUMA);
  }
  let light = sum / f32(METER_TAPS) * PI / atmos_ground_albedo();
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos_adaptation()), 1.0, atmos_max_adaptation());
  return vec4f(atmos_exposure() * adapted, 0.0, 0.0, 1.0);
}
