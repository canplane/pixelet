// Yale Bright Star Catalog stars as quads at infinity, dimmed by the air.
// Added untoned to the world's color, which a point this small cannot tell
// apart: over a dark sky the encode shows it as it shows the star's own light.

// Its words' fields and accessors are frame/shader/word.rs `stars`'s, which
// build.rs writes here. The viewport and the up are the frame's, the Moon
// the air's.
@group(1) @binding(0) var<uniform> stars_words: array<vec4f, STARS_VEC4S>;

@group(1) @binding(1) var<storage, read> catalog: array<vec4f>;

struct StarOut {
  @builtin(position) clip: vec4f,
  @location(0) at: vec2f,
  @location(1) light: vec3f,
  @location(2) sigma: f32,
  @location(3) halo: f32,
};

// The flux a star of `SKY_STAR_HALO_MAG` has, by the law the catalog's flux is
// read off its magnitude by (gpu/star.rs `star_floats`).
const HALO_FROM: f32 = pow(10.0, -0.4 * SKY_STAR_HALO_MAG);

// Kasten and Young 1989, finite at the horizon where 1/sin is not.
fn airmass(sin_alt: f32) -> f32 {
  let alt = degrees(asin(clamp(sin_alt, 0.0, 1.0)));
  return 1.0 / (max(sin_alt, 0.0) + 0.50572 * pow(alt + 6.07995, -1.6364));
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> StarOut {
  let at = catalog[STAR_RECORD_VEC4S * i];
  let color = catalog[STAR_RECORD_VEC4S * i + 1u].rgb;
  let dir = (stars_sky() * vec4f(at.xyz, 0.0)).xyz;

  var o: StarOut;
  let flux = at.w;
  // No twinkle: the frame redraws only on motion, so a twinkle would become a
  // blink, and redrawing every frame for it costs the device.
  let height = dot(dir, frame_up());
  // The air straight up from the eye, from orbit none.
  let depth = depth_to_top(camera_r(), 1.0);
  let light = color * flux * SKY_STAR_GAIN * exp(-depth * airmass(height)) * stars_intensity();
  var clip = stars_view_proj() * vec4f(dir, 0.0);
  let behind_moon = length(cross(dir, atmos_moon_dir())) < MOON_ANGULAR_RADIUS * SKY_DISC_SCALE && dot(dir, atmos_moon_dir()) > 0.0;
  if (falls_to_land(dir) || behind_moon || clip.w <= 0.0 || max(light.r, max(light.g, light.b)) < SKY_STAR_FLOOR) {
    o.clip = vec4f(0.0, 0.0, 2.0, 1.0);
    return o;
  }
  let sigma = (SKY_STAR_SIGMA_PX + SKY_STAR_SWELL_PX * sqrt(max(flux - 1.0, 0.0))) * stars_density();
  let halo = select(0.0, SKY_STAR_HALO_GAIN * sqrt(flux / HALO_FROM), flux > HALO_FROM);
  let corner = vec2f(f32(v & 1u), f32(v >> 1u)) * 2.0 - 1.0;
  let half = SKY_STAR_CUTOFF_SIGMAS * sigma * select(1.0, SKY_STAR_HALO_SIGMA, halo > 0.0);
  clip.z = 0.0;
  clip = vec4f(clip.xy + corner * half * 2.0 / frame_viewport() * clip.w, clip.z, clip.w);
  o.clip = jittered(clip);
  o.at = corner * half;
  o.light = min(light, vec3f(SKY_STAR_LIGHT_MAX));
  o.sigma = sigma;
  o.halo = halo;
  return o;
}

@fragment
fn fs(i: StarOut) -> @location(0) vec4f {
  let r2 = dot(i.at, i.at);
  let halo_sigma = i.sigma * SKY_STAR_HALO_SIGMA;
  let a = exp(-r2 / (2.0 * i.sigma * i.sigma)) + i.halo * exp(-r2 / (2.0 * halo_sigma * halo_sigma));
  return vec4f(i.light * a, 0.0);
}
