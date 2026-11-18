// The lights through the air and what they light, either for the lights as the
// words give them or with one light raised a little (`Lighting`). Every lit pass
// reads it, and so do the exposure's meter and the sky view's slopes.

// The sun's and the moon's directions, for which the sky is drawn and from which
// they cast, and the moon's color. The sun's color is white.
struct Lighting {
  sun: vec3f,
  moon: vec3f,
  moon_color: vec3f,
};

fn lights_now() -> Lighting {
  return Lighting(atmos_sun_dir(), atmos_moon_dir(), atmos_moon_color() * atmos_white());
}

// Direction `d` with its cosine over `up` raised by `h` and its azimuth kept.
fn raised_dir(d: vec3f, up: vec3f, h: f32) -> vec3f {
  let mu = dot(d, up);
  let across = d - mu * up;
  let m = clamp(mu + h, -1.0, 1.0);
  return normalize(across / max(length(across), 1e-12) * sqrt(max(1.0 - m * m, 0.0)) + up * m);
}

// The lights with the sun, or the moon where `moon`, raised by `h`, both where
// it is drawn and where it casts.
fn lights_raised(s: Lighting, moon: bool, h: f32) -> Lighting {
  let to = raised_dir(select(s.sun, s.moon, moon), frame_up(), h);
  return Lighting(select(to, s.sun, moon), select(s.moon, to, moon), s.moon_color);
}

// The light of the sun and of the moon (`other`) through the air above a point,
// and the sky's light on an up-facing face there (`fill`).
struct Light {
  sun: vec3f,
  other: vec3f,
  fill: vec3f,
};

// The light at radius `r` before the eye's exposure, which is what the exposure
// meters. The moon's sky is added only where `moon_sky`, and its table is not
// read elsewhere. The moon's direct light is added everywhere.
fn light_unexposed_in(r: f32, up: vec3f, s: Lighting, moon_sky: bool) -> Light {
  var sky = sky_ground(r, dot(up, s.sun)).rgb;
  if (moon_sky) {
    sky += sky_ground(r, dot(up, s.moon)).rgb * atmos_moon_light();
  }
  return Light(
    atmos_white() * sun_transmittance(r, dot(up, s.sun)),
    s.moon_color * sun_transmittance(r, dot(up, s.moon)),
    (sky + NIGHT_SKY) * atmos_white(),
  );
}

// The light at a point, with the moon's sky added only where the moon reaches
// the air (atmos_medium.wgsl `reaches`), as the moon's air is.
fn light_unexposed(r: f32, up: vec3f) -> Light {
  return light_unexposed_in(r, up, lights_now(), reaches(1u));
}

// The light the ground bounces up to a down-facing face, at the Earth's mean
// albedo.
fn ground_bounce_in(up: vec3f, l: Light, s: Lighting) -> vec3f {
  return GROUND_ALBEDO * (l.sun * max(dot(up, s.sun), 0.0) + l.other * max(dot(up, s.moon), 0.0) + l.fill);
}

// The light on a matte face of albedo one, divided by pi: Lambert, as three.js's
// standard material at roughness 1. Every lit pass reads it, and so does the
// imagery past the terrain. Each light is darkened by its own visibility alone.
// `lit` is the share of the sun and of the moon that the face's shadow lets
// reach it (shade.wgsl `shade`, from shadow.wgsl `shadow_visibility`), and it
// darkens their direct light. `ao` is how open the face is to the sky, from the
// screen's occlusion and the corners', and it darkens the sky's light, its
// bounce, and the share that reaches every face alike. Each is 1 where nothing
// casts a shadow or closes in.
fn surface_light_in(n: vec3f, up: vec3f, lit: vec2f, ao: f32, l: Light, s: Lighting) -> vec3f {
  // A face turned away from a light reads no shadow map for it, and the 0 its
  // `lit` then holds is no shadow, so the share that reaches every face alike
  // keeps that light whole.
  let shaded = select(vec2f(1.0), mix(vec2f(1.0), lit, SHADOW_BY_UP_TAKEN), vec2<bool>(dot(n, s.sun) > 0.0, dot(n, s.moon) > 0.0));
  let by_up = (l.sun * max(dot(up, s.sun), 0.0) * shaded.x + l.other * max(dot(up, s.moon), 0.0) * shaded.y + l.fill) * ao;
  let hemi = mix(ground_bounce_in(up, l, s), l.fill, dot(n, up) * 0.5 + 0.5) * ao;
  let by_normal = l.sun * max(dot(n, s.sun), 0.0) * lit.x + l.other * max(dot(n, s.moon), 0.0) * lit.y + hemi;
  return mix(by_up, by_normal, LIGHT_BY_NORMAL) / PI;
}

