// AO at full resolution: a 3x3 of half-resolution taps weighed by the full-
// resolution plane and normal, bilinear where no tap agrees.

@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(0) @binding(2) var half_depth: texture_2d<f32>;
@group(0) @binding(3) var blurred: texture_2d<f32>;
@group(0) @binding(4) var blurred_sampler: sampler;

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(frame_viewport()) - 1), 0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let fp = vec2i(i.clip.xy);
  let depth = depth_at(fp);
  if (depth == 0.0) {
    return vec4f(1.0);
  }
  let uv = i.clip.xy / frame_viewport();
  let pos = view_pos(depth, uv);
  let normal = view_normal(fp, frame_viewport(), uv);
  let half_max = vec2i(ao_half()) - 1;
  var total = 0.0;
  var acc = vec4f(0.0);
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let p = clamp(fp / 2 + vec2i(x, y), vec2i(0), half_max);
      let p_uv = vec2f(p) / ao_half();
      let tap_depth = textureLoad(half_depth, p, 0).r;
      let tap = textureLoad(blurred, p, 0);
      let tap_normal = tap.gba * 2.0 - 1.0;
      let tap_pos = view_pos(tap_depth, p_uv);
      let plane_dist = abs(dot(tap_pos - pos, normal));
      let weight = exp(-plane_dist / AO_DISTANCE_FALLOFF) * max(dot(normal, tap_normal), 0.0);
      total += weight;
      acc += tap * weight;
    }
  }
  var texel: vec4f;
  if (total > 0.0) {
    texel = acc / total;
  } else {
    texel = textureSampleLevel(blurred, blurred_sampler, uv, 0.0);
  }
  return vec4f(pow(texel.r, AO_INTENSITY), 0.0, 0.0, 1.0);
}
