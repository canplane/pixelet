// Where a point stands in the air, the lights there, and what the air before
// it does to it (`Air`). Every painter reads them (atmos_air.wgsl
// `seen_from_eye`).

// The planet's center, straight under the eye at the eye's radius from it, in
// world units.
fn planet_center() -> vec3f {
  return vec3f(0.0, 0.0, -camera_r() / atmos_km_per_u());
}

// Straight up at a point, away from the planet's center.
fn up_at(world_pos: vec3f) -> vec3f {
  return normalize(world_pos - planet_center());
}

// A point's radius in the air, taken from its height on the flat map: the
// eye's radius plus the point's height over the eye, along z.
fn radius_of(world_pos: vec3f) -> f32 {
  return max(camera_r() + (world_pos.z - frame_camera_pos().z) * atmos_km_per_u(), BOTTOM_RADIUS_KM);
}

fn shown() -> vec3f {
  return atmos_white() * exposure();
}

fn light_at(r: f32, up: vec3f) -> Light {
  let l = light_unexposed(r, up);
  let e = exposure();
  return Light(l.sun * e, l.other * e, l.fill * e);
}

fn light_of(world_pos: vec3f) -> Light {
  return light_at(radius_of(world_pos), up_at(world_pos));
}

// The air before a point: what it lets through and what it scatters toward
// the eye. What it scatters per share it takes varies slowly across
// directions, which lets the air's volume hold it (atmos_air.wgsl). The lights
// at the point are not part of it: they change with the point's height, are
// read where it stands (`light_of`), and are passed beside it, as no struct
// holds another (build/check.rs `refuse_faults`).
struct Air {
  transmittance: f32,
  scattered: vec3f,
};

// The sun's glare along `dir`, drawn over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(camera_r(), dir, s, dot(s, frame_up()), exposure());
}
