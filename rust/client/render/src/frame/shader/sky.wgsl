// The sky: one triangle at the far plane, drawn after the opaque world, with
// the sun's and moon's discs through the air.

@group(0) @binding(2) var moon_map: texture_2d_array<f32>;
@group(0) @binding(3) var moon_sampler: sampler;

const MOON_MAP_RADIUS_TEXELS: f32 = f32(MOON_TEXELS) / 2.0;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}

fn sun_disc(dir: vec3f, r: f32, a: f32, w: f32) -> vec3f {
  let mu = dot(dir, frame_up());
  // Behind the land the eye stands over.
  if (falls_to_land(dir) || dot(dir, atmos_sun_dir()) <= 0.0 || a >= 1.0 + w) {
    return vec3f(0.0);
  }
  let inside = min(a, 1.0);
  let limb = 1.0 - SKY_SUN_LIMB * (1.0 - sqrt(1.0 - inside * inside));
  let edge = 1.0 - smoothstep(1.0 - w, 1.0 + w, a);
  return disc_light(r, mu, exposure()) * limb * edge;
}

// Lommel-Seeliger, so a full Moon is as bright at its limb as at its center.
fn moon_disc(dir: vec3f, r: f32, across: vec3f, a: f32, w: f32) -> vec3f {
  let axis = atmos_moon_dir();
  let mu = dot(dir, frame_up());
  if (falls_to_land(dir) || dot(dir, axis) <= 0.0 || a >= 1.0 + w) {
    return vec3f(0.0);
  }
  let edge = 1.0 - smoothstep(1.0 - w, 1.0 + w, a);
  let on = across / max(a, 1.0);
  let normal = on - axis * sqrt(max(1.0 - dot(on, on), 0.0));

  // Selenographic: north toward the pole, east to the right as seen from Earth.
  let north = normalize(sky_pole() - dot(sky_pole(), axis) * axis);
  let east = cross(axis, north);
  let lon = atan2(dot(normal, east), -dot(normal, axis));
  let lat = asin(clamp(dot(normal, north), -1.0, 1.0));
  var face = MOON_MAP_MEAN;
  if (sky_mapped() > 0.5) {
    let uv = vec2f(lon / PI + 0.5, 0.5 - lat / PI);
    let lod = log2(max(MOON_MAP_RADIUS_TEXELS * w, 1e-6));
    face = textureSampleLevel(moon_map, moon_sampler, uv, 0, lod).r;
  }
  let albedo = MOON_ALBEDO * face / MOON_MAP_MEAN;

  let sun = atmos_sun_dir();
  let lit = max(dot(normal, sun), 0.0);
  let seen = max(-dot(normal, dir), 1e-4);
  let radiance = albedo / PI * 2.0 * lit / (lit + seen);
  let shown_at = atmos_white() * min(exposure(), SKY_MOON_FACE_SHOWN * PI / MOON_ALBEDO);
  var light = radiance * shown_at;
  if (atmos_moon_light() > 0.0) {
    let earth = 0.5 * (1.0 + dot(sun, axis));
    light += vec3f(SKY_MOON_EARTHSHINE_SHOWN * earth * face / MOON_MAP_MEAN);
  }
  return transmittance_to_top(r, mu) * light * edge;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let dir = normalize(sky_ray() * vec3f(v.ndc + frame_jitter(), 1.0));
  let r = camera_r();
  // Derivatives, so taken where every pixel takes them.
  let sun_a = length(cross(dir, atmos_sun_dir())) / (atmos_sun_angular_radius() * SKY_DISC_SCALE);
  let sun_w = max(fwidth(sun_a), 1e-4);
  let across = (dir - dot(dir, atmos_moon_dir()) * atmos_moon_dir()) / (MOON_ANGULAR_RADIUS * SKY_DISC_SCALE);
  let moon_a = length(across);
  let moon_w = max(fwidth(moon_a), 1e-4);
  var color = beyond(dir) + sun_disc(dir, r, sun_a, sun_w) + moon_disc(dir, r, across, moon_a, moon_w) + glare(dir);
  return vec4f(tone(color), 1.0);
}
