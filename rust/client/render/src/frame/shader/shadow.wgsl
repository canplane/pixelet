// Reads the shadow map (pass/shadow.rs), which is cast from the moon where the sun is down
// (`shadow_light`). A receiver takes 3x3 PCF with a texel of normal offset, and each tap is
// compared at the depth the receiver's own plane has where the tap lies. A receiver the map
// does not reach is lit. The shadow fades toward the effects' reach as every effect does
// (effect.wgsl `effect_share`), so it ends where the others end, with no edge. A pass that
// draws the map binds a blank one here and reads as lit.

@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

// How much the depth of a plane with normal `n` changes a texel across and a texel down the
// map, as a share of the box's depth. It is the twin of pxl-engine `light::slope`, which the
// casters' push also takes (`Light::of`, `param::light::SLOPE_BIAS`), so receivers and casters
// agree. The steps are the light box's own, which the frame holds (`LIGHT_STEPS`): a texel
// across, a texel down, and the box's whole depth along the light's rays.
fn light_slope(n: vec3f, texel: f32) -> vec2f {
  let steps = frame_light_steps();
  let d = dot(n, steps[2]);
  // A face that lies along the light's rays is entered by no ray, so it takes no slope.
  if (d == 0.0) {
    return vec2f(0.0);
  }
  return -vec2f(dot(n, steps[0]), -dot(n, steps[1])) * texel / d;
}

// The direction the map is cast from: the sun's, or the moon's where the frame says so
// (renderer.rs `prepare`).
fn shadow_light() -> vec3f {
  return select(atmos_sun_dir(), atmos_moon_dir(), frame_shadow_moon() > 0.0);
}

fn shadow_visibility(world_pos: vec3f, n: vec3f) -> f32 {
  // A face turned away from the light takes none of it in shade.wgsl, which weighs the light by
  // the same facing, so its taps are skipped.
  if (dot(n, shadow_light()) <= 0.0) {
    return 0.0;
  }
  // With no map drawn, or past the effects' reach, the receiver is lit without being projected
  // or tapped.
  let strength = frame_shadow_strength() * effect_share(length(world_pos));
  if (strength <= 0.0) {
    return 1.0;
  }
  // The receiver is moved a texel along its normal, so a surface does not shadow itself.
  let p = frame_light_view_proj() * vec4f(world_pos + n * frame_shadow_texel(), 1.0);
  let ndc = p.xyz / p.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0 || ndc.z < 0.0) {
    return 1.0;
  }
  let size = vec2f(textureDimensions(shadow_map));
  // A tap `LIGHT_SPREAD` texels off lies where the receiver's plane is nearer the light or
  // farther by the plane's slope. Each tap is compared at the plane's depth there
  // (receiver-plane depth bias, Isidoro 2006), not at the receiver's, so the casters' push need
  // cover only the texels each tap filters.
  let slope = light_slope(n, 2.0 / size.x) * LIGHT_SPREAD;
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let o = vec2f(f32(x), f32(y));
      lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + o * LIGHT_SPREAD / size, ndc.z + dot(slope, o));
    }
  }
  return mix(1.0, lit / 9.0, strength);
}
