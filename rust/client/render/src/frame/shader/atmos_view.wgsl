// The air before a point as the eye sees it, marched (`air_held`): what the
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
// the world's top, where a position's Z ends (`HEIGHT_BITS` about sea level),
// none while it stands within the world. The air's volume starts there, so an
// eye high over the world spends no slice on the empty air under it.
fn air_near() -> f32 {
  let top = f32(1u << (HEIGHT_BITS - 1u));
  return max(atmos_camera_altitude() / atmos_km_per_u() - top, 0.0);
}

// How far from the eye the air along `dir` ends, in units: where the march
// stops, at the air's ground (`to_ground`, the sea's radius), past which a
// point's air is the air to there; else the volume's whole reach.
fn air_ground(dir: vec3f) -> f32 {
  let r = camera_r();
  let mu = dot(frame_up(), dir);
  return select(air_far(), to_ground(r, mu) / atmos_km_per_u(), meets_ground(r, mu));
}

// Where along `dir` the air's volume lays its slices, in units, nearest and
// farthest: from where the air begins that anything drawn may lie in — past
// `air_near`, and past the air's top for an eye over it — to the sea's
// plane, the world's z = 0, under which little is drawn; where the ray never
// falls to it, the air's ground or its far edge (`air_ground`). Laid from the
// air's top to the sea, an eye high over the world spends no slice on the
// space before the air or past the sea, which had left the air a ray crosses
// a few slices tens of km thick and the sea read between them, banded a
// slice at a time about the point under the eye. A column of the volume
// takes its neighbours' too (atmos_froxel.wgsl `column_span`).
fn air_span(dir: vec3f) -> vec2f {
  let r = camera_r();
  let mu = dot(frame_up(), dir);
  let near = max(air_near(), to_sphere(r, mu, atmos_top_radius()) / atmos_km_per_u());
  let sea = atmos_camera_altitude() / atmos_km_per_u() / max(-dir.z, 1e-6);
  let far = select(air_ground(dir), min(sea, air_far()), dir.z < 0.0 && atmos_camera_altitude() > 0.0);
  return vec2f(near, max(far, near * (1.0 + 1e-6) + 1e-6));
}

// A span as the volume holds it, in a 16-bit float a side: its ends' log2
// past one, and one more so a 16-bit float holds it at full precision, which
// keeps a span of metres and one of thousands of km alike to a percent.
// Rounded here to the 10 bits of mantissa the texel keeps, so the column's
// maker marches the span its reader reads and the froxels are read where
// they were made.
fn air_span_held(span: vec2f) -> vec2f {
  let v = log2(span + 1.0) + 1.0;
  return bitcast<vec2f>((bitcast<vec2u>(v) + vec2u(0x1000u)) & vec2u(0xffffe000u));
}

fn air_span_of(held: vec2f) -> vec2f {
  return exp2(held - 1.0) - 1.0;
}

// How far from the eye a froxel `z` deep reaches along a ray whose slices lie
// over `span`, in units: its depth cubed of the span. A slice's reach grows
// with its depth, so the near view, where most of what is drawn lies, takes
// most of the slices — from the ground looking out level, the first ~4 m,
// the second ~116 m, the ninth ~21 km — and the last ends where the span
// does, so what is drawn there, the sea from high, reads the march to it
// whole (atmos_froxel.wgsl, atmos_air.wgsl).
fn air_reach(span: vec2f, z: f32) -> f32 {
  return span.x + (span.y - span.x) * z * z * z;
}

// `air_reach` the other way: how deep a point `d` units from the eye lies.
fn air_z(span: vec2f, d: f32) -> f32 {
  return pow(clamp((d - span.x) / max(span.y - span.x, 1e-6), 0.0, 1.0), 1.0 / 3.0);
}

// The march to a point, the volume's one maker (atmos_froxel.wgsl): what
// the air scatters toward the eye, and in alpha what it lets through. An
// authored color is dimmed alike at every wavelength: by wavelength, a light
// sea's blue went and its green stayed.
fn air_held(world_pos: vec3f) -> vec4f {
  let v = air_to(world_pos);
  return vec4f(v.scattered, dot(v.through, LUMA));
}


/* glare */

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(dir, s, dot(s, frame_up()), exposure());
}
