// The clouds: a layer of them over the whole planet, gpu/clouds.rs's
// ALTITUDE_KM up and curved with it, drawn after the opaque world as one
// triangle over the screen. Each pixel's ray meets the layer's sphere — from
// below looking up, or from above looking down — and where it does, the
// cloud there is value noise on the map's own lattice, anchored to the
// ground and drifting with the wind, lit by the sun and the moon through the
// air as they reach that height and by the sky above it, and seen through
// the air between.
//
// It writes no depth of its own, so the depth test runs before the shader:
// the triangle lies at the far plane, and from below the layer it is drawn
// only where the opaque world left the sky, so the ground and a mountain
// stand in front of the clouds and a pixel they cover never runs this; from
// above, it is drawn over everything, and the clouds hide the ground under
// them (gpu/clouds.rs picks the pipeline). What that gets wrong is a peak
// standing above the layer: from below, its top is not hidden by the cloud
// in front of it, and from above, it is.

struct Clouds {
  // A pixel's ray in the root frame, affine in its NDC (gpu/renderer.rs's
  // put_ray).
  ray: mat3x3f,
  // Where the root frame's origin is on the map's lattice, which the clouds
  // are anchored to: the lattice index, exact, and the fraction of a unit
  // past it (gpu/clouds.rs).
  base: vec2u,
  frac: vec2f,
  time: f32,
  // The lattice's count across the world, where it wraps.
  count: u32,
  // The layer's height, a fair-weather cumulus base (gpu/clouds.rs's
  // ALTITUDE_KM).
  altitude_km: f32,
  _p0: f32,
  // What reaches the layer's height of the sun's light and of the moon's,
  // by channel, unexposed: the same for every point of it the frame shows,
  // so the CPU takes it once (look/atmosphere.rs's light_at).
  sun_at: vec3f,
  _p1: f32,
  moon_at: vec3f,
  _p2: f32,
};

@group(0) @binding(1) var<uniform> clouds: Clouds;

// How much of the sky the layer covers.
const CLOUD_COVER: f32 = 0.42;
// The size of the largest cloud shapes: the lattice the noise is drawn on,
// 2^12 cells, some eight kilometres in the middle latitudes. A power of two,
// so it divides the world and the clouds wrap with the map (gpu/clouds.rs's
// LATTICE_SHAMT).
const CLOUD_LATTICE_CELLS: u32 = 4096u;
// The wind that carries them, in cells a second: about eight metres a
// second, at the cell's size in the middle latitudes.
const CLOUD_WIND: vec2f = vec2f(3.0, 1.25);
// How thick a full cloud is to light, as optical depth: past a few, its
// underside is in its own shadow and grey, and its edges, thin, are bright.
const CLOUD_DEPTH: f32 = 6.0;
const CLOUD_ALBEDO: f32 = 0.9;
// Past this, from under the layer or close over it, the layer is too thin a
// sliver of the view to be worth the noise, and fades into the haze it is
// already mostly behind. From higher up the reach grows with the height
// over the layer, so a view from orbit still sees the clouds below it.
const CLOUD_REACH_KM: f32 = 120.0;

struct CloudOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> CloudOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: CloudOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}


// Henyey-Greenstein, for the light a cloud throws forward — the silver
// lining round a cloud in front of the sun.
fn hg(c: f32, g: f32) -> f32 {
  return INV_4PI * (1.0 - g * g) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

// What a cloud scatters toward the eye, per unit of its albedo, from a
// light toward `toward` of which `arriving` reaches the layer, shown at
// `shown_at`: thrown forward and back by the droplets and spread through the
// cloud by scattering many times over.
fn lit_by(toward: vec3f, view: vec3f, arriving: vec3f, shown_at: vec3f) -> vec3f {
  let c = dot(view, toward);
  let phase = 0.7 * hg(c, 0.6) + 0.3 * hg(c, -0.25) + INV_4PI;
  return arriving * phase * 4.0 * shown_at;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let dir = normalize(clouds.ray * vec3f(v.ndc, 1.0));

  // The layer's sphere from the camera's: the near crossing from above, the
  // way out from below.
  let r0 = camera_r();
  let shell = atmos.bottom_radius + clouds.altitude_km;
  let disc = r0 * r0 * (dir.z * dir.z - 1.0) + shell * shell;
  var t = -1.0;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    t = select(-r0 * dir.z + root, -r0 * dir.z - root, r0 > shell);
  }
  let reach = CLOUD_REACH_KM + 3.0 * max(atmos.camera_altitude - clouds.altitude_km, 0.0);

  // Where the ray meets the layer, in cells from the root frame's origin —
  // the kilometres along the layer taken back to the map's units at its
  // centre — and so in lattice units past the origin's lattice point; and
  // how many of those a pixel spans, a derivative, so taken before any
  // pixel is discarded.
  let du = t / atmos.km_per_u;
  let cells = frame.camera_pos.xy + dir.xy * du + clouds.time * CLOUD_WIND;
  let p = clouds.frac + cells / f32(CLOUD_LATTICE_CELLS);
  let footprint = max(length(fwidth(p)), 1e-6);
  if (t <= 0.0 || t > reach) {
    discard;
  }
  let shape = lattice_fbm(clouds.base, p, clouds.count, 0u, 0u, footprint) * 0.75 + lattice_fbm(clouds.base, p, clouds.count, 2u, 16u, footprint) * 0.25;
  let density = smoothstep(1.0 - CLOUD_COVER - 0.12, 1.0 - CLOUD_COVER + 0.18, shape);
  if (density <= 0.001) {
    discard;
  }
  let depth = density * CLOUD_DEPTH;
  let alpha = (1.0 - exp(-depth)) * (1.0 - smoothstep(0.6, 1.0, t / reach));

  // Lit from above: a cloud seen from below shows its underside, which the
  // cloud itself shades as it thickens; seen from above, its lit top.
  let below = r0 < shell;
  let self_shade = select(1.0, mix(1.0, 0.3, 1.0 - exp(-depth * 0.5)), below);
  var light = lit_by(atmos.sun_dir, dir, clouds.sun_at, shown());
  var sky = zenith_sky();
  if (atmos.moon_light > 0.0) {
    light += lit_by(atmos.moon_dir, dir, clouds.moon_at, moon_shown());
    sky += moon_view(vec3f(0.0, 0.0, 1.0)) * moon_shown();
  }
  light *= self_shade;
  // The sky above lights the layer as well: its zenith, half of it thrown
  // back out the side the eye sees.
  var color = CLOUD_ALBEDO * (light + sky * 0.5);

  // Where the layer lies in the root frame, for the air in front of it.
  let world = frame.camera_pos + dir * du;
  color = aerial(color, world, air_along(dir));
  return vec4f(finish(shoulder(color), v.clip.xy), alpha);
}
