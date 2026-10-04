// One cloud layer over the planet, drawn after the opaque world. It writes no
// depth: from below it fills only the sky, from above it covers everything, so
// a peak above the layer is wrong either way.

struct Clouds {
  ray: mat3x3f,
  // The same for every point shown, so the CPU computes it once.
  sun_at: vec3f,
  _p1: f32,
  moon_at: vec3f,
  _p2: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(1) var<uniform> clouds_words: array<vec4f, 5>;

fn clouds_ray() -> mat3x3f { return mat3x3f(clouds_words[0].xyz, clouds_words[1].xyz, clouds_words[2].xyz); }
fn clouds_sun_at() -> vec3f { return clouds_words[3].xyz; }
fn clouds_moon_at() -> vec3f { return clouds_words[4].xyz; }

const CLOUD_ALBEDO: f32 = 0.9;
// Past this the layer is a sliver behind haze; from above the reach grows so
// it still meets the horizon.
const CLOUD_REACH_KM: f32 = 120.0;

struct CloudOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> CloudOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: CloudOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}

fn hg(c: f32, g: f32) -> f32 {
  return INV_4PI * (1.0 - g * g) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

fn lit_by(toward: vec3f, view: vec3f, arriving: vec3f, shown_at: vec3f) -> vec3f {
  let c = dot(view, toward);
  let phase = 0.7 * hg(c, 0.6) + 0.3 * hg(c, -0.25) + INV_4PI;
  return arriving * phase * 4.0 * shown_at;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let dir = normalize(clouds_ray() * vec3f(v.ndc, 1.0));

  // Near crossing from above, the way out from below.
  let r0 = camera_r();
  let shell = atmos_bottom_radius() + atmos_cloud_altitude_km();
  let mu = dot(dir, atmos_up());
  let disc = r0 * r0 * (mu * mu - 1.0) + shell * shell;
  var t = -1.0;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    t = select(-r0 * mu + root, -r0 * mu - root, r0 > shell);
  }
  let reach = CLOUD_REACH_KM + 3.0 * max(atmos_camera_altitude() - atmos_cloud_altitude_km(), 0.0);

  // A derivative, so taken before any pixel is discarded.
  let du = t / atmos_km_per_u();
  let p = cloud_lattice(frame_camera_pos() + dir * du);
  let footprint = max(length(fwidth(p)), 1e-6);
  if (t <= 0.0 || t > reach) {
    discard;
  }
  let opacity = cloud_opacity(p, footprint);
  if (opacity <= 0.002) {
    discard;
  }
  let alpha = opacity * (1.0 - smoothstep(0.6, 1.0, t / reach)) * cloud_shown();

  let below = r0 < shell;
  // Opacity 1 - exp(-depth), half weighted, gives 1 - sqrt(1 - opacity).
  let self_shade = select(1.0, mix(1.0, 0.3, 1.0 - sqrt(1.0 - opacity)), below);
  var light = lit_by(atmos_sun_dir(), dir, clouds_sun_at(), shown());
  var sky = zenith_sky();
  if (atmos_moon_light() > 0.0) {
    light += lit_by(atmos_moon_dir(), dir, clouds_moon_at(), moon_shown());
    sky += moon_view(atmos_up()) * moon_shown();
  }
  light *= self_shade;
  // The terrain's fill too, so a cloud is never darker than the ground under
  // it: without it night clouds were black blots.
  let sky_light = sky_fill();
  let fill = (frame_ambient() + sky_light) / PI + frame_floor();
  var color = CLOUD_ALBEDO * (light + sky * 0.5 + fill);

  let world = frame_camera_pos() + dir * du;
  color = aerial(color, world);
  return vec4f(finish(shoulder(color), v.clip.xy), alpha);
}
