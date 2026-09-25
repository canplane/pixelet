// The sky: one triangle over the screen at the far plane, behind everything
// and drawn after it, where nothing opaque stands (gpu/renderer.rs), each
// pixel the light along its own ray (atmos_view.wgsl's beyond, over
// the tables gpu/atmosphere.rs draws) — the air's, and the ground's past
// the terrain's drawn edge. Over it the sun's disc and the moon's, each
// seen through the air. The stars and the clouds are passes of their own over this
// one (gpu/stars.rs, gpu/clouds.rs). The output is graded, vignetted and
// encoded to sRGB as every pass's is — the frame uniform is bound for the
// vignette.

struct Sky {
  // A pixel's ray in the root frame, affine in its NDC (gpu/renderer.rs's
  // put_ray): the frame the air's sun and moon are in.
  ray: mat3x3f,
  // The celestial pole in the same frame, which the Moon's north is turned
  // toward, and whether its face has arrived (gpu/moon.rs).
  pole: vec3f,
  mapped: f32,
};

@group(0) @binding(1) var<uniform> sky: Sky;
@group(0) @binding(2) var moon_map: texture_2d_array<f32>;
@group(0) @binding(3) var moon_sampler: sampler;

// The sun's disc before the exposure: far past white once exposed, so it
// is the air in front of it that colors it, as it reddens a low sun.
const SUN_DISC: f32 = 4.0;
// Limb darkening: the disc's edge is the sun's cooler outer layer, seen
// through more of it.
const SUN_LIMB: f32 = 0.6;
// How much larger than they stand the Sun's and the Moon's discs are drawn.
// At their true size each is eight pixels on a phone, smaller than the Sun
// and the Moon anyone remembers seeing: the eye enlarges them — the moon
// illusion near the horizon, the glare round a bright one. Four, a whole
// multiple of the truth, taken by look between it and the dome shader's
// eight-odd, which had no derivation. Only their size is not theirs; their
// light, their faces and the Moon's phase are.
const DISC_SCALE: f32 = 4.0;
// The Moon's mean semi-diameter as seen from the Earth, and what its surface
// sends back on average of the sunlight falling square on it.
const MOON_ANGULAR_RADIUS: f32 = 0.004522;
const MOON_ALBEDO: f32 = 0.12;
// The face's map (gpu/moon.rs): texels across a radius, and its mean over the
// disc as the Earth sees it, which asset/script/download_moon.rs prints — the
// map is relative, and this is what scales it to MOON_ALBEDO.
const MOON_MAP_RADIUS_TEXELS: f32 = 256.0;
const MOON_MAP_MEAN: f32 = 0.2459;
// How bright a full Moon's face is shown at most, on average: the eye looking
// at the Moon adapts to it, and sees its seas and its highlands where the
// sky's exposure would drown the disc white. By day the sky's exposure is
// the lower, and the face shows at it, a pale disc about as bright as the
// blue around it.
const MOON_FACE_SHOWN: f32 = 0.6;
// Earthshine: the Earth's sunlit face lighting the Moon's dark side, the
// ashen light of a crescent. What it has is a ten-thousandth of the
// sunlit side's, which no eight-bit screen holds beside it; what it is shown
// at is how a crescent's dark side reads to the eye at night, and it goes as
// the Earth's phase from the Moon, full when the Moon is new.
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

// The Sun's disc along `dir` from radius `r`, `a` the ray's offset from its
// center in disc radii and `w` how far `a` moves across a pixel: limb-
// darkened, through the air in front of it, its edge a pixel soft. Nothing
// where the planet stands in the way.
fn sun_disc(dir: vec3f, r: f32, a: f32, w: f32) -> vec3f {
  let mu = dot(dir, atmos.up);
  if (meets_ground(r, mu) || dot(dir, atmos.sun_dir) <= 0.0 || a >= 1.0 + w) {
    return vec3f(0.0);
  }
  let inside = min(a, 1.0);
  let limb = 1.0 - SUN_LIMB * (1.0 - sqrt(1.0 - inside * inside));
  let edge = 1.0 - smoothstep(1.0 - w, 1.0 + w, a);
  return transmittance_to_top(r, mu) * shown() * SUN_DISC * limb * edge;
}

