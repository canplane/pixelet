// Finds the eye's exposure for the lights as the words give them. The first
// thread writes the first texel: the exposure in stops (atmos_light.wgsl
// `eye_stops`) and, beside it, the table's white, which every pass after this
// one reads there (atmos_eye.wgsl). When the ground's slope is to be measured
// again, the first thread measures it too (`ground`). The next two threads are
// the sun's and the moon's. Each measures how far its light reaches the air at
// this exposure, and how fast that changes with the light's height (`reach`).
// Everything measured goes into the views' words (atmos_skyview.wgsl
// `slopes`). This is a compute pass, a texel a thread, so its timestamps span
// its own work: a render pass's begin is stamped before the passes it waits on
// have finished (gpu/atmosphere.rs `exposure`).

// `own.y` is 1 when this drawing measures the ground's slope again
// (gpu/atmosphere.rs `prepare`).
@group(0) @binding(14) var<uniform> own: vec4f;
@group(0) @binding(1) var exposure_out: texture_storage_2d<rgba16float, write>;
// The views' words, plain rather than atomic here: each word this pass writes
// belongs to one thread alone, the ground's to the first and a light's to that
// light's thread, and the words are read only once the pass is done
// (gpu/atmosphere.rs `slopes`).
@group(0) @binding(20) var<storage, read_write> slopes: array<u32, SLOPE_WORDS>;

// How far the radius moves over one row of the transmittance table from
// radius `r`, taking the larger of the two sides. The land's light is read
// through that table.
fn radius_step(r: f32) -> f32 {
  let uv = transmittance_uv(r, 1.0);
  let texel = vec2f(0.0, 1.0 / TRANSMITTANCE_SIZE.y);
  return max(abs(transmittance_r_mu(uv + texel).x - r), abs(transmittance_r_mu(uv - texel).x - r));
}

// The land's lit face under the eye as shown, with the land at radius `r` and
// the eye exposed for that land.
fn land_shown(r: f32, s: Lights) -> vec3f {
  let up = frame_up();
  return surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(r, up, s, true), s) * exp2(eye_stops_at(r, s));
}

// Measures the exposure's slope in the height of the ground under the eye, as
// the lights' views measure theirs (atmos_skyview.wgsl `slopes`). The slope is
// the most display levels per km that the land's lit face moves, at albedos
// an octave apart from 1 down to 1/256, with the exposure moving along. It is
// taken over one row of the transmittance table where the land stands, which
// is its sample. A move over the sample counts as one level at least, so the
// step a reader takes from the slope never passes the sample.
fn ground() {
  let s = lights_now();
  let r = land_radius();
  let dr = radius_step(r);
  let lit0 = land_shown(r, s);
  let lit1 = land_shown(r + dr, s);
  var moved = 0.0;
  for (var j = 0; j <= 8; j++) {
    let albedo = exp2(-f32(j));
    moved = max(moved, most(abs(levels(lit1 * albedo) - levels(lit0 * albedo))));
  }
  if (dr > 0.0) {
    slopes[GROUND_SLOPES] = bitcast<u32>(max(moved, 1.0) / dr);
    slopes[GROUND_SLOPES + 1u] = bitcast<u32>(dr);
  }
}

// How far light `k` at height `mu` reaches the air, with the eye at `stops`:
// the most display levels its sky shows along the light's own vertical,
// toward its azimuth, where the sky about a light is brightest. It reads
// `ATMOS_REACH_ROWS` of the rows of the light's view as laid
// (atmos_medium.wgsl `view_layout`). What the air scatters before a point is
// no more than the sky along the same ray, so a light whose sky shows under
// half a level leaves its air under half a level too.
fn shows(k: u32, mu: f32, stops: f32) -> f32 {
  let r = camera_r();
  let shown_by = atmos_white() * light_gain(k) * exp2(stops);
  let last = f32(ATMOS_REACH_ROWS - 1);
  var most_shown = 0.0;
  for (var j = 0; j < ATMOS_REACH_ROWS; j++) {
    let dir = view_dir(view_layout(k).x, vec2f(0.0, f32(j) / last));
    most_shown = max(most_shown, most(levels(sky_radiance(r, dir, view_light(mu), ATMOS_SKY_VIEW_STEPS) * shown_by)));
  }
  return most_shown;
}

// Measures how far light `k` reaches the air now, and the slope of that reach
// in the light's height, as the views measure theirs (atmos_skyview.wgsl
// `slopes`). The slope is the levels moved as the light rises by its sample,
// one step of the transmittance table's own sampling where the light stands,
// with the eye's stops moving along. A move over the sample counts as one
// level at least. A light out of reach comes into reach no sooner than the
// step to `REACH` that this slope gives (gpu/atmosphere.rs `LightMemo::step`).
fn reach(k: u32) {
  let now = lights_now();
  let mu = dot(light_dir(k), frame_up());
  let h = height_step(camera_r(), mu);
  let shown0 = shows(k, mu, eye_stops(now));
  let shown1 = shows(k, mu + h, eye_stops(lights_raised(now, k == 1u, h)));
  slopes[LIGHT_SLOPES * k + 3u] = bitcast<u32>(shown0);
  slopes[LIGHT_SLOPES * k + 4u] = bitcast<u32>(max(abs(shown1 - shown0), 1.0) / max(h, 1e-6));
}

@compute @workgroup_size(3)
fn exposure_cs(@builtin(local_invocation_index) x: u32) {
  var texel = vec4f(0.0);
  if (x == 0u) {
    if (own.y > 0.5) {
      ground();
    }
    texel = vec4f(eye_stops(lights_now()), table_white());
  } else {
    reach(x - 1u);
  }
  textureStore(exposure_out, vec2u(x, 0u), texel);
}
