// The cloud slab over the planet, marched at half the screen's size and laid
// over the opaque world by cloud_composite.wgsl. It writes no depth: from below
// it fills only the sky, from above it covers everything, so a peak above the
// layer is wrong either way.

struct Clouds {
  ray: mat3x3f,
  // The same for every point shown, so the CPU computes it once.
  sun_at: vec3f,
  _p1: f32,
  moon_at: vec3f,
  _p2: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(1) var<uniform> clouds_words: array<vec4f, 5>;

fn clouds_ray() -> mat3x3f { return mat3x3f(clouds_words[0].xyz, clouds_words[1].xyz, clouds_words[2].xyz); }
fn clouds_sun_at() -> vec3f { return clouds_words[3].xyz; }
fn clouds_moon_at() -> vec3f { return clouds_words[4].xyz; }

const CLOUD_ALBEDO: f32 = 0.9;
// Past this the layer is a sliver behind haze; from above the reach grows so
// it still meets the horizon.
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

fn hg(c: f32, g: f32) -> f32 {
  return INV_4PI * (1.0 - g * g) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

fn lit_by(toward: vec3f, view: vec3f, arriving: vec3f, shown_at: vec3f) -> vec3f {
  let c = dot(view, toward);
  let phase = 0.7 * hg(c, 0.6) + 0.3 * hg(c, -0.25) + INV_4PI;
  return arriving * phase * 4.0 * shown_at;
}

// A cumulus's depth, 1 to 2 km; the march's steps, few since nothing
// accumulates over frames drawn two seconds apart.
const CLOUD_THICK_KM: f32 = 1.5;
const CLOUD_STEPS: u32 = 8u;
// The share of the slab a cloud's top softens over.
const CLOUD_EDGE: f32 = 0.35;

// Where a ray from radius `r0` at cosine `mu` is inside a sphere of radius `r`, or (-1, -1).
fn ray_sphere(r0: f32, mu: f32, r: f32) -> vec2f {
  let disc = r0 * r0 * (mu * mu - 1.0) + r * r;
  if (disc < 0.0) {
    return vec2f(-1.0);
  }
  let root = sqrt(disc);
  return vec2f(-r0 * mu - root, -r0 * mu + root);
}

// Roberts' R2 sequence as a dither mask (2018): each pixel starts the march at
// its own offset, spread evenly with no diagonal grain, so the steps blur
// rather than band.
fn step_offset(at: vec2f) -> f32 {
  return fract(dot(floor(at), vec2f(0.7548776662, 0.5698402910)));
}

// Where clouds gather: value noise over cells this many lattice units wide,
// about 64 km, spread even through the logistic and cut so some country stays
// clear and some is crowded, rather than every field equally dotted.
const WEATHER_CELL: u32 = 8u;
// The bilinear smoothstep value noise's deviation about its 0.5 mean, from its formula.
const WEATHER_SIGMA: f32 = 0.2145;
const WEATHER_CLEAR: f32 = 0.35;
const WEATHER_CROWDED: f32 = 0.75;

// How much of a layer's field shows at lattice point `p`, its own per `salt`.
// The base is split on the coarse lattice, so the weather stays on its ground
// as the origin moves.
fn weather(p: vec2f, salt: u32) -> f32 {
  let base = atmos_cloud_base();
  let at = (vec2f(base % vec2u(WEATHER_CELL)) + p) / f32(WEATHER_CELL);
  let v = lattice_noise(base / WEATHER_CELL, at, 0xffffffffu, salt);
  let even = 1.0 / (1.0 + exp(-LOGISTIC_SCALE * (v - 0.5) / WEATHER_SIGMA));
  return smoothstep(WEATHER_CLEAR, WEATHER_CROWDED, even);
}

// The thin layers over the cumulus, each one sheet read once where a ray meets
// it: altocumulus' small heaps at 4 to 5 km, cirrus' veil at 6 to 12.
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
  // Linear light times opacity, through the air to it.
  color: vec3f,
  opacity: f32,
  // How far along the ray, in km.
  t: f32,
};

// Where `dir` meets the sphere `km` over the ground: the way out from under it,
// the way in from over it.
fn sheet_t(r0: f32, mu: f32, km: f32) -> f32 {
  let r = atmos_bottom_radius() + km;
  let at = ray_sphere(r0, mu, r);
  return select(at.y, at.x, r0 > r);
}

fn sheet(s: Sheet, dir: vec3f, t: f32, footprint: f32, lit: vec3f, sky_light: vec3f, reach: f32) -> Layer {
  var out: Layer;
  out.t = t;
  if (t <= 0.0 || t > reach) {
    return out;
  }
  let world = frame_camera_pos() + dir * (t / atmos_km_per_u());
  let p = cloud_lattice(world);
  // Read a whole number of units off, so each layer has its own clouds.
  let field = cloud_opacity_finer(p + vec2f(f32(s.salt % CLOUD_TILE_UNITS)), footprint * s.softer, s.finer);
  out.opacity = s.opacity * field * weather(p, s.salt) * (1.0 - smoothstep(0.6, 1.0, t / reach));
  out.color = aerial(CLOUD_ALBEDO * lit, world, sky_light) * out.opacity;
  return out;
}

// `near` laid over `far`.
fn over_layer(near: Layer, far: Layer) -> Layer {
  var out: Layer;
  out.color = near.color + (1.0 - near.opacity) * far.color;
  out.opacity = near.opacity + (1.0 - near.opacity) * far.opacity;
  out.t = near.t;
  return out;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let dir = normalize(clouds_ray() * vec3f(v.ndc, 1.0));
  let r0 = camera_r();
  let mu = dot(dir, frame_up());
  let reach = CLOUD_REACH_KM + 3.0 * max(atmos_camera_altitude() - atmos_cloud_altitude_km(), 0.0);

  // The slab between the cumulus' base and its top: the stretch of the ray
  // inside the top's sphere and out of the base's.
  let base = atmos_bottom_radius() + atmos_cloud_altitude_km();
  let under = ray_sphere(r0, mu, base);
  let over = ray_sphere(r0, mu, base + CLOUD_THICK_KM);
  var t0 = max(over.x, 0.0);
  var t1 = select(over.y, under.x, under.x > 0.0);
  if (r0 < base) {
    t0 = under.y;
    t1 = over.y;
  }
  t1 = min(t1, reach);
  let alto_t = sheet_t(r0, mu, ALTOCUMULUS.km);
  let cirrus_t = sheet_t(r0, mu, CIRRUS.km);

  // Derivatives, so taken before any branch.
  let footprint = max(length(fwidth(cloud_lattice(frame_camera_pos() + dir * (t0 / atmos_km_per_u())))), 1e-6);
  let alto_footprint = max(length(fwidth(cloud_lattice(frame_camera_pos() + dir * (max(alto_t, 0.0) / atmos_km_per_u())))), 1e-6);
  let cirrus_footprint = max(length(fwidth(cloud_lattice(frame_camera_pos() + dir * (max(cirrus_t, 0.0) / atmos_km_per_u())))), 1e-6);

  var light = lit_by(atmos_sun_dir(), dir, clouds_sun_at(), shown());
  var sky = zenith_sky();
  if (atmos_moon_light() > 0.0) {
    light += lit_by(atmos_moon_dir(), dir, clouds_moon_at(), moon_shown());
    sky += moon_view(frame_up()) * moon_shown();
  }
  // The terrain's fill too, so a cloud is never darker than the ground under
  // it: without it night clouds were black blots.
  let sky_light = sky_fill();
  let around = sky * 0.5 + sky_light / PI;

  // The cumulus, marched. A column holds the flat layer's optical depth, risen as
  // high as it is dense, so a full cloud fills the slab and a thin one stays near the base.
  var low: Layer;
  low.t = t0;
  if (t1 > t0 && t1 > 0.0) {
    let extinction = CLOUD_DEPTH / CLOUD_THICK_KM;
    let dt = (t1 - t0) / f32(CLOUD_STEPS);
    let start = step_offset(v.clip.xy);
    var through = 1.0;
    var color = vec3f(0.0);
    var seen_at = 0.0;
    for (var i = 0u; i < CLOUD_STEPS; i++) {
      let t = t0 + (f32(i) + start) * dt;
      let r = sqrt(t * t + 2.0 * r0 * mu * t + r0 * r0);
      let height = clamp((r - base) / CLOUD_THICK_KM, 0.0, 1.0);
      let p = cloud_lattice(frame_camera_pos() + dir * (t / atmos_km_per_u()));
      let opacity = cloud_opacity(p, footprint) * weather(p, 0x0c0cu);
      let reaches = -log(max(1.0 - opacity, 1e-4)) / CLOUD_DEPTH;
      let density = extinction * clamp((reaches - height) / CLOUD_EDGE, 0.0, 1.0);
      if (density <= 0.0) {
        continue;
      }
      // Shaded by the cloud over it, as deep as the flat layer's underside was.
      let above = extinction * max(reaches - height, 0.0) * CLOUD_THICK_KM;
      let lit = light * mix(0.3, 1.0, exp(-above)) + around;
      let taken = through * (1.0 - exp(-density * dt));
      color += taken * CLOUD_ALBEDO * lit;
      seen_at += taken * t;
      through *= exp(-density * dt);
      if (through < 0.01) {
        break;
      }
    }
    let opacity = 1.0 - through;
    if (opacity > 0.002) {
      low.opacity = opacity * (1.0 - smoothstep(0.6, 1.0, t0 / reach));
      let world = frame_camera_pos() + dir * (seen_at / opacity / atmos_km_per_u());
      low.color = aerial(color / opacity, world, sky_light) * low.opacity;
    }
  }

  let alto = sheet(ALTOCUMULUS, dir, alto_t, alto_footprint, light + around, sky_light, reach);
  let high = sheet(CIRRUS, dir, cirrus_t, cirrus_footprint, light + around, sky_light, reach);
  // Three by their distance along the ray, nearest over.
  var a = low;
  var b = alto;
  var c = high;
  if (b.t < a.t) { let x = a; a = b; b = x; }
  if (c.t < b.t) { let x = b; b = c; c = x; }
  if (b.t < a.t) { let x = a; a = b; b = x; }
  let all = over_layer(a, over_layer(b, c));
  let alpha = all.opacity * cloud_shown();
  if (alpha <= 0.002) {
    discard;
  }
  // Times its opacity, as the composite filters it (cloud_composite.wgsl).
  return vec4f(finish(all.color / all.opacity, v.clip.xy) * alpha, alpha);
}
