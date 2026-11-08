// pxl-render/frame/shader/ao_apply.wgsl

// Multiplies the occlusion into the world's color, drawn as a quad for each of
// the screen's tiles (screen_tile.wgsl, gpu/ao.rs `apply`). A pixel takes the
// four half-resolution texels whose centers surround its own, each weighed by
// how near it lies to the plane of the pixel's half texel and by how alike its
// normal is. The pixel returns black at the alpha by which they darken it, so
// the blend multiplies what the static opaque draws laid. The draw runs in the
// world's pass, which holds the full-resolution depth as its target, so the
// depth of the pixel's half texel, which the downsample kept from its 2×2,
// stands for the pixel's own. A pixel whose half texel is the sky takes its
// taps that are not sky. The sky is drawn later over every empty pixel, so no
// empty pixel stays darkened.

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
  // `base` is the first of the four half texels whose centers surround this
  // pixel's center. The pixel's own half texel is among them.
  let base = (fp - 1) / i32(AO_DOWNSCALE);
  var total = 0.0;
  var acc = 0.0;
  var open_cnt = 0.0;
  var open_acc = 0.0;
  // Where the pixel lies, which sets how much of the effects it shows: at its
  // half texel, or, when that is the sky, at its first tap that is not.
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
  let share = effect_share(length(at));
  return vec4f(0.0, 0.0, 0.0, (1.0 - pow(occ, AO_INTENSITY)) * share);
}
