// The sun's shadow map (gpu/shadow.rs), cast from the Moon where the sun is
// down (`shadow_light`): 3x3 PCF with a texel of normal
// offset, each tap compared at the depth the receiver's own plane has
// where it lies; 1 where the map does not reach, and fading out toward the
// casters' reach, so the shadows end where they do with no edge. A pass
// drawing the map binds a blank one here and reads as lit.

@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

// How much the depth of a plane with normal `n` changes a texel across and
// a texel down the map, as a share of the box's depth: pxl-spec-core
// `sun::slope`, the one statement of it, which the casters' push takes too
// (`Sun::of`, `param::sun::SLOPE_BIAS`). Its steps are the light box's own: the
// box is orthographic, so each row of its matrix is the world's step along
// that axis of the box over its length squared, a texel's across and down
// and the box's whole depth along the sun's rays.
fn sun_slope(n: vec3f, texel: f32) -> vec2f {
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

// Where the map is cast from: the sun, or the Moon (renderer.rs `lay_out`).
fn shadow_light() -> vec3f {
  return select(atmos_sun_dir(), atmos_moon_dir(), frame_shadow_moon() > 0.0);
}

fn sun_lit(world_pos: vec3f, n: vec3f) -> f32 {
  // shade.wgsl weighs the light by the same facing, so the taps would be multiplied by zero.
  if (dot(n, shadow_light()) <= 0.0) {
    return 0.0;
  }
  if (frame_shadow_strength() <= 0.0) {
    return 1.0;
  }
  // A texel along the normal so a surface does not shadow itself.
  let p = frame_light_view_proj() * vec4f(world_pos + n * frame_shadow_texel(), 1.0);
  let ndc = p.xyz / p.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0 || ndc.z < 0.0) {
    return 1.0;
  }
  let rim = frame_shadow_rim();
  let strength = frame_shadow_strength() * (1.0 - smoothstep(SHADOW_FADE * rim, rim, length(ndc.xy)));
  let size = vec2f(textureDimensions(shadow_map));
  // A tap a texel and a half off lies where the receiver's plane is that
  // much nearer the sun or further: compared at its own depth there
  // (receiver-plane depth bias, Isidoro 2006), not the receiver's, so the
  // casters' push need cover only the texels each tap filters.
  let slope = sun_slope(n, 2.0 / size.x) * SUN_SPREAD;
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let o = vec2f(f32(x), f32(y));
      lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + o * SUN_SPREAD / size, ndc.z + dot(slope, o));
    }
  }
  return mix(1.0, lit / 9.0, strength);
}
