// Draws the thin cloud layers over the cumulus (pxl-core
// domain/scene/environment/cloud.rs): altocumulus' small heaps at 4 to 5 km and
// cirrus' veil at 6 to 12 km. Each layer is one plane at its altitude over the
// eye, out to the clouds' reach, laid over the opaque world. The layers write
// no depth, and the world's depth decides what stands before them, so a peak
// over a sheet hides it from below and stands through it from above.

// The clouds' words. Their fields are frame/shader/word.rs `clouds`'s, and
// build.rs writes their accessors here.
@group(0) @binding(1) var<uniform> clouds_words: array<vec4f, CLOUDS_VEC4S>;
@group(0) @binding(2) var cloud_tile: texture_2d<f32>;
@group(0) @binding(3) var cloud_tile_sampler: sampler;

struct Sheet {
  km: f32,
  // How many times finer than the tile its pattern is.
  finer: u32,
  // How much softer its edges read, as a multiple of the footprint.
  softer: f32,
  opacity: f32,
  salt: u32,
};

const ALTOCUMULUS: Sheet = Sheet(
  CLOUD_ALTOCUMULUS_KM,
  CLOUD_ALTOCUMULUS_FINER,
  CLOUD_ALTOCUMULUS_SOFTER,
  CLOUD_ALTOCUMULUS_OPACITY,
  CLOUD_ALTOCUMULUS_SALT,
);
const CIRRUS: Sheet = Sheet(
  CLOUD_CIRRUS_KM,
  CLOUD_CIRRUS_FINER,
  CLOUD_CIRRUS_SOFTER,
  CLOUD_CIRRUS_OPACITY,
  CLOUD_CIRRUS_SALT,
);

fn sheet_of(i: u32) -> Sheet {
  if (i == 0u) {
    return ALTOCUMULUS;
  }
  return CIRRUS;
}

// How much of the sheet stands where the ray meets it, `t` km out.
fn sheet_opacity(s: Sheet, world: vec3f, t: f32, footprint: f32, dist: f32) -> f32 {
  if (t > dist) {
    return 0.0;
  }
  let p = cloud_lattice(world);
  // Each layer reads the tile a whole number of units off, so each has clouds
  // of its own.
  let field = cloud_opacity_finer(p + vec2f(f32(s.salt % CLOUD_TILE_UNITS)), footprint * s.softer, s.finer);
  return s.opacity * field * weather(p, s.salt) * draw_dist_fade(t, dist);
}

struct CloudOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) @interpolate(flat) sheet: u32,
};

// Each sheet is a square at its altitude on the flat map, which the world's
// ground is laid on too, wide enough to hold every point within the clouds'
// reach from the eye. The farther sheet is drawn first, so the nearer is
// blended over it.
@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> CloudOut {
  let alto = ALTOCUMULUS.km - atmos_camera_alt();
  let cirrus = CIRRUS.km - atmos_camera_alt();
  let k = select(1u - i, i, abs(alto) > abs(cirrus));
  let h = select(cirrus, alto, k == 0u);
  let dist = clouds_draw_dist();
  let half = sqrt(max(dist * dist - h * h, 0.0)) / atmos_km_per_u();
  let step = vec2f(quad_corner(v));
  var o: CloudOut;
  o.world = frame_camera_pos() + vec3f((step * 2.0 - 1.0) * half, 0.0) + frame_up() * (h / atmos_km_per_u());
  o.clip = jittered(frame_view_proj() * vec4f(o.world, 1.0));
  o.sheet = k;
  return o;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let to = v.world - frame_camera_pos();
  let dir = normalize(to);
  let t = length(to) * atmos_km_per_u();
  // The pixel's footprint on the lattice is taken here, before any branch, as
  // a derivative needs uniform control flow.
  let footprint = max(length(fwidth(cloud_lattice(v.world))), 1e-6);
  let alpha = sheet_opacity(sheet_of(v.sheet), v.world, t, footprint, clouds_draw_dist()) * clouds_shown();
  if (alpha <= HALF_STEP) {
    discard;
  }
  // The sheet reads the air before it and the lights on it as every painter
  // does (atmos_air.wgsl `seen_from_eye`, atmos_view.wgsl `light_of`). It is
  // lit as the cumulus' faces are, with its face turned to the eye, so all the
  // layers are one white.
  let seen = seen_from_eye(v.world);
  let up = up_at(v.world);
  let lit = CLOUD_ALBEDO * surface_light(-sign(dot(dir, up)) * up, up, light_of(v.world));
  return vec4f(tone(lit * seen.transmittance + seen.scattered), alpha);
}
