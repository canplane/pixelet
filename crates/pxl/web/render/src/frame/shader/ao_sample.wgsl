// The occlusion at half resolution: the hemisphere over a pixel's normal
// sampled at AO_SAMPLES points within AO_RADIUS of it, each spun about the
// normal and pushed along its ray by the pixel's blue noise, and counted
// as occluding where the depth image is nearer than the point by more
// than nothing and by less than the falloff. Out: the occlusion, 1 for
// none, and the normal packed into the other three channels for the
// denoise to weigh its taps by.

@group(0) @binding(1) var half_depth: texture_2d<f32>;
@group(0) @binding(2) var blue_noise: texture_2d<f32>;

// A spiral over the disc lifted onto the hemisphere: N8AO's
// generateHemisphereSamples(8), the golden angle between turns.
const HEMISPHERE = array<vec3f, 8>(
  vec3f(0.250000, 0.000000, 0.968246),
  vec3f(-0.319290, 0.292496, 0.901388),
  vec3f(0.048872, -0.556877, 0.829156),
  vec3f(0.402445, 0.524917, 0.750000),
  vec3f(-0.738535, -0.130636, 0.661438),
  vec3f(0.699604, -0.445032, 0.559017),
  vec3f(-0.234003, 0.870484, 0.433013),
  vec3f(-0.446273, -0.859268, 0.250000),
);

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(half_depth, clamp(p, vec2i(0), vec2i(ao.half) - 1), 0).r;
}

fn rotation_z(theta: f32) -> mat3x3f {
  let c = cos(theta);
  let s = sin(theta);
  return mat3x3f(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let hp = vec2i(i.clip.xy);
  let t = textureLoad(half_depth, hp, 0);
  let depth = t.r;
  if (depth == 0.0) {
    return vec4f(1.0);
  }
  let uv = i.clip.xy / ao.half;
  let pos = view_pos(depth, uv);
  let normal = t.gba;
  let noise = textureLoad(blue_noise, hp & vec2i(AO_NOISE_MASK), 0);
  var helper = vec3f(0.0, 1.0, 0.0);
  if (dot(helper, normal) > 0.99) {
    helper = vec3f(1.0, 0.0, 0.0);
  }
  let tangent = normalize(cross(helper, normal));
  let bitangent = cross(normal, tangent);
  let tbn = mat3x3f(tangent, bitangent, normal) * rotation_z(noise.r * TAU);
  // N8AO scales the falloff by a fifth where the radius is in world
  // units, as it is here.
  let falloff = AO_RADIUS * AO_DISTANCE_FALLOFF * 0.2;
  var occluded = 0.0;
  var total = 0.0;
  var offset_move = noise.g;
  let offset_step = 1.0 / f32(AO_SAMPLES);
  var hemisphere = HEMISPHERE;
  for (var k = 0; k < AO_SAMPLES; k++) {
    let dir = tbn * hemisphere[k];
    let move_amount = fract(offset_move);
    offset_move += offset_step;
    let sample_pos = pos + AO_RADIUS * move_amount * dir;
    let o = ao.proj * vec4f(sample_pos, 1.0);
    let ndc = o.xyz / o.w;
    let s = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (all(s * (1.0 - s) > vec2f(0.0)) && ndc.z > 0.0 && ndc.z < 1.0) {
      let texel = floor(s * ao.half);
      let sample_depth = textureLoad(half_depth, vec2i(texel), 0).r;
      if (sample_depth > 0.0) {
        let dist_sample = view_dist(sample_depth);
        let dist_world = view_dist(ndc.z);
        let range_check = smoothstep(0.0, 1.0, falloff / abs(dist_sample - dist_world));
        // Not the pixel's own texel: the pixel center is half a texel
        // past its corner, so a difference under one is itself.
        let diff = i.clip.xy - texel;
        occluded += range_check * f32(dist_sample != dist_world) * f32(sample_depth != depth)
          * step(dist_sample, dist_world) * step(1.0, dot(diff, diff));
      }
      total += 1.0;
    }
  }
  let occ = clamp(1.0 - occluded / select(total, 1.0, total == 0.0), 0.0, 1.0);
  return vec4f(occ, 0.5 + 0.5 * normal);
}
