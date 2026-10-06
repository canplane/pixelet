// The flat map laid back on the planet.

// WGSL has no expm1; a Mercator ordinate a unit from the center is a millionth of a radian.
fn expm1(x: f32) -> f32 {
  if (abs(x) < 0.1) {
    return x * (1.0 + x * (0.5 + x * (1.0 / 6.0 + x / 24.0)));
  }
  return exp(x) - 1.0;
}

// WGSL allows its sin and atan an absolute error of 2^-11, which times the
// planet's radius tears apart things a unit from the center. The series keeps
// relative precision to 1e-8 over the half radian the window reaches.
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

// Identity at the center, so what is near the camera does not move.
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

// Drops the height. atan2 is held to its ulps, not 2^-11.
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
