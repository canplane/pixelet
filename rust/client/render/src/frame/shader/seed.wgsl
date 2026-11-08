// pxl-render/seed.wgsl

// What draws on the world's nodes turn on (pxl-engine seed.rs, xpute-kit `derive_seed_u32`,
// pxl-content share.rs).

/// `wrap_idx`.
fn wrap_idx(zoom: u32, x: i32) -> i32 {
  if (zoom == 0u) {
    return 0i;
  }
  return (x << (32u - zoom)) >> (32u - zoom);
}

/// `xpute_kit::derive_seed_u32`.
fn seed_mix(base: u32, tag: u32) -> u32 {
  var z = (base ^ tag) + SEED_SALT;
  z = (z ^ (z >> 16u)) * MIX_FIRST;
  z = (z ^ (z >> 13u)) * MIX_LAST;
  return z ^ (z >> 16u);
}

/// `seed::node_seed` of a node of the plane.
fn quad_seed(tag: u32, zoom: u32, c: vec2<i32>) -> u32 {
  let h = seed_mix(seed_mix(MASTER_SEED, tag), zoom);
  return seed_mix(seed_mix(h, u32(wrap_idx(zoom, c.x))), u32(c.y));
}

/// `seed::node_seed`.
fn node_seed(tag: u32, zoom: u32, c: vec3<i32>) -> u32 {
  return seed_mix(quad_seed(tag, zoom, c.xy), u32(c.z));
}

fn rotl(v: u32, s: u32) -> u32 {
  return (v << (s & 31u)) | (v >> ((32u - s) & 31u));
}

/// `share::halve`.
fn halve(n: u32, cap_0: u32, cap_1: u32, w: u32) -> u32 {
  let scaled = n * (w >> 16u);
  let part = (scaled >> 16u) + u32((scaled & 0xffffu) > (w & 0xffffu));
  let least = select(0u, n - cap_1, n > cap_1);
  if (part < least) {
    return least;
  }
  return min(part, cap_0);
}
