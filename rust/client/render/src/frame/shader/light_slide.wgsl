// pxl-render/light_slide.wgsl

// Drops what an earlier cast of the epoch left in a tile of the box where the box slid since,
// a `TILE`-aligned corner at a time (pass/shadow.rs `aim`), where the eye's zooms moved
// (`light::HDR_LAST_RANGE`), or where the world under the tile changed. It runs one thread
// for each tile, after the host moved the reads with the box, so each tile's word is the word
// of the tile that held its world texels under the corner before (`light::HDR_LAST_CORNER`;
// ray/cast.rs `prepare`). A tile's read is set to none where the box slid onto the tile,
// where the slide or the zooms moved it into another band (`Light::keeps`), or where its rays
// may meet a cell of its band over a column of the world that changed (`Light::reaches`,
// `light::COLUMNS_AT`). The propagation then damages such a tile anew if a receiver reads it
// (light_propagate.wgsl). The other tiles hold what their cast left. Each thread writes only
// its own tile's word, and the first thread also writes the cast's number as the last cast the
// slide ran in (`light::SLIDE_RAN`). The slide runs where the box slid, the zooms moved or the
// world changed, and a receiver stands (`light::RECEIVER_TILES_SLIDE`). It runs after the
// receivers stood, and before the propagation.

/// Whether the rays of box tile `tile`, asking at its band, may meet a cell over the world's
/// column `c`, `[x0, y0, x1, y1]` in units from the eye (`Light::reaches`). Only a band from
/// the first to the last of `bands` reads the change. Seen from above, the tile's rays sweep a
/// hexagon as its texels move along the light. The column, grown by a cell of the band and a
/// texel, misses that hexagon where one of five axes parts them: X, Y, and the axis across each
/// of the hexagon's three edges.
fn reaches(tile: vec2<u32>, c: vec4<f32>, bands: vec2<u32>) -> bool {
  let band = tile_band(vec2<i32>(tile));
  if (band < bands.x || band > bands.y) {
    return false;
  }
  let grow = ldexp(1.0, i32(UNIT_ZOOM) - i32(band)) + eye_f32(EYE_LIGHT + LIGHT_WIDTH);
  let lo = c.xy - vec2<f32>(grow);
  let hi = c.zw + vec2<f32>(grow);
  let t = f32(LIGHT_TILE);
  let i = f32(tile.x) * t - 0.5;
  let j = f32(tile.y) * t - 0.5;
  let across = light_v3(LIGHT_ACROSS).xy;
  let down = light_v3(LIGHT_DOWN).xy;
  let e0 = across * t;
  let e1 = down * t;
  let e2 = light_v3(LIGHT_DIR).xy * eye_f32(EYE_LIGHT + LIGHT_T_MAX);
  let mid = light_v3(0u).xy + i * across + j * down + 0.5 * (e0 + e1 + e2);
  let at = 0.5 * (lo + hi) - mid;
  let half = 0.5 * (hi - lo);
  return !(parts(vec2<f32>(1.0, 0.0), at, half, e0, e1, e2)
    || parts(vec2<f32>(0.0, 1.0), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e0.y, e0.x), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e1.y, e1.x), at, half, e0, e1, e2)
    || parts(vec2<f32>(-e2.y, e2.x), at, half, e0, e1, e2));
}

/// Whether axis `n` parts the column from the hexagon whose edges are `e0`, `e1` and `e2`. The
/// column is `half` wide each way, and its middle lies `at` from the hexagon's middle.
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
  if (all(id.xy == vec2<u32>(0u))) {
    list_put(hdr_word(LIGHT_HDR_SLIDE) + LIGHT_SLIDE_RAN, cast_number());
  }
  let was = vec2<i32>(i32(hdr_word(LIGHT_HDR_LAST_CORNER)), i32(hdr_word(LIGHT_HDR_LAST_CORNER + 1u)));
  var reached = !keeps_tile(id.xy, was);
  // The head holds `LIGHT_COLUMNS` columns at most (pxl-engine `light::COLUMNS`).
  let columns = min(u32(hdr[LIGHT_COLUMNS_AT].x), LIGHT_COLUMNS);
  for (var k = 0u; k < columns && !reached; k += 1u) {
    let at = LIGHT_COLUMNS_AT + 1u + 2u * k;
    reached = reaches(id.xy, bitcast<vec4<f32>>(hdr[at]), bitcast<vec2<u32>>(hdr[at + 1u].xy));
  }
  if (reached) {
    list_put(reads_at() + id.y * tiles + id.x, 0u);
  }
}
