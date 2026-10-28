// AO at full resolution: the four half-resolution taps about the pixel,
// weighed by the plane and the normal of the half-resolution texel the
// pixel lies in (its normal as the downsample formed it, carried through
// the denoise), bilinear where no tap agrees.

@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(0) @binding(2) var half_depth: texture_2d<f32>;
@group(0) @binding(3) var blurred: texture_2d<f32>;
@group(0) @binding(4) var blurred_sampler: sampler;

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let fp = vec2i(i.clip.xy);
  let depth = textureLoad(scene_depth, fp, 0);
  if (depth == 0.0) {
    return vec4f(1.0);
  }
  let uv = i.clip.xy / frame_viewport();
  let pos = view_pos(depth, uv);
  let half_max = vec2i(ao_half()) - 1;
  let own = clamp(fp / i32(AO_DOWNSCALE), vec2i(0), half_max);
  let normal = textureLoad(blurred, own, 0).gba * 2.0 - 1.0;
  // The four half texels whose centers surround this pixel's.
  let base = (fp - 1) / i32(AO_DOWNSCALE);
  var total = 0.0;
  var acc = vec4f(0.0);
  for (var y = 0; y <= 1; y++) {
    for (var x = 0; x <= 1; x++) {
      let p = clamp(base + vec2i(x, y), vec2i(0), half_max);
      let p_uv = (vec2f(p) + 0.5) / ao_half();
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
