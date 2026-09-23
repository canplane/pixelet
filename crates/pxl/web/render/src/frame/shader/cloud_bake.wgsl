// The cloud tile's own pass (cloud_tile.wgsl): a texel's place on the
// lattice through the octaves, once for a device. The lattice is the tile's,
// so what the fbm wraps at is the tile's own width and the texture tiles
// seamlessly; the octaves run whole, since a texel is the finest the tile
// holds and its mip chain is what a pixel reads instead of a footprint.
//
// What a texel keeps is not the shape but what the shape comes to: how much
// of the light through it the cloud there stops. The whole curve from one to
// the other is here, so a pixel does none of it, and so the mip chain
// averages the quantity a pixel wider than a cloud wants averaged — a pixel
// over half a cloud and half clear sky is half opaque, where the mean of the
// shape under it would have been the field's own mean and left clear sky.

// How much of the sky the layer covers, and how thick a full cloud is to
// light, as optical depth: past a few, its underside is in its own shadow
// and grey, and its edges, thin, are bright.
const CLOUD_COVER: f32 = 0.42;
const CLOUD_DEPTH: f32 = 6.0;
// The share of the cover that is a cloud's full thickness rather than its
// edge. A curve laid over the shape itself covered half the sky and reached
// full thickness nowhere: the field is octaves of value noise summed, so it
// is heaped about its mean, and a threshold near that mean cuts it where it
// is densest — a thin veil everywhere and a cloud nowhere.
const CLOUD_FULL_SHARE: f32 = 0.25;
// Being that sum, the field is near normal: its mean is the octaves'
// amplitudes halved, 0.46875 exactly, and its deviation, measured over the
// tile, is 0.0963. Through the normal's own distribution the shape becomes
// its quantile, and a threshold on a quantile is the share of the sky it
// leaves, which is what CLOUD_COVER then means. The logistic stands in for
// the normal's integral, which WGSL has no builtin for, within a hundredth
// of it over the range a cloud's edge lies in.
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
