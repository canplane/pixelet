// pxl-render/decor_rect.wgsl

// The boxes of the frame's that reach what a candidate reads (pxl-content `generation::reaches`), off the list as its pass binds it (`list_word`).

/// `reaches`: whether a box `b` of the frame's, of leaf cells at `zoom`, reaches what a
/// candidate in `at` reads, leaf cells of the clip level at `cell_zoom`.
fn reaches(b: vec4<i32>, zoom: u32, cell_zoom: u32, at: vec4<i32>) -> bool {
  if (zoom != EVERY && zoom < cell_zoom) {
    return false;
  }
  let s = LEAF_CELL_ZOOM - select(zoom, cell_zoom, zoom == EVERY);
  let r = READ_REACH << s;
  let low = ((at.xy >> vec2<u32>(s)) << vec2<u32>(s)) - r;
  let high = ((((at.zw - 1i) >> vec2<u32>(s)) + 1i) << vec2<u32>(s)) + r;
  // `meets`, X round the world.
  let west = wrap_idx(LEAF_CELL_ZOOM, b.x - low.x);
  let width = b.z - b.x;
  let lap = 1i << LEAF_CELL_ZOOM;
  let across = width >= lap || (west < high.x - low.x && west + width > 0i) || west + width > lap;
  return across && b.y < high.y && b.w > low.y;
}

/// A box of the frame's reaches the leaf cells `at`.
fn is_changed(cell_zoom: u32, at: vec4<i32>) -> bool {
  // The list holds `RECT_CAP` boxes at most (pxl-content `generation::LIST_AT`).
  let rects = min(list_word(LIST_RECTS), RECT_CAP);
  for (var k = 0u; k < rects; k += 1u) {
    let b = RECTS_AT + RECT_WORDS * k;
    let edit = vec4<i32>(i32(list_word(b)), i32(list_word(b + 1u)), i32(list_word(b + 2u)), i32(list_word(b + 3u)));
    if (reaches(edit, list_word(b + 4u), cell_zoom, at)) {
      return true;
    }
  }
  return false;
}
