// Eye adaptation to the light around the eye, whichever way it looks: the sky
// and the Earth past the terrain, unexposed, their mean luminance over the
// whole sphere against the same meter's reading at sea level under a sun
// overhead, over the Earth's mean albedo. There the eye is at one, and
// EXPOSURE alone says how bright that shows.

const METER_TAPS: i32 = 64;
// The golden angle, which spreads the taps evenly about the axis.
const GOLDEN_ANGLE: f32 = 2.39996323;

// Half the sphere is the sky, half the ground lit by the sun and that sky.
fn meter_reference() -> f32 {
  let r = atmos_bottom_radius();
  let sky = sky_ground(r, 1.0);
  let ground = atmos_ground_albedo() / PI * dot((sun_transmittance(r, 1.0) + sky.rgb) * atmos_white(), LUMA);
  return 0.5 * (sky.a + ground);
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  var sum = 0.0;
  for (var i = 0; i < METER_TAPS; i++) {
    let z = 1.0 - (f32(i) + 0.5) * 2.0 / f32(METER_TAPS);
    let s = sqrt(1.0 - z * z);
    let phi = GOLDEN_ANGLE * f32(i);
    sum += dot(beyond(air_along(vec3f(s * cos(phi), s * sin(phi), z))), LUMA);
  }
  let light = sum / f32(METER_TAPS) / meter_reference();
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos_adaptation()), 1.0, atmos_max_adaptation());
  return vec4f(atmos_exposure() * adapted, 0.0, 0.0, 1.0);
}