// The share of the light from `l` that a face mirrors toward `v`, as three.js's
// `BRDF_GGX` computes it: Schlick's Fresnel from `f0`, the height-correlated
// Smith visibility, and the GGX distribution.
fn ggx(l: vec3f, v: vec3f, n: vec3f, f0: vec3f, roughness: f32) -> vec3f {
  let a2 = pow(roughness, 4.0);
  let h = normalize(l + v);
  let nl = max(dot(n, l), 0.0);
  let nv = max(dot(n, v), 0.0);
  let nh = max(dot(n, h), 0.0);
  let vh = max(dot(v, h), 0.0);
  let fresnel = exp2((-5.55473 * vh - 6.98316) * vh);
  let f = f0 * (1.0 - fresnel) + fresnel;
  let gv = nl * sqrt(a2 + (1.0 - a2) * nv * nv);
  let gl = nv * sqrt(a2 + (1.0 - a2) * nl * nl);
  let d = nh * nh * (a2 - 1.0) + 1.0;
  return f * 0.5 / max(gv + gl, 1e-6) * a2 / (PI * d * d);
}

// What a glossy face mirrors toward `v` of the direct lights that
// `surface_light_in` scatters. Each is weighed by the face's normal and
// darkened by its own visibility `lit`, as there. The sky's light is not
// mirrored, as no image of the sky is held here to mirror.
fn surface_specular_in(n: vec3f, v: vec3f, f0: vec3f, roughness: f32, lit: vec2f, l: Light, s: Lighting) -> vec3f {
  return l.sun * max(dot(n, s.sun), 0.0) * lit.x * ggx(s.sun, v, n, f0, roughness) + l.other * max(dot(n, s.moon), 0.0) * lit.y * ggx(s.moon, v, n, f0, roughness);
}

// The light on a face where nothing casts a shadow or closes in: the Earth's
// image and the clouds.
fn surface_light(n: vec3f, up: vec3f, l: Light) -> vec3f {
  return surface_light_in(n, up, vec2f(1.0), 1.0, l, lights_now());
}

fn noon() -> f32 {
  let r = BOTTOM_RADIUS_KM;
  return dot((sun_transmittance(r, 1.0) + sky_ground(r, 1.0).rgb) * atmos_white(), LUMA) / PI;
}

// The eye's exposure in stops, adapted to the light on the land under it at
// radius `land_r`. The land is measured as an up-facing face of albedo one,
// against the same face at sea level under a sun overhead (`noon`). A fall in
// light from noon shows as `SHOWN_FALL` of itself, down to
// `NIGHT_STOPS` under noon, and the eye takes back the rest.
fn eye_stops_at(land_r: f32, s: Lighting) -> f32 {
  let up = frame_up();
  let land = dot(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_r, up, s, true), s), LUMA);
  let stops = log2(max(land, 1e-30) / noon());
  let shown = clamp(stops * SHOWN_FALL, -NIGHT_STOPS, 0.0);
  return log2(EXPOSURE) + shown - stops;
}

fn eye_stops(s: Lighting) -> f32 {
  return eye_stops_at(land_radius(), s);
}

// The sun's glare seen along `dir`, exposed by `e`, for the sun toward `sun` at
// height `mu` over an eye at radius `r`, as the air above that eye lets it
// through.
fn glare_toward(r: f32, dir: vec3f, sun: vec3f, mu: f32, e: f32) -> vec3f {
  let theta = degrees(atan2(length(cross(dir, sun)), dot(dir, sun))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return sun_transmittance(r, mu) * atmos_white() * e * psf * (LENS_GLARE_VEIL / LENS_GLARE_HALO_SHARE);
}

// The disc's center seen at `mu` from radius `r`, exposed by `e`.
fn disc_light(r: f32, mu: f32, e: f32) -> vec3f {
  return transmittance_to_top(r, mu) * atmos_white() * e * SKY_SUN_DISC_GAIN;
}
