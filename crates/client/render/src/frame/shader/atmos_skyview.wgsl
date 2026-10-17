// Hillaire's sky-view table, drawn once for the sun and once for the moon,
// each relative to its light's azimuth.

@group(0) @binding(14) var<uniform> light: vec4f;

// Each light's slope as drawn: the most display levels per unit of its
// height that any texel of its sky with the sun's glare over it, the sun's
// disc, or the land's lit face under the eye moves, its exposure moving with
// it; the sun's first. A local linearization,
// one step of the transmittance table's own sampling where the light stands,
// trusted over that one sample alone: a light moving it counts as a level at
// least, so the step a reader takes off it (1 / slope) is never past the
// sample, and the slope is measured again there.
// Its bits: a positive float's order is its bits'.
@group(0) @binding(20) var<storage, read_write> slopes: array<atomic<u32>, 2>;

const SKY_VIEW_STEPS: i32 = 30;

// What the canvas writes of a linear color, in its levels.
fn levels(c: vec3f) -> vec3f {
  return to_srgb(c) * 255.0;
}

// How far the light's height moves over one texel of the transmittance
// table, which its light is read through, from where it stands: the larger
// side.
fn height_step(r: f32, mu: f32) -> f32 {
  let uv = transmittance_uv(r, mu);
  let texel = vec2f(1.0 / TRANSMITTANCE_SIZE.x, 0.0);
  return max(abs(transmittance_r_mu(uv + texel).y - mu), abs(transmittance_r_mu(uv - texel).y - mu));
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / SKY_VIEW_SIZE;
  let r = camera_r();
  let dir = view_dir(r, vec2f(unit_of(uv.x, SKY_VIEW_SIZE.x), unit_of(uv.y, SKY_VIEW_SIZE.y)));
  let moon = light.x > 0.5;
  let mu = dot(select(atmos_sun_dir(), atmos_moon_dir(), moon), frame_up());
  let radiance = sky_radiance(r, dir, view_light(mu), SKY_VIEW_STEPS);

  let h = height_step(r, mu);
  let now = lights_now();
  let raised = lights_raised(now, moon, h);
  let gain = atmos_white() * select(1.0, atmos_moon_light(), moon);
  let e0 = exp2(eye_stops(now));
  let e1 = exp2(eye_stops(raised));
  // The sun's own light over its sky: the glare along this texel, and the disc.
  let sun = select(1.0, 0.0, moon);
  let lit0 = radiance * gain * e0 + glare_toward(dir, view_light(mu), mu, e0) * sun;
  let lit1 = sky_radiance(r, dir, view_light(mu + h), SKY_VIEW_STEPS) * gain * e1 + glare_toward(dir, view_light(mu + h), mu + h, e1) * sun;
  let sky = abs(levels(lit1) - levels(lit0));
  let disc = abs(levels(disc_light(r, mu + h, e1)) - levels(disc_light(r, mu, e0))) * sun;
  let up = frame_up();
  let land_r = land_radius();
  let land = abs(levels(surface_light_in(up, up, light_unexposed_in(land_r, up, raised), raised) * e1) - levels(surface_light_in(up, up, light_unexposed_in(land_r, up, now), now) * e0));
  let each = max(max(sky, disc), land);
  let most = max(max(each.r, each.g), each.b);
  if (h > 0.0) {
    atomicMax(&slopes[select(0u, 1u, moon)], bitcast<u32>(max(most, 1.0) / h));
  }
  return vec4f(radiance, 1.0);
}