// The Moon's face along `dir` from radius `r`: `across` is the ray's offset
// from the disc's center in disc radii and `a` its length, and `w` how far
// `a` moves across a pixel, taken where every pixel of the pass takes it.
// Each point of the face is the map's, the near side turned with its north
// toward the celestial pole; it sends back the sunlight by the
// Lommel–Seeliger law — which is why a full Moon is as bright at its limb as
// at its center, where a Lambertian ball would dim — so its phase is where
// the Sun lights it, pixel by pixel. It is seen through the air as the Sun
// is. Its limb is a pixel's soft edge. Nothing where the planet stands in
// the way.
fn moon_disc(dir: vec3f, r: f32, across: vec3f, a: f32, w: f32) -> vec3f {
  let axis = atmos.moon_dir;
  let mu = dot(dir, atmos.up);
  if (meets_ground(r, mu) || dot(dir, axis) <= 0.0 || a >= 1.0 + w) {
    return vec3f(0.0);
  }
  let edge = 1.0 - smoothstep(1.0 - w, 1.0 + w, a);
  let on = across / max(a, 1.0);
  let normal = on - axis * sqrt(max(1.0 - dot(on, on), 0.0));

  // Selenographic: north toward the pole, east to the right as the Earth
  // sees it, and the near side's center toward the Earth.
  let north = normalize(sky.pole - dot(sky.pole, axis) * axis);
  let east = cross(axis, north);
  let lon = atan2(dot(normal, east), -dot(normal, axis));
  let lat = asin(clamp(dot(normal, north), -1.0, 1.0));
  var face = MOON_MAP_MEAN;
  if (sky.mapped > 0.5) {
    let uv = vec2f(lon / PI + 0.5, 0.5 - lat / PI);
    let lod = log2(max(MOON_MAP_RADIUS_TEXELS * w, 1e-6));
    face = textureSampleLevel(moon_map, moon_sampler, uv, 0, lod).r;
  }
  let albedo = MOON_ALBEDO * face / MOON_MAP_MEAN;

  let sun = atmos.sun_dir;
  let lit = max(dot(normal, sun), 0.0);
  let seen = max(-dot(normal, dir), 1e-4);
  let radiance = albedo / PI * 2.0 * lit / (lit + seen);
  let shown_at = atmos.white * min(exposure(), MOON_FACE_SHOWN * PI / MOON_ALBEDO);
  var light = radiance * shown_at;
  if (atmos.moon_light > 0.0) {
    let earth = 0.5 * (1.0 + dot(sun, axis));
    light += vec3f(EARTHSHINE_SHOWN * earth * face / MOON_MAP_MEAN);
  }
  return transmittance_to_top(r, mu) * light * edge;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let dir = normalize(sky.ray * vec3f(v.ndc, 1.0));
  let r = camera_r();
  // The ray's place on each disc, in its radii, and how far that moves
  // across a pixel: a derivative, so taken here, where every pixel takes it.
  let sun_a = length(cross(dir, atmos.sun_dir)) / (atmos.sun_angular_radius * DISC_SCALE);
  let sun_w = max(fwidth(sun_a), 1e-4);
  let across = (dir - dot(dir, atmos.moon_dir) * atmos.moon_dir) / (MOON_ANGULAR_RADIUS * DISC_SCALE);
  let moon_a = length(across);
  let moon_w = max(fwidth(moon_a), 1e-4);
  var color = beyond(air_along(dir)) + sun_disc(dir, r, sun_a, sun_w) + moon_disc(dir, r, across, moon_a, moon_w) + glare(dir);
  color = shoulder(color);
  color += (hash(v.clip.xy) - 0.5) * (1.0 / 255.0);
  return vec4f(finish(color, v.clip.xy), 1.0);
}
