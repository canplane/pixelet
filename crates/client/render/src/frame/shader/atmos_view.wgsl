// The air before a point as the eye sees it, marched (`seen_at`): what the
// air's volume is made of (atmos_froxel.wgsl), which every painter reads
// (atmos_air.wgsl `seen_from_eye`); and the lights at a point.



/* where a point stands */

// The air's centre, straight under the eye at the air's radius from it.
fn planet_center() -> vec3f {
  return vec3f(0.0, 0.0, -camera_r() / atmos_km_per_u());
}

// Straight up at a point, away from the air's centre.
fn up_at(world_pos: vec3f) -> vec3f {
  return normalize(world_pos - planet_center());
}

// From the eye's radius: the frame's units are exact near it.
fn radius_of(world_pos: vec3f) -> f32 {
  let c = planet_center();
  return max(camera_r() + (length(world_pos - c) - length(frame_camera_pos() - c)) * atmos_km_per_u(), atmos_bottom_radius());
}


/* lights */

fn shown() -> vec3f {
  return atmos_white() * exposure();
}

fn moon_shown() -> vec3f {
  return shown() * atmos_moon_light();
}

fn light_at(r: f32, up: vec3f) -> Light {
  let l = light_unexposed(r, up);
  let e = exposure();
  return Light(l.sun * e, l.other * e, l.fill * e);
}

fn light_of(world_pos: vec3f) -> Light {
  return light_at(radius_of(world_pos), up_at(world_pos));
}

/* the air before a point */

// The light per unit of depth bends sharply only in the ozone: under it three
// knots err less than eight do from over it, a percent or two. Crossing the
// line, the whole frame steps by that much at once and shows no flicker.
fn air_knots() -> i32 {
  return select(8, 3, atmos_camera_altitude() < atmos_ozone_center() - atmos_ozone_half_width());
}

// The points `d` km along `dir`, nearest first, unexposed.
fn air_segments(dir: vec3f, d: vec3f) -> array<Veil, 3> {
  var v = gather(camera_r(), frame_up(), dir, d, atmos_sun_dir(), atmos_moon_dir(), atmos_moon_light(), air_knots());
  for (var i = 0; i < 3; i++) {
    v[i].scattered *= atmos_white();
  }
  return v;
}

fn air_to(world_pos: vec3f) -> Veil {
  let to = world_pos - frame_camera_pos();
  let du = length(to);
  if (du < 1e-4) {
    return Veil(vec3f(1.0), vec3f(0.0));
  }
  let v = air_segments(to / du, vec3f(du * atmos_km_per_u()))[2];
  return Veil(v.through, v.scattered * exposure());
}

// The air before a point: what it lets through, what it scatters toward
// the eye, and the lights there. Every term varies slowly across the view,
// which is what lets the air's volume hold it (atmos_air.wgsl).
struct Seen {
  through: f32,
  scattered: vec3f,
  light: Light,
};

// How far the air's volume reaches, in units: a ray out of the eye through the
// air to where it leaves it, the longest the air reaches past the horizon.
fn air_far() -> f32 {
  let b = atmos_bottom_radius();
  let r = camera_r();
  let t = atmos_top_radius();
  return (sqrt(max(r * r - b * b, 0.0)) + sqrt(t * t - b * b)) / atmos_km_per_u();
}

// The share of `air_far` a froxel `z` deep reaches, its depth cubed: a
// slice's reach grows with its depth, so the near view, where most of what
// is drawn lies, takes most of the slices — the first ~4 m from the ground,
// the second ~116 m, the ninth ~21 km — and one volume reaches as far as
// the air (atmos_froxel.wgsl, atmos_air.wgsl).
fn air_depth(z: f32) -> f32 {
  return z * z * z;
}

// The march to a point, the volume's one maker (atmos_froxel.wgsl).
fn seen_at(world_pos: vec3f) -> Seen {
  let v = air_to(world_pos);
  var s: Seen;
  // An authored color is dimmed alike at every wavelength: by wavelength, a
  // light sea's blue went and its green stayed.
  s.through = dot(v.through, LUMA);
  s.scattered = v.scattered;
  s.light = light_of(world_pos);
  return s;
}


/* glare */

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(dir, s, dot(s, frame_up()), exposure());
}
