// 3x3 PCF with a texel of normal offset; 1 where the map does not reach. A
// pass drawing the map binds a blank one here and reads as lit.

// Taps a texel and a half apart, so the filtered edge spans about a unit: one
// texel apart, the edge cut each cell's shadow out as a hard block.
const SHADOW_SPREAD: f32 = 1.5;

@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

fn sun_lit(world_pos: vec3f, n: vec3f) -> f32 {
  // shade.wgsl weighs the sun by the same facing, so the taps would be multiplied by zero.
  if (dot(n, frame_sun_dir()) <= 0.0) {
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
  let strength = frame_shadow_strength();
  let step = 1.0 / vec2f(textureDimensions(shadow_map));
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2f(f32(x), f32(y)) * step * SHADOW_SPREAD, ndc.z);
    }
  }
  return mix(1.0, lit / 9.0, strength);
}
