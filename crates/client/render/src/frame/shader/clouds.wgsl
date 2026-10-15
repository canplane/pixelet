// The thin cloud layers over the cumulus (pxl-kernel scene/environment/cloud.rs): altocumulus'
// small heaps at 4 to 5 km and cirrus' veil at 6 to 12, each one sheet read
// once where a ray meets it, laid over the opaque world. They write no depth:
// from below they fill only the sky, from above they cover everything, so a
// peak above a sheet is wrong either way.

struct Clouds {
  ray: mat3x3f,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(1) var<uniform> clouds_words: array<vec4f, 3>;
@group(0) @binding(2) var cloud_tile: texture_2d<f32>;
@group(0) @binding(3) var cloud_tile_sampler: sampler;

fn clouds_ray() -> mat3x3f { return mat3x3f(clouds_words[0].xyz, clouds_words[1].xyz, clouds_words[2].xyz); }
// Faded over the upper half of the climb to where the clouds are gone (look/cloud.rs `shown`).
fn clouds_shown() -> f32 { return clouds_words[0].w; }

// The lattice's footprint where `dir` is `t` km on, before any branch takes a derivative.
fn footprint_at(dir: vec3f, t: f32) -> f32 {
  return max(length(fwidth(cloud_lattice(frame_camera_pos() + dir * (max(t, 0.0) / atmos_km_per_u())))), 1e-6);
}

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

// Where a ray from radius `r0` at cosine `mu` is inside a sphere of radius `r`, or (-1, -1).
fn ray_sphere(r0: f32, mu: f32, r: f32) -> vec2f {
  let disc = r0 * r0 * (mu * mu - 1.0) + r * r;
  if (disc < 0.0) {
    return vec2f(-1.0);
  }
  let root = sqrt(disc);
  return vec2f(-r0 * mu - root, -r0 * mu + root);
}

struct Sheet {
  km: f32,
  // How many times finer than the tile its pattern is.
  finer: u32,
  // How much softer its edges read, as a footprint's multiple.
  softer: f32,
  opacity: f32,
  salt: u32,
};

const ALTOCUMULUS: Sheet = Sheet(4.5, 3u, 1.0, 0.6, 0x0a1cu);
const CIRRUS: Sheet = Sheet(9.0, 1u, 4.0, 0.45, 0x0c1cu);

struct Layer {
  // Linear light times opacity, before the air to it.
  color: vec3f,
  opacity: f32,
  // How far along the ray it is seen, in km.
  t: f32,
};

// Where `dir` meets the sphere `km` over the ground: the way out from under it,
// the way in from over it.
fn sheet_t(r0: f32, mu: f32, km: f32) -> f32 {
  let r = atmos_bottom_radius() + km;
  let at = ray_sphere(r0, mu, r);
  return select(at.y, at.x, r0 > r);
}

// Lit where the ray meets it as the cumulus' faces are, the face turned to the
// eye, so the layers are one white.
fn sheet(s: Sheet, dir: vec3f, t: f32, footprint: f32, reach: f32) -> Layer {
  var out: Layer;
  out.t = t;
  if (t <= 0.0 || t > reach) {
    return out;
  }
  let world = frame_camera_pos() + dir * (t / atmos_km_per_u());
  let p = cloud_lattice(world);
  // Read a whole number of units off, so each layer has its own clouds.
  let field = cloud_opacity_finer(p + vec2f(f32(s.salt % CLOUD_TILE_UNITS)), footprint * s.softer, s.finer);
  out.opacity = s.opacity * field * weather(p, s.salt) * reach_fade(t, reach);
  let up = up_at(world);
  out.color = CLOUD_ALBEDO * surface_light(-sign(dot(dir, up)) * up, up, light_of(world)) * out.opacity;
  return out;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let dir = normalize(clouds_ray() * vec3f(v.ndc, 1.0));
  let r0 = camera_r();
  let mu = dot(dir, frame_up());
  let reach = cloud_reach(CIRRUS.km);
  let alto_t = sheet_t(r0, mu, ALTOCUMULUS.km);
  let cirrus_t = sheet_t(r0, mu, CIRRUS.km);
  let alto_footprint = footprint_at(dir, alto_t);
  let cirrus_footprint = footprint_at(dir, cirrus_t);

  var near = sheet(ALTOCUMULUS, dir, alto_t, alto_footprint, cloud_reach(ALTOCUMULUS.km));
  var far = sheet(CIRRUS, dir, cirrus_t, cirrus_footprint, reach);
  if (far.t < near.t) {
    let x = near;
    near = far;
    far = x;
  }
  // The air to both in one gather along the ray.
  let air = air_segments(dir, clamp(vec3f(near.t, far.t, far.t), vec3f(0.0), vec3f(reach)));
  near.color = near.color * air[0].through + air[0].scattered * exposure() * near.opacity;
  far.color = far.color * air[1].through + air[1].scattered * exposure() * far.opacity;
  let color = near.color + (1.0 - near.opacity) * far.color;
  let opacity = near.opacity + (1.0 - near.opacity) * far.opacity;
  let alpha = opacity * clouds_shown();
  if (alpha <= 0.002) {
    discard;
  }
  return vec4f(finish(color / opacity, v.clip.xy), alpha);
}
