// Half-resolution AO over the normal's hemisphere. Out: occlusion (1 for none)
// and the packed normal for the denoise.

@group(0) @binding(1) var half_depth: texture_2d<f32>;
@group(0) @binding(2) var blue_noise: texture_2d<f32>;

// N8AO's generateHemisphereSamples(4).
const HEMISPHERE = array<vec3f, AO_SAMPLES>(
  vec3f(0.353553, 0.000000, 0.935414),
  vec3f(-0.451544, 0.413652, 0.790569),
  vec3f(0.069116, -0.787542, 0.612372),
  vec3f(0.569142, 0.742346, 0.353553),
);

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(half_depth, clamp(p, vec2i(0), vec2i(ao_half()) - 1), 0).r;
}

fn rot_z(theta: f32) -> mat3x3f {
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
  let uv = i.clip.xy / ao_half();
  let pos = view_pos(depth, uv);
  let normal = t.gba;
  let noise = textureLoad(blue_noise, hp & vec2i(AO_NOISE_MASK), 0);
  var helper = vec3f(0.0, 1.0, 0.0);
  if (dot(helper, normal) > 0.99) {
    helper = vec3f(1.0, 0.0, 0.0);
  }
  let tangent = normalize(cross(helper, normal));
  let bitangent = cross(normal, tangent);
  let tbn = mat3x3f(tangent, bitangent, normal) * rot_z(noise.r * TAU);
  let falloff = AO_RADIUS * AO_DIST_FALLOFF * AO_FALLOFF_SHARE;
  var occluded = 0.0;
  var total = 0.0;
  var off_move = noise.g;
  let off_step = 1.0 / f32(AO_SAMPLES);
  var hemisphere = HEMISPHERE;
  for (var k = 0; k < AO_SAMPLES; k++) {
    let dir = tbn * hemisphere[k];
    let move_amount = fract(off_move);
    off_move += off_step;
    let sample_pos = pos + AO_RADIUS * move_amount * dir;
    let o = ao_proj() * vec4f(sample_pos, 1.0);
    let ndc = o.xyz / o.w;
    let s = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (all(s * (1.0 - s) > vec2f(0.0)) && ndc.z > 0.0 && ndc.z < 1.0) {
      let texel = floor(s * ao_half());
      let sample_depth = textureLoad(half_depth, vec2i(texel), 0).r;
      if (sample_depth > 0.0) {
        let dist_sample = view_dist(sample_depth);
        let dist_world = view_dist(ndc.z);
        let range_check = smoothstep(0.0, 1.0, falloff / abs(dist_sample - dist_world));
        // The pixel center is half a texel past its corner, so a difference under one is itself.
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
