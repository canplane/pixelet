// The air as the frame shows it, for the sky and for every lit pass: the
// sky along a direction, the ground under the air where no terrain is
// drawn, and what the air between the eye and a point does to its color.
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
  return atmos_white() * exposure();
}

// The sun's sky straight up, shown: what the exposure pass left beside the
// exposure (atmos_exposure.wgsl), one texel for every fragment that wants
// it rather than a table read each.
fn zenith_sky() -> vec3f {
  let t = textureLoad(exposure_texel, vec2i(0, 0), 0);
  return t.gba * atmos_white() * t.r;
}

// The sky's light on a face turned up, on the terrain's own scale: the
// scale EXPOSURE puts the air on, before the eye's adaptation, since the
// terrain's direct light (look/atmosphere.rs's sun_light) does not adapt
// either — a fill that did would wash the ground out as the sun sank, when
// the adaptation rises and the sun's light falls. The sun's sky, from the
// exposure pass's second texel, and while the moon lights one, the moon's,
// as its zenith over the hemisphere. It is the sky's half of the fill the
// terrain is lit by (shade.wgsl), in place of a color a theme chose: a
// sunset's orange, a moonlit night's blue, and what little a moonless night
// leaves.
fn sky_fill() -> vec3f {
  var fill = textureLoad(exposure_texel, vec2i(1, 0), 0).rgb * atmos_white() * atmos_exposure();
  if (atmos_moon_light() > 0.0) {
    fill += moon_view(atmos_up()) * moon_shown() * PI;
  }
  return fill;
}

// What the ground under a point throws back up at it: the land's color
// against everything that falls on a face turned up — the sun and the second
// light at their own heights, the themes' ambient, and the sky's own light as
// the vertex took it — over pi, and the floor, which the ground takes as
// every other face does. It is `beyond`'s ground under the same lights, so
// the fill a face takes from below is the ground it would see if it looked,
// and it goes orange into a sunset and blue into a moonlit night where a
// theme's one color could not.
fn ground_bounce(fill: vec3f) -> vec3f {
  let up = vec3f(0.0, 0.0, 1.0);
  let lit = frame_sun_color() * max(frame_sun_dir().z, 0.0) + frame_other_color() * max(frame_other_dir().z, 0.0) + frame_ambient() + fill;
  return atmos_land() * (lit / PI + frame_floor());
}

// The moonlit sky's table to the frame's: at the brightness a moonlit sky
// reads at against the day's, which the eye's night adaptation sets and not
// the moon's light (look/atmosphere.rs's MOONLIT_SKY).
fn moon_shown() -> vec3f {
  return atmos_white() * atmos_exposure() * atmos_moon_light();
}

// The sky the moon lights along `dir`.
fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos_moon_dir());
}

