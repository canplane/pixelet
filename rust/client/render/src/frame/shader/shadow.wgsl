// The shadow map (gpu/shadow.rs), cast from the moon where the sun is
// down (`shadow_light`), a layer a level: read at the finest level that
// holds the receiver (`shadow_level`), by one computation at every level,
// 3x3 PCF with a texel of the level's of normal offset, each tap compared at
// the depth the receiver's own plane has where it lies; 1 past the last
// level, fading out toward its reach, where the eye stops drawing, so the
// shadows end there with no edge. A pass drawing the map binds a blank one
// here and reads as lit.

// The first level, and the rest a layer each, a side of their own
// (gpu/shadow.rs `size`).
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(23) var shadow_levels: texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

// How much the depth of a plane with normal `n` changes a texel across and
// a texel down the map, as a share of the box's depth: pxl-spec-core
// `light::slope`, the one statement of it, which the casters' push takes too
// (`Light::of`, `param::light::SLOPE_BIAS`). Its steps are the light box's own: the
// box is orthographic, so each row of its matrix is the world's step along
// that axis of the box over its length squared, a texel's across and down
// and the box's whole depth along the light's rays.
fn light_slope(n: vec3f, texel: f32) -> vec2f {
  let m = frame_light_view_proj();
  let across = vec3f(m[0].x, m[1].x, m[2].x);
  let up = vec3f(m[0].y, m[1].y, m[2].y);
  let deep = vec3f(m[0].z, m[1].z, m[2].z);
  let d = dot(n, deep / dot(deep, deep));
  // A face along the rays, which no ray enters across.
  if (d == 0.0) {
    return vec2f(0.0);
  }
  return -vec2f(dot(n, across / dot(across, across)), -dot(n, up / dot(up, up))) * texel / d;
}

// Where the map is cast from: the sun, or the moon (renderer.rs `prepare`).
fn shadow_light() -> vec3f {
  return select(atmos_sun_dir(), atmos_moon_dir(), frame_shadow_moon() > 0.0);
}

// How far out from the eye `p`, from it, lies for the levels, in the first
// level's half-widths: across the light's rays, or along them times the
// share of a half-width a receiver reads across, whichever is more, as
// pxl-spec-core `Light::level_dist` measures it, which the marks read. Along
// the rays off the light box's rows: its depth's over its across's, a unit
// each, the box orthographic.
fn shadow_dist(p: vec3f) -> f32 {
  let m = frame_light_view_proj();
  let turned = (m * vec4f(p, 0.0)).xyz;
  let across = vec3f(m[0].x, m[1].x, m[2].x);
  let deep = vec3f(m[0].z, m[1].z, m[2].z);
  let along = abs(turned.z) * length(across) / length(deep);
  return max(length(turned.xy), along * frame_shadow_radius_ndc());
}

// The level a receiver `q` out from the eye reads (`shadow_dist`), in the
// first level's half-widths: the finest whose reach holds it, each twice
// the last's, the power of two the float's own exponent, as pxl-spec-core
// `light::level_of` splits them, which the marks read.
fn shadow_level(q: f32) -> u32 {
  let x = q / frame_shadow_radius_ndc();
  if (x <= 1.0) {
    return 0u;
  }
  let bits = bitcast<u32>(x);
  let e = i32((bits >> 23u) & 0xffu) - 127;
  return u32(e + select(0, 1, (bits & 0x7fffffu) != 0u));
}

// Where `p`, from the eye, lies in level `l`'s box, its NDC and depth: the
// first level's turn, scaled across by `2^-l` and deep by the first's depth
// over the level's, as the levels' boxes are, and the level's own
// translation (gpu/shadow.rs `level_clip`).
fn shadow_level_clip(p: vec3f, l: u32) -> vec3f {
  let turned = (frame_light_view_proj() * vec4f(p, 0.0)).xyz;
  let at = frame_light_levels(l);
  return vec3f(turned.xy * exp2(-f32(l)), turned.z * at.w) + at.xyz;
}

// Level `l`'s compare at `uv` against `depth`: the first's own map, the
// rest's layer of theirs.
fn shadow_tap(l: u32, uv: vec2f, depth: f32) -> f32 {
  if (l == 0u) {
    return textureSampleCompareLevel(shadow_map, shadow_sampler, uv, depth);
  }
  return textureSampleCompareLevel(shadow_levels, shadow_sampler, uv, l - 1u, depth);
}

fn shadow_visibility(world_pos: vec3f, n: vec3f) -> f32 {
  // shade.wgsl weighs the light by the same facing, so the taps would be multiplied by zero.
  if (dot(n, shadow_light()) <= 0.0) {
    return 0.0;
  }
  if (frame_shadow_strength() <= 0.0) {
    return 1.0;
  }
  let q = shadow_dist(world_pos);
  let l = shadow_level(q);
  let levels = u32(frame_shadow_levels());
  if (l >= levels) {
    return 1.0;
  }
  let size = select(vec2f(textureDimensions(shadow_levels)), vec2f(textureDimensions(shadow_map)), l == 0u);
  // A texel of its level along the normal so a surface does not shadow
  // itself: its share of the box's width, `2^l` of the first level's half-
  // width, off the first's row across (gpu/shadow.rs `texel`).
  let m = frame_light_view_proj();
  let texel = 2.0 * exp2(f32(l)) / (size.x * length(vec3f(m[0].x, m[1].x, m[2].x)));
  let ndc = shadow_level_clip(world_pos + n * texel, l);
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0 || ndc.z < 0.0) {
    return 1.0;
  }
  let last = frame_shadow_radius_ndc() * exp2(f32(levels - 1u));
  let strength = frame_shadow_strength() * (1.0 - smoothstep(SHADOW_FADE_START * last, last, q));
  // A tap a texel and a half off lies where the receiver's plane is that
  // much nearer the light or further: compared at its own depth there
  // (receiver-plane depth bias, Isidoro 2006), not the receiver's, so the
  // casters' push need cover only the texels each tap filters. Off the
  // first level's box, its share of the level's: `2^l` times as wide, its
  // depth scaled (`shadow_level_clip`).
  let slope = light_slope(n, 2.0 / size.x) * exp2(f32(l)) * frame_light_levels(l).w * LIGHT_SPREAD;
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let o = vec2f(f32(x), f32(y));
      lit += shadow_tap(l, uv + o * LIGHT_SPREAD / size, ndc.z + dot(slope, o));
    }
  }
  return mix(1.0, lit / 9.0, strength);
}
