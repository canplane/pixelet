// The lights through the air and what they light, for the lights as the words
// say them or as a light raised a little (`Lights`): every lit pass's, the
// exposure's meter, and the sky view's slope.

// The sun's and the moon's directions, which the sky is drawn for and which
// cast, and the moon's color; the sun's is white.
struct Lights {
  sun: vec3f,
  moon: vec3f,
  moon_color: vec3f,
};

fn lights_now() -> Lights {
  return Lights(atmos_sun_dir(), atmos_moon_dir(), atmos_moon_color() * atmos_white());
}

// `d` raised by `h` in its cosine over `up`, its azimuth kept.
fn raised_dir(d: vec3f, up: vec3f, h: f32) -> vec3f {
  let mu = dot(d, up);
  let across = d - mu * up;
  let m = clamp(mu + h, -1.0, 1.0);
  return normalize(across / max(length(across), 1e-12) * sqrt(max(1.0 - m * m, 0.0)) + up * m);
}

// The sun, or the moon, raised by `h`, where it is drawn and where it casts.
fn lights_raised(s: Lights, moon: bool, h: f32) -> Lights {
  let to = raised_dir(select(s.sun, s.moon, moon), frame_up(), h);
  var r = s;
  r.sun = select(to, s.sun, moon);
  r.moon = select(s.moon, to, moon);
  return r;
}

// Each light through the air above a point, and the sky's on an up-facing face there.
struct Light {
  sun: vec3f,
  other: vec3f,
  fill: vec3f,
};

// Before the eye: what the exposure meters.
fn light_unexposed_in(r: f32, up: vec3f, s: Lights) -> Light {
  let sky = sky_ground(r, dot(up, s.sun)).rgb + sky_ground(r, dot(up, s.moon)).rgb * atmos_moon_light();
  return Light(
    atmos_white() * sun_transmittance(r, dot(up, s.sun)),
    s.moon_color * sun_transmittance(r, dot(up, s.moon)),
    (sky + atmos_night_sky()) * atmos_white(),
  );
}

fn light_unexposed(r: f32, up: vec3f) -> Light {
  return light_unexposed_in(r, up, lights_now());
}

// The ground under a down-facing face, at the Earth's mean albedo.
fn ground_bounce_in(up: vec3f, l: Light, s: Lights) -> vec3f {
  return atmos_ground_albedo() * (l.sun * max(dot(up, s.sun), 0.0) + l.other * max(dot(up, s.moon), 0.0) + l.fill);
}

// A matte face of albedo one, over pi, Lambert as three's standard material at
// roughness 1: every lit pass's, and the Earth's past the terrain. Each of
// its lights is darkened by its own visibility alone: `lit`, the share of the
// sun and of the moon the face's shadow lets reach it (shade.wgsl `shade`
// off shadow.wgsl `light_lit`), their direct light's; `open`, how open the face
// is to the sky (the screen's occlusion and its corners'), the light from the
// sky and its bounce, and the share that reaches every face alike. Each is 1
// where nothing casts or closes in.
fn surface_light_in(n: vec3f, up: vec3f, lit: vec2f, open: f32, l: Light, s: Lights) -> vec3f {
  // Turned from a light, a face reads no shadow map for it.
  let shaded = select(vec2f(1.0), mix(vec2f(1.0), lit, SHADOW_ALIKE_TAKEN), vec2<bool>(dot(n, s.sun) > 0.0, dot(n, s.moon) > 0.0));
  let alike = (l.sun * max(dot(up, s.sun), 0.0) * shaded.x + l.other * max(dot(up, s.moon), 0.0) * shaded.y + l.fill) * open;
  let hemi = mix(ground_bounce_in(up, l, s), l.fill, dot(n, up) * 0.5 + 0.5) * open;
  let directed = l.sun * max(dot(n, s.sun), 0.0) * lit.x + l.other * max(dot(n, s.moon), 0.0) * lit.y + hemi;
  return mix(alike, directed, LIGHT_DIRECTED) / PI;
}

// Where nothing casts: the Earth's picture, the clouds.
fn surface_light(n: vec3f, up: vec3f, l: Light) -> vec3f {
  return surface_light_in(n, up, vec2f(1.0), 1.0, l, lights_now());
}

/* the eye */

fn noon() -> f32 {
  let r = atmos_bottom_radius();
  return dot((sun_transmittance(r, 1.0) + sky_ground(r, 1.0).rgb) * atmos_white(), LUMA) / PI;
}

// The eye, adapted to the light on the land under it: an up-facing face of
// albedo one, against the same at sea level under a sun overhead. A fall from
// noon shows as SHOWN_FALL of it down to NIGHT_STOPS under; the eye takes back
// the rest. In stops, which an opened night eye needs.
fn eye_stops(s: Lights) -> f32 {
  let up = frame_up();
  let land = dot(surface_light_in(up, up, vec2f(1.0), 1.0, light_unexposed_in(land_radius(), up, s), s), LUMA);
  let stops = log2(max(land, 1e-30) / noon());
  let shown = clamp(stops * atmos_shown_fall(), -atmos_night_stops(), 0.0);
  return log2(atmos_exposure()) + shown - stops;
}

/* glare */

// The sun's glare along `dir`, exposed by `e`: the sun toward `sun`, at height
// `mu` over the eye, as the air above the eye lets it through.
fn glare_toward(dir: vec3f, sun: vec3f, mu: f32, e: f32) -> vec3f {
  let theta = degrees(atan2(length(cross(dir, sun)), dot(dir, sun))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return sun_transmittance(camera_r(), mu) * atmos_white() * e * psf * (LENS_GLARE_VEIL / LENS_GLARE_HALO_SHARE);
}

/* the sun's disc */

// The disc's center seen at height `mu` from radius `r`, exposed by `e`.
fn disc_light(r: f32, mu: f32, e: f32) -> vec3f {
  return transmittance_to_top(r, mu) * atmos_white() * e * SKY_SUN_DISC_GAIN;
}
