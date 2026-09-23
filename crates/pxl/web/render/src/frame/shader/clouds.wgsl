// The clouds: a layer of them over the whole planet, gpu/clouds.rs's
// ALTITUDE_KM up and curved with it, drawn after the opaque world as one
// triangle over the screen. Each pixel's ray meets the layer's sphere — from
// below looking up, or from above looking down — and where it does, the
// cloud there is value noise on the map's own lattice, anchored to the
// ground and drifting with the wind, lit by the sun and the moon through the
// air as they reach that height and by the sky above it, and seen through
// the air between.
//
// It writes no depth of its own, so the depth test runs before the shader:
// the triangle lies at the far plane, and from below the layer it is drawn
// only where the opaque world left the sky, so the ground and a mountain
// stand in front of the clouds and a pixel they cover never runs this; from
// above, it is drawn over everything, and the clouds hide the ground under
// them (gpu/clouds.rs picks the pipeline). What that gets wrong is a peak
// standing above the layer: from below, its top is not hidden by the cloud
// in front of it, and from above, it is.

struct Clouds {
  // A pixel's ray in the root frame, affine in its NDC (gpu/renderer.rs's
  // put_ray).
  ray: mat3x3f,
  // What reaches the layer's height of the sun's light and of the moon's,
  // by channel, unexposed: the same for every point of it the frame shows,
  // so the CPU takes it once (look/atmosphere.rs's light_at).
  sun_at: vec3f,
  _p1: f32,
  moon_at: vec3f,
  _p2: f32,
};

@group(0) @binding(1) var<uniform> clouds: Clouds;

const CLOUD_ALBEDO: f32 = 0.9;
// Past this, from under the layer or close over it, the layer is too thin a
// sliver of the view to be worth the noise, and fades into the haze it is
// already mostly behind. Over the layer the reach grows with the height
// over it, so the layer still reaches the horizon while it fades out.
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


// Henyey-Greenstein, for the light a cloud throws forward — the silver
// lining round a cloud in front of the sun.
fn hg(c: f32, g: f32) -> f32 {
  return INV_4PI * (1.0 - g * g) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

// What a cloud scatters toward the eye, per unit of its albedo, from a
// light toward `toward` of which `arriving` reaches the layer, shown at
// `shown_at`: thrown forward and back by the droplets and spread through the
// cloud by scattering many times over.
fn lit_by(toward: vec3f, view: vec3f, arriving: vec3f, shown_at: vec3f) -> vec3f {
  let c = dot(view, toward);
  let phase = 0.7 * hg(c, 0.6) + 0.3 * hg(c, -0.25) + INV_4PI;
  return arriving * phase * 4.0 * shown_at;
}

@fragment
fn fs(v: CloudOut) -> @location(0) vec4f {
  let dir = normalize(clouds.ray * vec3f(v.ndc, 1.0));

  // The layer's sphere from the camera's: the near crossing from above, the
  // way out from below.
  let r0 = camera_r();
  let shell = atmos.bottom_radius + atmos.cloud_altitude_km;
  let mu = dot(dir, atmos.up);
  let disc = r0 * r0 * (mu * mu - 1.0) + shell * shell;
  var t = -1.0;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    t = select(-r0 * mu + root, -r0 * mu - root, r0 > shell);
  }
  let reach = CLOUD_REACH_KM + 3.0 * max(atmos.camera_altitude - atmos.cloud_altitude_km, 0.0);

  // Where the ray meets the layer on the cloud field's lattice
  // (cloud_field.wgsl), and how many lattice units a pixel spans: a
  // derivative, so taken before any pixel is discarded.
  let du = t / atmos.km_per_u;
  let p = cloud_lattice(frame.camera_pos + dir * du);
  let footprint = max(length(fwidth(p)), 1e-6);
  if (t <= 0.0 || t > reach) {
    discard;
  }
  let depth = cloud_depth(p, footprint);
  if (depth <= 0.001 * CLOUD_DEPTH) {
    discard;
  }
  // Thinned as the camera climbs over the layer (cloud_field.wgsl's
  // cloud_shown); past where it is gone the pass is not drawn at all.
  let alpha = (1.0 - exp(-depth)) * (1.0 - smoothstep(0.6, 1.0, t / reach)) * cloud_shown();

  // Lit from above: a cloud seen from below shows its underside, which the
  // cloud itself shades as it thickens; seen from above, its lit top.
  let below = r0 < shell;
  let self_shade = select(1.0, mix(1.0, 0.3, 1.0 - exp(-depth * 0.5)), below);
  var light = lit_by(atmos.sun_dir, dir, clouds.sun_at, shown());
  var sky = zenith_sky();
  if (atmos.moon_light > 0.0) {
    light += lit_by(atmos.moon_dir, dir, clouds.moon_at, moon_shown());
    sky += moon_view(atmos.up) * moon_shown();
  }
  light *= self_shade;
  // The sky above lights the layer as well: its zenith, half of it thrown
  // back out the side the eye sees. And the fill the terrain is lit by —
  // the themes' ambient and sky light and the floor, as shade.wgsl takes
  // them for a face turned up — so a cloud is never darker than the ground
  // it lies over: at night the air's light is all but gone, the ground is
  // lit by the fill, and a cloud without it read as a black blot on it. By
  // day the sun has already taken the layer past white.
  let fill = (frame.ambient + sky_fill()) / PI + frame.floor;
  var color = CLOUD_ALBEDO * (light + sky * 0.5 + fill);

  // Where the layer lies in the root frame, for the air in front of it.
  let world = frame.camera_pos + dir * du;
  color = aerial(color, world, air_along(dir));
  return vec4f(finish(shoulder(color), v.clip.xy), alpha);
}
