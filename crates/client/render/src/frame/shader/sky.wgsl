// The sky: one triangle at the far plane, drawn after the opaque world, with
// the sun's and moon's discs through the air.

@group(0) @binding(2) var moon_map: texture_2d_array<f32>;
@group(0) @binding(3) var moon_sampler: sampler;

// Far past white once exposed, so the air in front colors it.
const SUN_DISC: f32 = 4.0;
const SUN_LIMB: f32 = 0.6;
const MOON_ALBEDO: f32 = 0.12;
// The map's mean over the disc (printed by download_moon) scales it to MOON_ALBEDO.
const MOON_MAP_RADIUS_TEXELS: f32 = 256.0;
const MOON_MAP_MEAN: f32 = 0.2459;
// The eye adapts to the Moon, so its seas show where the sky's exposure would
// drown the disc white.
const MOON_FACE_SHOWN: f32 = 0.6;
// Earthshine is ~1e-4 of the sunlit side, which no 8-bit screen holds; this is
// how it reads at night, going with the Earth's phase.
const EARTHSHINE_SHOWN: f32 = 0.03;

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
  if (meets_ground(r, mu) || dot(dir, atmos_sun_dir()) <= 0.0 || a >= 1.0 + w) {
    return vec3f(0.0);
  }
  let inside = min(a, 1.0);
  let limb = 1.0 - SUN_LIMB * (1.0 - sqrt(1.0 - inside * inside));
  let edge = 1.0 - smoothstep(1.0 - w, 1.0 + w, a);
  return transmittance_to_top(r, mu) * shown() * SUN_DISC * limb * edge;
}

// Lommel-Seeliger, so a full Moon is as bright at its limb as at its center.
fn moon_disc(dir: vec3f, r: f32, across: vec3f, a: f32, w: f32) -> vec3f {
  let axis = atmos_moon_dir();
  let mu = dot(dir, frame_up());
  if (meets_ground(r, mu) || dot(dir, axis) <= 0.0 || a >= 1.0 + w) {
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
  let shown_at = atmos_white() * min(exposure(), MOON_FACE_SHOWN * PI / MOON_ALBEDO);
  var light = radiance * shown_at;
  if (atmos_moon_light() > 0.0) {
    let earth = 0.5 * (1.0 + dot(sun, axis));
    light += vec3f(EARTHSHINE_SHOWN * earth * face / MOON_MAP_MEAN);
  }
  return transmittance_to_top(r, mu) * light * edge;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let dir = normalize(sky_ray() * vec3f(v.ndc, 1.0));
  let r = camera_r();
  // Derivatives, so taken where every pixel takes them.
  let sun_a = length(cross(dir, atmos_sun_dir())) / (atmos_sun_angular_radius() * DISC_SCALE);
  let sun_w = max(fwidth(sun_a), 1e-4);
  let across = (dir - dot(dir, atmos_moon_dir()) * atmos_moon_dir()) / (MOON_ANGULAR_RADIUS * DISC_SCALE);
  let moon_a = length(across);
  let moon_w = max(fwidth(moon_a), 1e-4);
  var color = beyond(air_along(dir)) + sun_disc(dir, r, sun_a, sun_w) + moon_disc(dir, r, across, moon_a, moon_w) + glare(dir);
  // On the encoded value, where a step is one step at every brightness.
  return vec4f(finish(color, v.clip.xy) + (hash(v.clip.xy) - 0.5) / 255.0, 1.0);
}
