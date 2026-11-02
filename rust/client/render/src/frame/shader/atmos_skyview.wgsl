// Hillaire's sky-view table, drawn once for the sun and once for the moon,
// each relative to its light's azimuth, with its light's share of the air's
// volume (atmos_froxel.wgsl), as laid (atmos_medium.wgsl `laid`); and,
// each where it is to be measured again, its light's slopes (`slopes`).

// Which light, 1 for the moon; 1 where its height's slope is measured with
// this drawing, and 1 where its altitude's is (gpu/atmosphere.rs
// `prepare`).
@group(0) @binding(14) var<uniform> light: vec4f;

// What the views and the exposure measure (frame/shader/word.rs
// `SLOPE_WORDS`), `LIGHT_SLOPES` words a light, the sun's first, then two of
// the ground's (atmos_exposure.wgsl). A light's: the most display levels per
// unit of its
// height that any texel of its sky with the sun's glare over it, the sun's
// disc, or the land's lit face under the eye moves, its exposure moving with
// it; the same per km of the eye's altitude, the land and the exposure
// standing; the height's sample, over which the first is taken; the most
// levels its sky shows, and their slope in its height (atmos_exposure.wgsl
// `reach`). Each slope a local
// linearization, trusted over its one sample alone: for the light, one step
// of the transmittance table's own sampling where the light stands; for the
// eye, `ATMOS_ALT_SAMPLE` of its altitude, `ATMOS_ALT_SAMPLE_LEAST_KM` at
// the least. Moving over it counts as a level at least, so the step a reader
// takes off a slope (1 / slope) is never past its sample, and each slope is
// measured again apart, where its own quantity has left the sample it was
// taken over.
// Their bits: a positive float's order is its bits'.
@group(0) @binding(20) var<storage, read_write> slopes: array<atomic<u32>, SLOPE_WORDS>;

// The eye's altitude sample, in km (`slopes`).
fn alt_sample() -> f32 {
  return max(atmos_camera_alt() * ATMOS_ALT_SAMPLE, ATMOS_ALT_SAMPLE_LEAST_KM);
}

// The texel's sky along `dir` as shown, its sky `radiance` drawn for light
// `k` at height `mu` from radius `r`, exposed by `e`, with the sun's glare.
fn shown_sky(r: f32, dir: vec3f, mu: f32, k: u32, radiance: vec3f, e: f32) -> vec3f {
  let sun = select(1.0, 0.0, k == 1u);
  return radiance * atmos_white() * light_gain(k) * e + glare_toward(r, dir, view_light(mu), mu, e) * sun;
}

// The texel's slope in its light's height: its sky, the sun's disc and the
// land's lit face under the eye as the light rises by its sample.
fn measure_light(r: f32, dir: vec3f, mu: f32, k: u32, radiance: vec3f) {
  let h = height_step(r, mu);
  let now = lights_now();
  let moon = k == 1u;
  let raised = lights_raised(now, moon, h);
  let e0 = exp2(eye_stops(now));
  let e1 = exp2(eye_stops(raised));
  let lit0 = levels(shown_sky(r, dir, mu, k, radiance, e0));
  let lit1 = levels(shown_sky(r, dir, mu + h, k, sky_radiance(r, dir, view_light(mu + h), ATMOS_SKY_VIEW_STEPS), e1));
  let sun = select(1.0, 0.0, moon);
  let disc = abs(levels(disc_light(r, mu + h, e1)) - levels(disc_light(r, mu, e0))) * sun;
  let up = frame_up();
  let land_r = land_radius();
  let land = abs(levels(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_r, up, raised), raised) * e1) - levels(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_r, up, now), now) * e0));
  if (h > 0.0) {
    atomicMax(&slopes[LIGHT_SLOPES * k], bitcast<u32>(max(most(max(max(abs(lit1 - lit0), disc), land)), 1.0) / h));
    atomicMax(&slopes[LIGHT_SLOPES * k + 2u], bitcast<u32>(h));
  }
}

// The texel's slope in the eye's altitude: its sky along the same direction,
// and the glare and the disc through the air over the eye, as the eye rises
// by its sample; the land's light, and the eye's stops off it, stand.
fn measure_alt(r: f32, dir: vec3f, mu: f32, k: u32, radiance: vec3f) {
  let dr = alt_sample();
  let e0 = exp2(eye_stops(lights_now()));
  let lit0 = levels(shown_sky(r, dir, mu, k, radiance, e0));
  let higher = levels(shown_sky(r + dr, dir, mu, k, sky_radiance(r + dr, dir, view_light(mu), ATMOS_SKY_VIEW_STEPS), e0));
  let sun = select(1.0, 0.0, k == 1u);
  let disc = abs(levels(disc_light(r + dr, mu, e0)) - levels(disc_light(r, mu, e0))) * sun;
  atomicMax(&slopes[LIGHT_SLOPES * k + 1u], bitcast<u32>(max(most(max(abs(higher - lit0), disc)), 1.0) / dr));
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / SKY_VIEW_SIZE;
  let r = camera_r();
  let k = u32(light.x);
  let dir = view_dir(laid(k).x, vec2f(unit_of(uv.x, SKY_VIEW_SIZE.x), unit_of(uv.y, SKY_VIEW_SIZE.y)));
  let mu = dot(light_dir(k), frame_up());
  let radiance = sky_radiance(r, dir, view_light(mu), ATMOS_SKY_VIEW_STEPS);
  if (light.y > 0.5) {
    measure_light(r, dir, mu, k, radiance);
  }
  if (light.z > 0.5) {
    measure_alt(r, dir, mu, k, radiance);
  }
  return vec4f(radiance, 1.0);
}
