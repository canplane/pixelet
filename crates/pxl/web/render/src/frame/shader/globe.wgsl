// The flat map laid back on the planet, for every pass that places a point
// of the world: frame.wgsl's `on_globe` with the frame's own values, and a
// pass with a uniform of its own (badge.wgsl) with the same values carried
// in it. `planet` is the camera centre — the map centre as the view has
// eased to it — in the root frame, the planet's radius in its units (0 for
// none) and the centre's height; `globe` the centre's
// Mercator exponential, its latitude's sine and cosine, and Mercator's radius
// in units (look/globe.rs).

// e^x − 1 without the cancellation near zero, which WGSL has no builtin for:
// a Mercator ordinate a unit from the centre's is a millionth of a radian.
fn expm1(x: f32) -> f32 {
  if (abs(x) < 0.1) {
    return x * (1.0 + x * (0.5 + x * (1.0 / 6.0 + x / 24.0)));
  }
  return exp(x) - 1.0;
}

// sin and atan for the small angles the placement lives on, as their series.
// WGSL holds its builtins to an absolute error of 2^-11 and no better, so a
// GPU is free to answer sin(1e-7) with anything within half a thousandth —
// and times the planet's radius in units, that is metres or hundreds of
// them: a thing a unit from the map centre torn apart along it. The series
// keeps its relative precision down to zero and, to the seventh power, to a
// part in a hundred million across the half radian the window can reach.
fn sin_small(x: f32) -> f32 {
  if (abs(x) > 1.0) {
    return sin(x);
  }
  let x2 = x * x;
  return x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0)));
}

fn atan_small(x: f32) -> f32 {
  if (abs(x) > 0.3) {
    return atan(x);
  }
  let x2 = x * x;
  return x * (1.0 - x2 * (1.0 / 3.0 - x2 * (1.0 / 5.0 - x2 * (1.0 / 7.0 - x2 / 9.0))));
}

// A point of the flat map where the planet has it: look/globe.rs's
// `Globe::place` in f32, taking each point back to its latitude and longitude
// and setting it on the sphere in the map centre's east-north-up frame. The
// identity at the centre, so what is near the camera does not move; further
// out, Mercator's stretch north is undone and the ground falls away, so the
// ground meets the horizon the sky is drawn with.
fn on_globe_of(p: vec3f, planet: vec4f, globe: vec4f) -> vec3f {
  let r = planet.z;
  if (r <= 0.0) {
    return p;
  }
  let d = p.xy - planet.xy;
  let e0 = globe.x;
  let dl = d.x / globe.w;
  let ey = expm1(d.y / globe.w);
  let e = e0 * (1.0 + ey);
  let dphi = 2.0 * atan_small(e0 * ey / (1.0 + e * e0));
  let c = 2.0 * e / (1.0 + e * e);
  let h = (p.z - planet.w) * c / globe.z;
  let rr = r + h;
  let shl = sin_small(0.5 * dl);
  let shp = sin_small(0.5 * dphi);
  let haversine = shp * shp + c * globe.z * shl * shl;
  let east = rr * c * sin_small(dl);
  let north = rr * (sin_small(dphi) + 2.0 * c * globe.y * shl * shl);
  return vec3f(planet.x + east, planet.y + north, planet.w + h - 2.0 * rr * haversine);
}

// Where on the flat map a point of the planet is: east and north of the map
// centre in map units — look/globe.rs's `Globe::unplace` in f32, the inverse
// of `on_globe_of` but for the height, which it does not keep. Its latitude
// and longitude are read off the sphere and taken to Mercator's ordinates;
// atan2 is held to its ulps rather than to 2^-11, and what reads this — the
// clouds' lattice, thousands of units across — is not a place a unit is
// seen.
fn off_globe_of(p: vec3f, planet: vec4f, globe: vec4f) -> vec2f {
  let r = planet.z;
  if (r <= 0.0) {
    return p.xy - planet.xy;
  }
  let v = vec3f(p.xy - planet.xy, p.z - planet.w + r);
  let axis = v.y * globe.z + v.z * globe.y;
  let meridian = -v.y * globe.y + v.z * globe.z;
  let across = length(vec2f(v.x, meridian));
  let e = (length(v) + axis) / across;
  return vec2f(globe.w * atan2(v.x, meridian), globe.w * log(e / globe.x));
}
