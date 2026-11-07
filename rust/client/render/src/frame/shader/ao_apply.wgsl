// pxl-render/frame/shader/ao_apply.wgsl

// The occlusion over the world's color, as the tiles' quads
// (screen_tiles.wgsl): the four half-resolution texels whose centers surround
// the pixel's, weighed by the plane and the normal of the half texel the pixel
// lies in, then black blended over by how much they darken, so what the static
// opaque draws laid is multiplied by it (gpu/ao.rs `apply`). The pass draws
// the depth it would read, so the pixel's half texel, the depth the
// downsample kept of its 2×2, stands for the pixel's own. A pixel whose half
// texel is the sky takes its taps that are not; the sky, drawn after over
// every empty pixel, leaves none darkened.

@group(0) @binding(1) var half_depth: texture_2d<f32>;
@group(0) @binding(2) var blurred: texture_2d<f32>;

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let fp = vec2i(v.clip.xy);
  let half_max = vec2i(ao_half()) - 1;
  let own = clamp(fp / i32(AO_DOWNSCALE), vec2i(0), half_max);
  let own_depth = textureLoad(half_depth, own, 0).r;
  let normal = textureLoad(blurred, own, 0).gba * 2.0 - 1.0;
  let pos = view_pos(own_depth, (vec2f(own) + 0.5) / ao_half());
  // The four half texels whose centers surround this pixel's; the pixel's
  // own among them.
  let base = (fp - 1) / i32(AO_DOWNSCALE);
  var total = 0.0;
  var acc = 0.0;
  var open_cnt = 0.0;
  var open_acc = 0.0;
  // Where the pixel lies, for its reach: its half texel's, or where that is
  // the sky, the first of its taps that is not.
  var at = pos;
  for (var y = 0; y <= 1; y++) {
    for (var x = 0; x <= 1; x++) {
      let p = clamp(base + vec2i(x, y), vec2i(0), half_max);
      let tap_depth = textureLoad(half_depth, p, 0).r;
      if (tap_depth == 0.0) {
        continue;
      }
      let tap = textureLoad(blurred, p, 0);
      let tap_pos = view_pos(tap_depth, (vec2f(p) + 0.5) / ao_half());
      if (own_depth == 0.0 && open_cnt == 0.0) {
        at = tap_pos;
      }
      open_cnt += 1.0;
      open_acc += tap.r;
      let tap_normal = tap.gba * 2.0 - 1.0;
      let plane_dist = abs(dot(tap_pos - pos, normal));
      let weight = exp(-plane_dist / AO_DIST_FALLOFF) * max(dot(normal, tap_normal), 0.0);
      total += weight;
      acc += tap.r * weight;
    }
  }
  var occ = 1.0;
  if (own_depth != 0.0 && total > 0.0) {
    occ = acc / total;
  } else if (open_cnt > 0.0) {
    occ = open_acc / open_cnt;
  }
  // To the effects' reach, faded alike before it (effect.wgsl `effect_share`).
  let share = effect_share(length(at));
  return vec4f(0.0, 0.0, 0.0, (1.0 - pow(occ, AO_INTENSITY)) * share);
}
