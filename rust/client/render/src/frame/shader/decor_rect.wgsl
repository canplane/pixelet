// pxl-render/decor_rect.wgsl

// Whether a box of the frame's reaches what a candidate reads (pxl-content
// `generation::reaches`). The boxes are read from the list through `list_word`, which each pass
// that includes this part binds in its own way.

/// `reaches`: whether box `b` of the frame's, in leaf cells at `zoom`, reaches what a candidate
/// in `at` reads, in leaf cells of the clip level at `cell_zoom`.
fn reaches(b: vec4<i32>, zoom: u32, cell_zoom: u32, at: vec4<i32>) -> bool {
  if (zoom != EVERY && zoom < cell_zoom) {
    return false;
  }
  let s = LEAF_CELL_ZOOM - select(zoom, cell_zoom, zoom == EVERY);
  let r = READ_REACH << s;
  let low = ((at.xy >> vec2<u32>(s)) << vec2<u32>(s)) - r;
  let high = ((((at.zw - 1i) >> vec2<u32>(s)) + 1i) << vec2<u32>(s)) + r;
  // `meets`, with X going round the world.
  let west = wrap_idx(LEAF_CELL_ZOOM, b.x - low.x);
  let width = b.z - b.x;
  let lap = 1i << LEAF_CELL_ZOOM;
  let across = width >= lap || (west < high.x - low.x && west + width > 0i) || west + width > lap;
  return across && b.y < high.y && b.w > low.y;
}

fn is_changed(cell_zoom: u32, at: vec4<i32>) -> bool {
  // The list holds `RECT_CAP` boxes at most (pxl-content `generation::LIST_AT`), whatever
  // count its word holds.
  let rects = min(list_word(LIST_RECTS), RECT_CAP);
  for (var k = 0u; k < rects; k += 1u) {
    let b = RECTS_AT + RECT_WORDS * k;
    let rect = vec4<i32>(i32(list_word(b)), i32(list_word(b + 1u)), i32(list_word(b + 2u)), i32(list_word(b + 3u)));
    if (reaches(rect, list_word(b + 4u), cell_zoom, at)) {
      return true;
    }
  }
  return false;
}
