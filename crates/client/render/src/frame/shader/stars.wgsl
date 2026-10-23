// Yale Bright Star Catalog stars as quads at infinity, dimmed by the air. Added
// after the encode, which a point this small cannot tell apart.

struct Stars {
  view_proj: mat4x4f,
  sky: mat4x4f,
  // Sized in CSS pixels so a dense screen does not shrink them.
  density: f32,
  intensity: f32,
  free: vec2f,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process. The viewport and the up are the frame's,
// the Moon the air's.
@group(1) @binding(0) var<uniform> stars_words: array<vec4f, STARS_VEC4S>;

fn stars_view_proj() -> mat4x4f { return mat4x4f(stars_words[0], stars_words[1], stars_words[2], stars_words[3]); }
fn stars_sky() -> mat4x4f { return mat4x4f(stars_words[4], stars_words[5], stars_words[6], stars_words[7]); }
fn stars_density() -> f32 { return stars_words[8].x; }
fn stars_intensity() -> f32 { return stars_words[8].y; }

@group(1) @binding(1) var<storage, read> catalog: array<vec4f>;

// In CSS pixels: a point a pixel wide reads as a speck the eye passes over.
const STAR_GAIN: f32 = 1.5;
const STAR_SIGMA_PX: f32 = 0.85;
const STAR_SWELL_PX: f32 = 0.6;
// A faint wide halo shows which star is brighter once both cores are white.
const HALO_FROM: f32 = 0.398;
const HALO_SIGMA: f32 = 4.0;
const HALO_GAIN: f32 = 0.06;
// Below one 8-bit step.
const STAR_FLOOR: f32 = 0.002;

struct StarOut {
  @builtin(position) clip: vec4f,
  @location(0) at: vec2f,
  @location(1) light: vec3f,
  @location(2) sigma: f32,
  @location(3) halo: f32,
};

// Kasten and Young 1989, finite at the horizon where 1/sin is not.
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
  // No twinkle: the frame redraws only on motion, so a twinkle would become a
  // blink, and redrawing every frame for it costs the device.
  let height = dot(dir, frame_up());
  // The air straight up from the eye, from orbit none.
  let depth = depth_to_top(camera_r(), 1.0);
  let light = color * flux * STAR_GAIN * exp(-depth * airmass(height)) * stars_intensity();
  var clip = stars_view_proj() * vec4f(dir, 0.0);
  let behind_moon = length(cross(dir, atmos_moon_dir())) < MOON_ANGULAR_RADIUS * DISC_SCALE && dot(dir, atmos_moon_dir()) > 0.0;
  if (falls_to_land(dir) || behind_moon || clip.w <= 0.0 || max(light.r, max(light.g, light.b)) < STAR_FLOOR) {
    o.clip = vec4f(0.0, 0.0, 2.0, 1.0);
    return o;
  }
  let sigma = (STAR_SIGMA_PX + STAR_SWELL_PX * sqrt(max(flux - 1.0, 0.0))) * stars_density();
  let halo = select(0.0, HALO_GAIN * sqrt(flux / HALO_FROM), flux > HALO_FROM);
  let corner = vec2f(f32(v & 1u), f32(v >> 1u)) * 2.0 - 1.0;
  let half = 3.0 * sigma * select(1.0, HALO_SIGMA, halo > 0.0);
  clip.z = 0.0;
  clip = vec4f(clip.xy + corner * half * 2.0 / frame_viewport() * clip.w, clip.z, clip.w);
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
