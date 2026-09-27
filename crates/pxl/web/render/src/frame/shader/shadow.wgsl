// The sun's shadow: how much of the sun reaches a world point, read off the
// shadow map with a 3x3 percentage-closer filter, a texel of normal offset;
// 1 where the map does not reach. The map and its sampler sit beside the
// frame uniform in group 0 in every pass that reads them; a pass that
// draws the map binds a blank one here, since it cannot read what it is
// writing, and every fragment reads as lit.

@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

fn sun_lit(world_pos: vec3f, n: vec3f) -> f32 {
  if (frame().shadow_strength <= 0.0) {
    return 1.0;
  }
  // Pushed along the normal by a texel, so a surface does not shadow
  // itself where the map's texel straddles it.
  let p = frame().light_view_proj * vec4f(world_pos + n * frame().shadow_texel, 1.0);
  let ndc = p.xyz / p.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0 || ndc.z < 0.0) {
    return 1.0;
  }
  let step = 1.0 / vec2f(textureDimensions(shadow_map));
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2f(f32(x), f32(y)) * step, ndc.z);
    }
  }
  return mix(1.0, lit / 9.0, frame().shadow_strength);
}
