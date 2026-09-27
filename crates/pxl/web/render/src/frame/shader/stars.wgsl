// The stars: the Yale Bright Star Catalog (pxl-tool/bin/bake_stars.rs), every
// star an eye sees on a dark night, each a quad a few pixels wide at
// infinity, drawn over the sky and under everything else. A star's place is
// the catalog's turned by the stars' frame — the sidereal time and the
// latitude (look/environment.rs) — and drawn through the root frame's
// matrix, which turns it with the map; its brightness is
// its magnitude, its color its temperature, both dimmed by the air it is
// seen through. Added to the sky, which is already encoded: a point this
// small reads the same added either side of the encode.

struct Stars {
  view_proj: mat4x4f,
  // The equatorial frame to the map's, in its upper three columns.
  sky: mat4x4f,
  viewport: vec2f,
  intensity: f32,
  _p0: f32,
  // The air's optical depth overhead, by channel, from the camera's
  // altitude: what an airmass of one takes off a star.
  depth: vec3f,
  // Device pixels to a CSS pixel: a star's size is the page's, so a dense
  // screen does not shrink it to a fraction of what the eye is shown.
  density: f32,
  // Straight up where the eye stands (atmos_medium.wgsl's `up`): a star's
  // height, and the air it is seen through, are measured from it.
  up: vec3f,
  _p1: f32,
};

// Held as vec4 words, each field read from its own by `stars_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(0) @binding(0) var<uniform> stars_words: array<vec4f, 11>;

fn stars_view_proj() -> mat4x4f { return mat4x4f(stars_words[0], stars_words[1], stars_words[2], stars_words[3]); }
fn stars_sky() -> mat4x4f { return mat4x4f(stars_words[4], stars_words[5], stars_words[6], stars_words[7]); }
fn stars_viewport() -> vec2f { return stars_words[8].xy; }
fn stars_intensity() -> f32 { return stars_words[8].z; }
fn stars__p0() -> f32 { return stars_words[8].w; }
fn stars_depth() -> vec3f { return stars_words[9].xyz; }
fn stars_density() -> f32 { return stars_words[9].w; }
fn stars_up() -> vec3f { return stars_words[10].xyz; }
fn stars__p1() -> f32 { return stars_words[10].w; }

// Each star as gpu/stars.rs took it once: its direction in the equatorial
// frame and its flux, then its color.
@group(0) @binding(1) var<storage, read> catalog: array<vec4f>;

// A magnitude-0 star's peak, and how a brighter one swells rather than
// clipping: a photograph's and an eye's bright star is a larger point. In
// CSS pixels, as a planetarium draws them: a point a pixel wide would read
// as a speck the eye passes over.
const STAR_GAIN: f32 = 1.5;
const STAR_SIGMA_PX: f32 = 0.85;
const STAR_SWELL_PX: f32 = 0.6;
// The glare round a star brighter than first magnitude: a wide, faint
// second gaussian, which is how a screen with an eight-bit range says one
// star outshines another when their cores have both reached white — the
// bright stars a constellation is drawn from then read from across the sky.
const HALO_FROM: f32 = 0.398;
const HALO_SIGMA: f32 = 4.0;
const HALO_GAIN: f32 = 0.06;
// Below this the star is under an eight-bit step and is not drawn.
const STAR_FLOOR: f32 = 0.002;

struct StarOut {
  @builtin(position) clip: vec4f,
  @location(0) at: vec2f,
  @location(1) light: vec3f,
  @location(2) sigma: f32,
  @location(3) halo: f32,
};

// The air a star is seen through, as a share of the air overhead (Kasten
// and Young 1989), which stays finite at the horizon where 1/sin does not.
fn airmass(sin_alt: f32) -> f32 {
  let alt = degrees(asin(clamp(sin_alt, 0.0, 1.0)));
  return 1.0 / (max(sin_alt, 0.0) + 0.50572 * pow(alt + 6.07995, -1.6364));
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> StarOut {
  let at = catalog[2u * i];
  let color = catalog[2u * i + 1u].rgb;
  let dir = (stars_sky() * vec4f(at.xyz, 0.0)).xyz;

  var o: StarOut;
  let flux = at.w;
  // No scintillation: a twinkle is motion, and the frame is drawn only when
  // something moves — at rest every two seconds, which turns a twinkle into
  // a blink — and drawing every frame for it is the device's to pay.
  let height = dot(dir, stars_up());
  let light = color * flux * STAR_GAIN * exp(-stars_depth() * airmass(height)) * stars_intensity();
  var clip = stars_view_proj() * vec4f(dir, 0.0);
  if (height < -0.02 || clip.w <= 0.0 || max(light.r, max(light.g, light.b)) < STAR_FLOOR) {
    o.clip = vec4f(0.0, 0.0, 2.0, 1.0);
    return o;
  }
  let sigma = (STAR_SIGMA_PX + STAR_SWELL_PX * sqrt(max(flux - 1.0, 0.0))) * stars_density();
  let halo = select(0.0, HALO_GAIN * sqrt(flux / HALO_FROM), flux > HALO_FROM);
  let corner = vec2f(f32(v & 1u), f32(v >> 1u)) * 2.0 - 1.0;
  let half = 3.0 * sigma * select(1.0, HALO_SIGMA, halo > 0.0);
  clip.z = 0.0;
  clip = vec4f(clip.xy + corner * half * 2.0 / stars_viewport() * clip.w, clip.z, clip.w);
  o.clip = clip;
  o.at = corner * half;
  o.light = min(light, vec3f(4.0));
  o.sigma = sigma;
  o.halo = halo;
  return o;
}

@fragment
fn fs(i: StarOut) -> @location(0) vec4f {
  let r2 = dot(i.at, i.at);
  let halo_sigma = i.sigma * HALO_SIGMA;
  let a = exp(-r2 / (2.0 * i.sigma * i.sigma)) + i.halo * exp(-r2 / (2.0 * halo_sigma * halo_sigma));
  return vec4f(to_srgb(i.light * a), 0.0);
}
