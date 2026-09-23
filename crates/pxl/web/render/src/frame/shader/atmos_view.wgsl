// The air as the frame shows it, for the sky and for every lit pass: the
// sky along a direction, the ground under the air where no terrain is
// drawn, and what the air between the eye and a point does to its colour.
// Over the sky-view table (atmos_sky.wgsl), shown at the exposure the eye
// has adapted to, which gpu/atmosphere.rs draws into a texel of its own.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;
@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;

// What the eye has adapted to: the factor a table's radiance is shown at.
fn exposure() -> f32 {
  return textureLoad(exposure_texel, vec2i(0, 0), 0).r;
}

// A table's radiance to the frame's: balanced, and exposed as the eye has
// adapted to the light the sky gives now.
fn shown() -> vec3f {
  return atmos.white * exposure();
}

// The sun's sky straight up, shown: what the exposure pass left beside the
// exposure (atmos_exposure.wgsl), one texel for every fragment that wants
// it rather than a table read each.
fn zenith_sky() -> vec3f {
  let t = textureLoad(exposure_texel, vec2i(0, 0), 0);
  return t.gba * atmos.white * t.r;
}

// The moonlit sky's table to the frame's: at the brightness a moonlit sky
// reads at against the day's, which the eye's night adaptation sets and not
// the moon's light (look/atmosphere.rs's MOONLIT_SKY).
fn moon_shown() -> vec3f {
  return atmos.white * atmos.exposure * atmos.moon_light;
}

// The sky the moon lights along `dir`.
fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos.moon_dir);
}

// The transmittance over `d` km from radius `r` along a ray at `mu`, as
// Bruneton takes it: a ratio of two lookups to the top, reversed for a ray
// that meets the ground, whose far end has no path to the top.
fn transmittance_along(r: f32, mu: f32, d: f32, ground: bool) -> vec3f {
  let rd = clamp(sqrt(d * d + 2.0 * r * mu * d + r * r), atmos.bottom_radius, atmos.top_radius);
  let mud = clamp((r * mu + d) / rd, -1.0, 1.0);
  if (ground) {
    return min(transmittance_to_top(rd, -mud) / max(transmittance_to_top(r, -mu), vec3f(1e-6)), vec3f(1.0));
  }
  return min(transmittance_to_top(r, mu) / max(transmittance_to_top(rd, mud), vec3f(1e-6)), vec3f(1.0));
}

// The air along a ray from the eye, in the root frame: the light it
// scatters toward the eye over its whole length, exposed and balanced —
// the sun's sky and, while it lights one, the moon's — and what it leaves
// of what lies at the ray's far end, the ground where the ray meets the
// planet and the top of the air where it does not. Read once for a ray,
// and shared by what lies along it (`aerial`) and past it (`beyond`).
struct Air {
  light: vec3f,
  end: vec3f,
  ground: bool,
};

fn air_along(dir: vec3f) -> Air {
  let r = camera_r();
  var air: Air;
  air.light = sky_view(dir, atmos.sun_dir) * shown();
  if (atmos.moon_light > 0.0) {
    air.light += moon_view(dir) * moon_shown();
  }
  air.ground = meets_ground(r, dir.z);
  if (air.ground) {
    air.end = transmittance_along(r, dir.z, to_ground(r, dir.z), true);
  } else {
    air.end = transmittance_to_top(r, dir.z);
  }
  return air;
}

// What the eye sees along a ray to its end: the air's light, and where the
// ray meets the planet, the ground's through the air. It is what lies past
// the terrain's drawn edge, so a view from high up reads as ground in haze
// there and not as sky under the horizon; and the ground is a flat face of
// the land's colour lit as shade.wgsl lights the terrain, by the frame's
// lights, so the edge is no seam.
fn beyond(air: Air) -> vec3f {
  var light = air.light;
  if (air.ground) {
    let lit = frame.sun_color * max(frame.sun_dir.z, 0.0) + frame.other_color * max(frame.other_dir.z, 0.0) + frame.ambient + frame.hemi_sky;
    light += air.end * atmos.land * (lit / PI + frame.floor);
  }
  return light;
}

// A straight segment's optical depth through a layer falling off over
// `scale` km, from altitude h0 to h1 over `d` km: the exponential's integral
// along a line, closed form. Flat rather than round, which a segment the
// terrain draws is short enough for, and exact at every length a view near
// the ground has, where a ratio of two table reads is noise.
fn layer_depth(h0: f32, h1: f32, d: f32, scale: f32) -> f32 {
  let dh = h1 - h0;
  if (abs(dh) < 1e-3) {
    return d * exp(-(h0 + h1) * 0.5 / scale);
  }
  return d * scale * (exp(-h0 / scale) - exp(-h1 / scale)) / dh;
}

