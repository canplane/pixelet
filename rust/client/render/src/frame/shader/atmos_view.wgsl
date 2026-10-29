// The air before a point as the eye sees it, marched (`air_held`): what the
// air's volume is made of (atmos_froxel.wgsl), which every painter reads
// (atmos_air.wgsl `seen_from_eye`); and the lights at a point.



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
// the eye, and the lights there. The air's two vary slowly across the view,
// which is what lets the air's volume hold them (atmos_air.wgsl); the lights
// turn with a point's height, and are read where it stands (`light_of`).
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

// The nearest anything drawn may lie, in units: how far the eye stands over
// the world's top, where a pos's Z ends (`HEIGHT_BITS` about sea level),
// none while it stands within the world. The air's volume starts there, so an
// eye high over the world spends no slice on the empty air under it.
fn air_near() -> f32 {
  let top = f32(1u << (HEIGHT_BITS - 1u));
  return max(atmos_camera_alt() / atmos_km_per_u() - top, 0.0);
}

// The volume's depth, one law for every column: slice z, 0 to 1, lies
// `air_reach(z)` units out, the cube of z over the span from the nearest
// anything is drawn (`air_near`) to the farthest the air reaches
// (`air_far`). A cube puts the near view, where most of what is drawn lies,
// in most of the slices and keeps a far slice a tenth or so past the one
// before. Where a slice lies depends on the eye alone, never on the column,
// so the screen's depth is continuous across it.
fn air_ends() -> vec2f {
  let near = air_near();
  return vec2f(near, max(air_far(), near + 1.0));
}

fn air_reach(z: f32) -> f32 {
  let e = air_ends();
  return e.x + (e.y - e.x) * z * z * z;
}

// Where a point `d` units out lies among the slices, 0 to the last: the
// law's inverse, smooth in `d` across every slice.
fn air_slice(d: f32) -> f32 {
  let e = air_ends();
  return pow(clamp((d - e.x) / (e.y - e.x), 0.0, 1.0), 1.0 / 3.0) * f32(AIR_FROXELS - 1u);
}

// The air from `t0` to `t1` units along `dir` from the eye, exposed: one
// stretch of the volume's column march (atmos_froxel.wgsl).
fn air_stretch(dir: vec3f, t0: f32, t1: f32) -> Veil {
  let km = atmos_km_per_u();
  let v = gather_between(camera_r(), frame_up(), dir, t0 * km, t1 * km, atmos_sun_dir(), atmos_moon_dir(), atmos_moon_light(), ATMOS_AIR_STRETCH_KNOTS);
  return Veil(v.through, v.scattered * atmos_white() * exposure());
}

// What a froxel holds of the air from the eye to it: what it scatters
// toward the eye per share of the light it takes, per channel, S / (1 - T).
// The air's thickness cancels from it, so it turns slowly across columns and
// slices however fast the air thickens, and a pixel scales it by its own
// ray's 1 - T (atmos_air.wgsl `air_before`). Before the air takes anything
// it holds none.
fn air_held(v: Veil) -> vec4f {
  let taken = vec3f(1.0) - v.through;
  return vec4f(select(vec3f(0.0), v.scattered / taken, taken > vec3f(1e-6)), 1.0);
}


/* glare */

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(dir, s, dot(s, frame_up()), exposure());
}
