// Where a point stands in the air and the lights there, and what the air
// before it is (`Seen`), which every painter reads (atmos_air.wgsl
// `seen_from_eye`).



/* where a point stands */

// The air's center, straight under the eye at the air's radius from it.
fn planet_center() -> vec3f {
  return vec3f(0.0, 0.0, -camera_r() / atmos_km_per_u());
}

// Straight up at a point, away from the air's center.
fn up_at(world_pos: vec3f) -> vec3f {
  return normalize(world_pos - planet_center());
}

// A point's radius in the air, by its height on the flat map: the eye's
// radius and the point's height over it, its z. Measured from the air's
// center instead, flat ground d out stood d²/2R up — 7 km at 300 km — and
// its light swept the tables' radius rows, a band at each.
fn radius_of(world_pos: vec3f) -> f32 {
  return max(camera_r() + (world_pos.z - frame_camera_pos().z) * atmos_km_per_u(), atmos_bottom_radius());
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

// The air before a point: what it lets through, what it scatters toward
// the eye, and the lights there. What it scatters per share it takes varies
// slowly across directions, which is what lets the air's volume hold it
// (atmos_air.wgsl); the lights turn with a point's height, and are read
// where it stands (`light_of`).
struct Seen {
  transmittance: f32,
  scattered: vec3f,
  light: Light,
};

/* glare */

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(camera_r(), dir, s, dot(s, frame_up()), exposure());
}