// Aerial perspective: what the air between the eye and `world_pos` does to
// a colour there — `through`, the segment's transmittance, dims it, and
// `scattered`, the light the air scatters along it, fills it in. `air` is
// the ray's through that point (`air_along`), whose light is scattered
// along it to its end, so a point part way along has the share of it that
// the segment's extinction is of the whole ray's.
struct Veil {
  through: vec3f,
  scattered: vec3f,
};

fn veil(world_pos: vec3f, air: Air) -> Veil {
  var v: Veil;
  let du = length(world_pos - frame.camera_pos);
  if (du < 1e-4) {
    v.through = vec3f(1.0);
    v.scattered = vec3f(0.0);
    return v;
  }
  let d = du * atmos.km_per_u;
  // Both ends' altitudes over the planet, measured from its centre, under
  // the map centre's ground by the radius: the point is on the globe
  // (frame.wgsl's on_globe), and the camera is where it stands.
  let h0 = clamp(atmos.camera_altitude, 0.0, atmos.top_radius - atmos.bottom_radius);
  let planet = vec3f(frame.planet.xy, frame.planet.w - frame.planet.z);
  let h1 = max(h0 + (length(world_pos - planet) - length(frame.camera_pos - planet)) * atmos.km_per_u, 0.0);
  let depth = atmos.rayleigh_scattering * layer_depth(h0, h1, d, atmos.rayleigh_scale_height)
    + vec3f(atmos.mie_extinction * layer_depth(h0, h1, d, atmos.mie_scale_height));
  v.through = exp(-depth);
  v.scattered = min(air.light * (vec3f(1.0) - v.through) / max(vec3f(1.0) - air.end, vec3f(1e-4)), air.light);
  return v;
}

fn aerial(color: vec3f, world_pos: vec3f, air: Air) -> vec3f {
  let v = veil(world_pos, air);
  return color * v.through + v.scattered;
}

// The air as a lit surface's vertex takes it, for its fragments to
// interpolate (shade.wgsl): the veil in front of the vertex, and what the
// sky pass draws past it along the same ray, which the band hiding the
// world's drawn edge fades into. Every term changes slowly across a face —
// a direction's sky, a distance's exponential — so a vertex a few pixels or
// a few hundred from the next is near enough, and the four or five table
// reads it costs are a vertex's rather than every fragment's, overdraw and
// all: what engines do for fog on a phone.
struct Seen {
  through: vec3f,
  scattered: vec3f,
  past: vec3f,
};

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let air = air_along(normalize(world_pos - frame.camera_pos));
  let v = veil(world_pos, air);
  var s: Seen;
  s.through = v.through;
  s.scattered = v.scattered;
  s.past = shoulder(beyond(air));
  return s;
}

// The Sun's glare along `dir`: the light a lens scatters round a bright
// source, which blows the Sun out past its disc as a photograph of it does.
// The shape is Spencer, Shirley, Zimmerman and Greenberg's point-spread
// function ("Physically-Based Glare Effects for Digital Images", 1995), its
// halo alone — the core is the disc — in its own units, the angle in degrees
// and the value per steradian. Its strength is a camera's rather than an
// eye's: the halo carries GLARE_HALO_SHARE of the light, which is an eye
// looking at the Sun, blinding for tens of degrees; a camera lens scatters
// its veiling glare index, ISO 9358's measure, one to ten percent — the
// middle of that is GLARE_VEIL. So the Sun's illuminance at the eye times it
// is the glare's radiance, white out to a degree or two, three at sunset.
//
// Drawn over the sky only: the sky pass draws it, and a mountain or a tree
// in front of the Sun is drawn over it whole, while the sky round it glows.
// An eye's or a lens's veil lies over the ground too; a Sun behind a tree
// would need how much of it the frame shows, from the depth, and the sky's
// glow round the tree is most of what that looks like.
const GLARE_HALO_SHARE: f32 = 0.6205;
const GLARE_VEIL: f32 = 0.03;

fn glare(dir: vec3f) -> vec3f {
  let s = atmos.sun_dir;
  let theta = degrees(atan2(length(cross(dir, s)), dot(dir, s))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return atmos.sun_seen * shown() * psf * (GLARE_VEIL / GLARE_HALO_SHARE);
}

// A soft shoulder over the sky's brightest, the sun's glow and the lit
// clouds, which would otherwise clip flat at the encode.
fn shoulder(c: vec3f) -> vec3f {
  let k = 0.75;
  return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}
