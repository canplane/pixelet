// The frame's uniform as every pass declares it, at group 0 binding 0: the
// view, the camera, the environment's run (look/environment.rs's output, sun_dir
// through its padding), the effects' strengths, and the light's view for the
// shadow, and the second light — the dimmer of the sun and the moon, which
// casts no shadow; and the planet's curvature. The layout is shader.rs's
// FRAME_AT to name; the glue fills it.

struct Frame {
  view_proj: mat4x4f,
  camera_pos: vec3f,
  edge_strength: f32,
  sun_dir: vec3f,
  low_spec: f32,
  sun_color: vec3f,
  fog_near: f32,
  ambient: vec3f,
  fog_far: f32,
  hemi_sky: vec3f,
  floor: f32,
  hemi_ground: vec3f,
  edge_width: f32,
  _p5: vec3f,
  rim_intensity: f32,
  viewport: vec2f,
  shadow_strength: f32,
  shadow_texel: f32,
  vignette: f32,
  ao_strength: f32,
  screen_ao: f32,
  _p2: f32,
  light_view_proj: mat4x4f,
  other_dir: vec3f,
  _p3: f32,
  other_color: vec3f,
  _p4: f32,
  // The planet the flat map is laid back on (look/globe.rs): the map
  // centre in the root frame, the planet's radius in its units (0 for none),
  // and the centre's ground height; then the centre's Mercator exponential,
  // its latitude's sine and cosine, and Mercator's radius in units.
  planet: vec4f,
  globe: vec4f,
};

@group(0) @binding(0) var<uniform> frame: Frame;

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
fn on_globe(p: vec3f) -> vec3f {
  let r = frame.planet.z;
  if (r <= 0.0) {
    return p;
  }
  let d = p.xy - frame.planet.xy;
  let e0 = frame.globe.x;
  let dl = d.x / frame.globe.w;
  let ey = expm1(d.y / frame.globe.w);
  let e = e0 * (1.0 + ey);
  let dphi = 2.0 * atan_small(e0 * ey / (1.0 + e * e0));
  let c = 2.0 * e / (1.0 + e * e);
  let h = (p.z - frame.planet.w) * c / frame.globe.z;
  let rr = r + h;
  let shl = sin_small(0.5 * dl);
  let shp = sin_small(0.5 * dphi);
  let haversine = shp * shp + c * frame.globe.z * shl * shl;
  let east = rr * c * sin_small(dl);
  let north = rr * (sin_small(dphi) + 2.0 * c * frame.globe.y * shl * shl);
  return vec3f(frame.planet.x + east, frame.planet.y + north, frame.planet.w + h - 2.0 * rr * haversine);
}
