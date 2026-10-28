// Bakes the cloud tile once per device. A texel stores opacity, not shape, so
// the mips average what a wide pixel needs: half a cloud is half opaque.

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let p = v.clip.xy / f32(CLOUD_TILE_TEXELS) * f32(CLOUD_TILE_UNITS);
  let shape = lattice_fbm(vec2u(0u), p, CLOUD_TILE_UNITS, 0u, 0u, 0.0) * (1.0 - CLOUD_DETAIL_SHARE)
    + lattice_fbm(vec2u(0u), p, CLOUD_TILE_UNITS, CLOUD_DETAIL_FIRST, CLOUD_DETAIL_SALT, 0.0) * CLOUD_DETAIL_SHARE;
  let quantile = 1.0 / (1.0 + exp(-LOGISTIC_SCALE * (shape - CLOUD_SHAPE_MEAN) / CLOUD_SHAPE_SIGMA));
  let depth = smoothstep(1.0 - CLOUD_COVER, 1.0 - CLOUD_FULL_SHARE * CLOUD_COVER, quantile) * CLOUD_DEPTH;
  return vec4f(1.0 - exp(-depth), 0.0, 0.0, 1.0);
}
