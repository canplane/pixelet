// The thin cloud layers over the cumulus (pxl-kernel domain/scene/environment/cloud.rs): altocumulus'
// small heaps at 4 to 5 km and cirrus' veil at 6 to 12, each one plane at its
// height over the eye, out to the clouds' reach, laid over the opaque world.
// They write no depth and the world's decides what stands before them, so a
// peak over a sheet hides it from below and stands through it from above.

// Its words' fields and accessors are frame/shader/word.rs `clouds`'s,
// which build.rs writes here.
@group(0) @binding(1) var<uniform> clouds_words: array<vec4f, CLOUDS_VEC4S>;
@group(0) @binding(2) var cloud_tile: texture_2d<f32>;
@group(0) @binding(3) var cloud_tile_sampler: sampler;

struct Sheet {
  km: f32,
  // How many times finer than the tile its pattern is.
  finer: u32,
  // How much softer its edges read, as a footprint's multiple.
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
fn sheet_opacity(s: Sheet, world: vec3f, t: f32, footprint: f32, reach: f32) -> f32 {
  if (t > reach) {
    return 0.0;
  }
  let p = cloud_lattice(world);
  // Read a whole number of units off, so each layer has its own clouds.
  let field = cloud_opacity_finer(p + vec2f(f32(s.salt % CLOUD_TILE_UNITS)), footprint * s.softer, s.finer);
  return s.opacity * field * weather(p, s.salt) * reach_fade(t, reach);
}

struct CloudOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) @interpolate(flat) sheet: u32,
};

// Each sheet a square on the flat map at its height over the sea, which the
// world's ground is laid on too, wide enough to hold every point within the
// reach of the eye. The farther first, so the nearer is blended over it.
@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> CloudOut {
  let alto = ALTOCUMULUS.km - atmos_camera_altitude();
  let cirrus = CIRRUS.km - atmos_camera_altitude();
  let k = select(1u - i, i, abs(alto) > abs(cirrus));
  let h = select(cirrus, alto, k == 0u);
  let reach = clouds_reach();
  let half = sqrt(max(reach * reach - h * h, 0.0)) / atmos_km_per_u();
  var corners = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));
  var o: CloudOut;
  o.world = frame_camera_pos() + vec3f(corners[v] * half, 0.0) + frame_up() * (h / atmos_km_per_u());
  o.clip = frame_view_proj() * vec4f(o.world, 1.0);
  o.sheet = k;
  return o;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let to = v.world - frame_camera_pos();
  let dir = normalize(to);
  let t = length(to) * atmos_km_per_u();
  // The lattice's footprint, before any branch takes a derivative.
  let footprint = max(length(fwidth(cloud_lattice(v.world))), 1e-6);
  let alpha = sheet_opacity(sheet_of(v.sheet), v.world, t, footprint, clouds_reach()) * clouds_shown();
  // Where none of it stands, nothing more is read.
  if (alpha <= DISPLAY_FLOOR) {
    discard;
  }
  // The air before it and the lights on it, as every painter reads them
  // (atmos_air.wgsl `seen_from_eye`); lit as the cumulus' faces are, the
  // face turned to the eye, so the layers are one white.
  let seen = seen_from_eye(v.world);
  let up = up_at(v.world);
  let lit = CLOUD_ALBEDO * surface_light(-sign(dot(dir, up)) * up, up, seen.light);
  return vec4f(finish(lit * seen.through + seen.scattered, v.clip.xy), alpha);
}
