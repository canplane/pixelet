// pxl-render/record_box.wgsl

// A drawable entry's box (pxl-engine raster.rs), as the record's draw tests it against the
// observer's view (record_draw.wgsl) and the raster draws it off the entry's slot
// (raster.wgsl): over the node cache as the module binds it, read through `node_cache_word`.

/// The entry at word `e`'s box from the origin `o`, the cell its asset keeps inset as it
/// fills (pxl-engine cast.rs `asset_box`).
fn entry_box(e: u32, k: Node, o: vec3<i32>) -> Planes {
  let p = kept_planes(k, o);
  let eighths = (node_cache_word(e + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
  var lo = p.lo;
  var hi = p.hi;
  if (eighths != 0u) {
    let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
    lo += inset;
    hi -= inset;
  }
  return Planes(lo, hi);
}
