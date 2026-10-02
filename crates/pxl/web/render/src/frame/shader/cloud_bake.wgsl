// Bakes the cloud tile once per device. A texel stores opacity, not shape, so
// the mips average what a wide pixel needs: half a cloud is half opaque.

// Optical depth of a full cloud: past a few its underside is self-shadowed.
const CLOUD_COVER: f32 = 0.42;
const CLOUD_DEPTH: f32 = 6.0;
// A threshold near the summed noise's mean cut it where it is densest: a thin
// veil everywhere and a full cloud nowhere.
const CLOUD_FULL_SHARE: f32 = 0.25;
// The octave sum is near normal: mean 0.46875 exactly, deviation 0.0963
// measured over the tile. A threshold on its quantile is the covered share;
// the logistic approximates the normal CDF to within 0.01.
const CLOUD_SHAPE_MEAN: f32 = 0.46875;
const CLOUD_SHAPE_SIGMA: f32 = 0.0963;
const LOGISTIC_SCALE: f32 = 1.702;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let p = v.clip.xy / f32(CLOUD_TILE_TEXELS) * f32(CLOUD_TILE_UNITS);
  let shape = lattice_fbm(vec2u(0u), p, CLOUD_TILE_UNITS, 0u, 0u, 0.0) * 0.75
    + lattice_fbm(vec2u(0u), p, CLOUD_TILE_UNITS, 2u, 16u, 0.0) * 0.25;
  let quantile = 1.0 / (1.0 + exp(-LOGISTIC_SCALE * (shape - CLOUD_SHAPE_MEAN) / CLOUD_SHAPE_SIGMA));
  let depth = smoothstep(1.0 - CLOUD_COVER, 1.0 - CLOUD_FULL_SHARE * CLOUD_COVER, quantile) * CLOUD_DEPTH;
  return vec4f(1.0 - exp(-depth), 0.0, 0.0, 1.0);
}
