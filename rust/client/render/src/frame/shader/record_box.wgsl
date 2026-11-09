// pxl-render/record_box.wgsl

// The box of a drawable entry (pxl-engine raster.rs). The record's draw tests this box against
// the observer's view (record_draw.wgsl), and the raster draws it from the entry's slot
// (raster.wgsl), so both see the same box. The module that includes this part binds the node
// cache and reads it through `node_cache_word`.

/// The box of the entry in slot `slot`, whose node is `k`, in units from the origin `o`
/// (pxl-engine cast.rs `asset_box`). Where the entry's asset fills only part of its cube, the
/// box is inset on each axis to the part it fills.
fn asset_box(slot: u32, k: Node, o: vec3<i32>) -> Aabb {
  let p = node_planes(k, o);
  let eighths = (node_cache_word(entry_at(slot) + W_ASSET) >> FILL_SHIFT) & (EIGHTHS - 1u);
  var lo = p.lo;
  var hi = p.hi;
  if (eighths != 0u) {
    let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
    lo += inset;
    hi -= inset;
  }
  return Aabb(lo, hi);
}