// The transmittance over `d` km from radius `r` along a ray at `mu`, as
// Bruneton takes it: a ratio of two lookups to the top, reversed for a ray
// that meets the ground, whose far end has no path to the top.
fn transmittance_along(r: f32, mu: f32, d: f32, ground: bool) -> vec3f {
  let rd = clamp(sqrt(d * d + 2.0 * r * mu * d + r * r), atmos_bottom_radius(), atmos_top_radius());
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
// planet and the top of the air where it does not: what lies past the
// terrain (`beyond`), which the sky pass draws and the band hiding the
// world's drawn edge fades into.
struct Air {
  light: vec3f,
  end: vec3f,
  ground: bool,
  // How far the ray runs to the planet, in km, where it meets it.
  ground_km: f32,
  // Where the ray meets the planet, which way is up there: the ground past
  // the terrain is lit on its own slope of the sphere.
  normal: vec3f,
};

fn air_along(dir: vec3f) -> Air {
  let r = camera_r();
  var air: Air;
  air.light = sky_view(dir, atmos_sun_dir()) * shown();
  if (atmos_moon_light() > 0.0) {
    air.light += moon_view(dir) * moon_shown();
  }
  let mu = dot(dir, atmos_up());
  air.ground = meets_land(r, mu);
  air.normal = atmos_up();
  if (air.ground) {
    let d = to_land(r, mu);
    air.ground_km = d;
    air.end = transmittance_along(r, mu, d, true);
    air.normal = normalize(atmos_up() * r + dir * d);
  } else {
    air.end = transmittance_to_top(r, mu);
  }
  return air;
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

// What the surface haze leaves of a color `d` km away, between altitudes
// `h0` and `h1`: the aerosol the tables do not carry, mixed through its own
// layer, so it thins with height as Earth's does and a view from above sees
// through it (look/atmosphere.rs's HAZE_VISUAL_RANGE_KM).
fn haze_through(h0: f32, h1: f32, d: f32) -> f32 {
  return exp(-atmos_haze_extinction() * layer_depth(h0, h1, d, atmos_haze_scale_height()));
}

// What the aerosol scatters of the light it takes: a continental haze's
// single-scattering albedo, about where OPAC's continental types sit at
// green (Hess, Koepke and Schult, 1998). Said aloud rather than derived: the
// haze is a layer the tables do not carry, so nothing else fixes it.
const HAZE_ALBEDO: f32 = 0.9;

// The light the air along `dir` takes and turns toward the eye, on the
// terrain's scale, by what scatters it: the two lights as they reach the
// terrain, each by the scatterer's phase, and the light from every other
// way — the sky's over the upper half, the ground's over the lower — as the
// terrain is lit by them. It is the light of the air where it is and not
// the ray's: the air in front of a mountain is lit where it stands, while
// the sky along the same ray is lit for hundreds of kilometers past the
// mountain, most of all along the horizon toward a low sun, and a veil taken
// from it laid that over the ground as a line at the horizon's height.
struct AirLight {
  rayleigh: vec3f,
  mie: vec3f,
};

fn air_light(dir: vec3f, fill: vec3f) -> AirLight {
  let c_sun = dot(dir, frame_sun_dir());
  let c_other = dot(dir, frame_other_dir());
  let around = 0.5 * ((fill + frame_ambient()) / PI + ground_bounce(fill));
  var l: AirLight;
  l.rayleigh = frame_sun_color() * rayleigh_phase(c_sun) + frame_other_color() * rayleigh_phase(c_other) + around;
  l.mie = frame_sun_color() * mie_phase(c_sun) + frame_other_color() * mie_phase(c_other) + around;
  return l;
}

// What the eye sees along a ray to its end: the air's light, and where the
// ray meets the planet, the ground's through the air. It is what lies past
// the terrain's drawn edge, so a view from high up reads as ground in haze
// there and not as sky under the horizon; and the ground is a flat face of
// the land's color lit as shade.wgsl lights the terrain, by the frame's
// lights, so the edge is no seam.
// `fill` is sky_fill's, which a caller that reads it for more than this
// takes once.
fn beyond(air: Air, fill: vec3f) -> vec3f {
  var light = air.light;
  if (air.ground) {
    let n = air.normal;
    let lit = frame_sun_color() * max(dot(frame_sun_dir(), n), 0.0) + frame_other_color() * max(dot(frame_other_dir(), n), 0.0) + frame_ambient() + fill;
    light += air.end * atmos_land() * (lit / PI + frame_floor());
    // The haze between, which the ray's transmittance does not carry: what
    // it hides reads as the air's own light along the same ray, which is
    // what distance looks like from the ground.
    light = mix(air.light, light, haze_through(max(atmos_camera_altitude(), 0.0), 0.0, air.ground_km));
  }
  return light;
}

// Aerial perspective: what the air between the eye and `world_pos` does to
// a color there — `through`, the segment's transmittance, dims it, and
// `scattered`, the light the air along it takes and turns toward the eye,
// fills it in (`air_light`), under the sky's fill `fill` (sky_fill).
struct Veil {
  through: vec3f,
  scattered: vec3f,
};

fn veil(world_pos: vec3f, fill: vec3f) -> Veil {
  var v: Veil;
  let du = length(world_pos - frame_camera_pos());
  if (du < 1e-4) {
    v.through = vec3f(1.0);
    v.scattered = vec3f(0.0);
    return v;
  }
  let d = du * atmos_km_per_u();
  // Both ends' altitudes over the planet, measured from its center, under
  // the map center's ground by the radius: the point is on the globe
  // (frame.wgsl's on_globe), and the camera is where it stands.
  let h0 = clamp(atmos_camera_altitude(), 0.0, atmos_top_radius() - atmos_bottom_radius());
  let planet = vec3f(frame_planet().xy, frame_planet().w - frame_planet().z);
  let h1 = max(h0 + (length(world_pos - planet) - length(frame_camera_pos() - planet)) * atmos_km_per_u(), 0.0);
  let rayleigh = atmos_rayleigh_scattering() * layer_depth(h0, h1, d, atmos_rayleigh_scale_height());
  let mie = atmos_mie_extinction() * layer_depth(h0, h1, d, atmos_mie_scale_height());
  let haze = atmos_haze_extinction() * layer_depth(h0, h1, d, atmos_haze_scale_height());
  // One segment, its scatterers each taking the share of what it
  // extinguishes that is theirs, and of that share scattering what their
  // albedo is: the single-scattering integral over a medium lit the same
  // along the segment, which a segment the terrain draws is short enough
  // for. Its light is the air's where the segment is (`air_light`).
  let depth = rayleigh + vec3f(mie + haze);
  v.through = exp(-depth);
  let lit = air_light((world_pos - frame_camera_pos()) / du, fill);
  let turned = rayleigh * lit.rayleigh + (mie * atmos_mie_scattering() / atmos_mie_extinction() + haze * HAZE_ALBEDO) * lit.mie;
  v.scattered = (vec3f(1.0) - v.through) * turned / max(depth, vec3f(1e-6));
  return v;
}

fn aerial(color: vec3f, world_pos: vec3f, fill: vec3f) -> vec3f {
  let v = veil(world_pos, fill);
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
  // The sky's fill (sky_fill), the same at every vertex of a frame and
  // carried beside the rest, since the lit passes read the air only here.
  fill: vec3f,
};

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let fill = sky_fill();
  let air = air_along(normalize(world_pos - frame_camera_pos()));
  let v = veil(world_pos, fill);
  var s: Seen;
  s.through = v.through;
  s.scattered = v.scattered;
  s.past = shoulder(beyond(air, fill));
  s.fill = fill;
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
  let s = atmos_sun_dir();
  let theta = degrees(atan2(length(cross(dir, s)), dot(dir, s))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return atmos_sun_seen() * shown() * psf * (GLARE_VEIL / GLARE_HALO_SHARE);
}

// A soft shoulder over the sky's brightest, the sun's glow and the lit
// clouds, which would otherwise clip flat at the encode.
fn shoulder(c: vec3f) -> vec3f {
  let k = 0.75;
  return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}
