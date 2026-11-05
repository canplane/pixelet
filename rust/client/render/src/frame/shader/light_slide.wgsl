// pxl-render/light_slide.wgsl

// The tiles of the box an earlier cast of the epoch cast, where the box slid since, a
// `TILE`-aligned corner at a time (gpu/shadow.rs `aim`), or the world under it changed: a
// thread a tile, after the host moved the reads with the box, each tile's word the word of
// the tile that held its world texels under the corner before (`light::HEAD_CORNER_WAS`;
// gpu/ray/cast.rs `prepare`). A tile the box slid onto, one the slide moved into another band
// (`Light::keeps`), and one whose rays may meet a cell of its band over a column of the world
// that changed (`Light::reaches`, `light::COLUMNS_AT`), is set to none, so the propagation
// takes it, read, as read anew (light_propagate.wgsl); the rest hold what their cast left.
// Each writes its own word alone. Run only where the box slid or the world changed.

/// Whether box tile `tile`'s rays, at its band, may meet a cell over the world's column `c`,
/// `[x0, y0, x1, y1]` units from the eye (`Light::reaches`): the column grown by a cell of the
/// band and a texel, against the hexagon the tile's texels sweep along the light seen from
/// above, on X, Y and across each of its three edges.
fn reaches(tile: vec2<u32>, c: vec4<f32>) -> bool {
  let band = tile_band(vec2<i32>(tile));
  let grow = ldexp(1.0, i32(UNIT_ZOOM) - i32(band)) + eye_f32(LIGHT + LIGHT_WIDTH);
  let lo = c.xy - vec2<f32>(grow);
  let hi = c.zw + vec2<f32>(grow);
  let t = f32(LIGHT_TILE);
  let i = f32(tile.x) * t - 0.5;
  let j = f32(tile.y) * t - 0.5;
  let across = light_v3(LIGHT_ACROSS).xy;
  let down = light_v3(LIGHT_DOWN).xy;
  let e0 = across * t;
  let e1 = down * t;
  let e2 = light_v3(LIGHT_DIR).xy * eye_f32(LIGHT + LIGHT_T_MAX);
  let mid = light_v3(0u).xy + i * across + j * down + 0.5 * (e0 + e1 + e2);
  let at = 0.5 * (lo + hi) - mid;
  let half = 0.5 * (hi - lo);
  return !(parts(vec2<f32>(1.0, 0.0), at, half, e0, e1, e2)
    || parts(vec2<f32>(0.0, 1.0), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e0.y, e0.x), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e1.y, e1.x), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e2.y, e2.x), at, half, e0, e1, e2));
}

/// Whether axis `n` parts the column `half` wide each way, `at` from the hexagon's middle,
/// from the hexagon of edges `e0`, `e1` and `e2`.
fn parts(n: vec2<f32>, at: vec2<f32>, half: vec2<f32>, e0: vec2<f32>, e1: vec2<f32>, e2: vec2<f32>) -> bool {
  let swept = abs(dot(e0, n)) + abs(dot(e1, n)) + abs(dot(e2, n));
  let column = abs(half.x * n.x) + abs(half.y * n.y);
  return abs(dot(at, n)) > 0.5 * swept + column;
}

@compute @workgroup_size(SQUARE_SIDE, SQUARE_SIDE, 1)
fn light_slide_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let tiles = light_tiles();
  if (id.x >= tiles || id.y >= tiles) {
    return;
  }
  let was = vec2<i32>(i32(head_word(LIGHT_HEAD_CORNER_WAS)), i32(head_word(LIGHT_HEAD_CORNER_WAS + 1u)));
  var reached = !keeps_tile(id.xy, was);
  let columns = u32(head[LIGHT_COLUMNS_AT].x);
  for (var k = 0u; k < columns && !reached; k += 1u) {
    reached = reaches(id.xy, bitcast<vec4<f32>>(head[LIGHT_COLUMNS_AT + 1u + k]));
  }
  if (reached) {
    list_put(reads_at() + id.y * tiles + id.x, 0u);
  }
}
