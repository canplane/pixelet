// Draws Hillaire's sky-view table, once for the sun and once for the moon, each
// laid about its light's azimuth (atmos_medium.wgsl `view_layout`). A table is
// drawn together with its light's share of the air's volume
// (atmos_froxel.wgsl). When they are to be measured again, the drawing also
// measures its light's slopes (`slopes`).

// `light.x` names the light, 1 for the moon. `light.y` is 1 when this drawing
// measures the slope in the light's mu, and `light.z` is 1 when it
// measures the slope in the eye's altitude (pass/atmosphere.rs `prepare`).
@group(0) @binding(14) var<uniform> light: vec4f;

// What the views and the exposure measure (frame/shader/word.rs
// `SLOPE_WORDS`): `LIGHT_SLOPES` words for each light, the sun's first, then
// two for the ground, its slope and its sample (atmos_exposure.wgsl). A
// light's first word is the most display levels per unit of its mu that
// any texel of its sky with the sun's glare over it, the sun's disc, or the
// land's lit face under the eye moves, with the exposure moving along. The
// second is the same per km of the eye's altitude, with the land and the
// exposure standing still. The third is the mu's sample, over which the
// first is taken. The fourth is the most levels the light's sky shows, and the
// fifth their slope in its mu (atmos_exposure.wgsl `reach`). Each slope is
// a local linearization, trusted over its one sample alone: for the light, one
// step of the transmittance table's own sampling where the light stands; for
// the eye, `ATMOS_ALT_SAMPLE` of its altitude, and `ATMOS_ALT_SAMPLE_LEAST_KM`
// at the least. A move over the sample counts as one level at least, so the
// step a reader takes from a slope (1 / slope) never passes its sample. Each
// slope is measured again on its own, once its quantity has left the sample it
// was taken over. The words hold the floats' bits, which `atomicMax` orders
// as it would the positive floats themselves.
@group(0) @binding(20) var<storage, read_write> slopes: array<atomic<u32>, SLOPE_WORDS>;

// The eye's altitude sample, in km (`slopes`).
fn alt_sample() -> f32 {
  return max(atmos_camera_alt() * ATMOS_ALT_SAMPLE, ATMOS_ALT_SAMPLE_LEAST_KM);
}

// The texel's sky along `dir` as shown, exposed by `e`, from the `radiance`
// drawn for light `k` at `mu` from radius `r`, with the sun's glare
// over it.
fn shown_sky(r: f32, dir: vec3f, mu: f32, k: u32, radiance: vec3f, e: f32) -> vec3f {
  let sun = select(1.0, 0.0, k == 1u);
  return radiance * atmos_white() * light_gain(k) * e + glare_toward(r, dir, view_light(mu), mu, e) * sun;
}

// Measures the texel's slope in its light's mu: how far its sky, the
// sun's disc and the land's lit face under the eye move as the light rises by
// its sample.
fn measure_light(r: f32, dir: vec3f, mu: f32, k: u32, radiance: vec3f) {
  let h = mu_step(r, mu);
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
  let land = abs(levels(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_r, up, raised, true), raised) * e1) - levels(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_r, up, now, true), now) * e0));
  if (h > 0.0) {
    atomicMax(&slopes[LIGHT_SLOPES * k], bitcast<u32>(max(most(max(max(abs(lit1 - lit0), disc), land)), 1.0) / h));
    atomicMax(&slopes[LIGHT_SLOPES * k + 2u], bitcast<u32>(h));
  }
}

// Measures the texel's slope in the eye's altitude: how far its sky along the
// same direction, and the glare and the disc through the air over the eye,
// move as the eye rises by its sample. The land's light, and the eye's stops
// set by it, stay as they are.
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
  let dir = view_dir(view_layout(k).x, vec2f(unit_of(uv.x, SKY_VIEW_SIZE.x), unit_of(uv.y, SKY_VIEW_SIZE.y)));
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
