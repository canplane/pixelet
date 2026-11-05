// pxl-render/zoom.wgsl

// The zoom asked at a distance on a face met at a slant, the camera grid's rule (pxl-engine
// grid.rs): every walk's (walk_node.wgsl), and the gizmo's that shows it (screen_shade.wgsl
// `cell_share`).

/// The zoom asked at `r2`, a distance squared, on a face `slant` octaves foreshortened
/// (`node_slant`; `Detail::on`).
fn zoom_at(r2: f32, slant: u32, range: vec2<u32>) -> u32 {
  return zoom_on(r2 * INV_SPLIT2, slant, range);
}

/// The zoom asked at `x2`, a distance squared in split distances squared, on a face met
/// head-on (`grid::cell_zoom_in_splits`).
fn zoom_in_splits(x2: f32, range: vec2<u32>) -> u32 {
  return zoom_on(x2, 0u, range);
}

/// The zoom asked at `x2`, a distance squared in split distances squared, on a face `slant`
/// octaves foreshortened (`grid::cell_zoom_on`), held to `range`.
fn zoom_on(x2: f32, slant: u32, range: vec2<u32>) -> u32 {
  let field = (bitcast<u32>(x2) >> 23u) & 255u;
  let e = (i32(select(field + slant, 0u, field == 0u)) - 127i) >> 1u;
  let z = u32(clamp(i32(UNIT_ZOOM) - e, 0i, i32(FINEST_ZOOM)));
  return min(max(z, range.x), range.y);
}

/// The whole octaves a face met at the cosine `c` is foreshortened by (`grid::slant`).
fn slant(c: f32) -> u32 {
  let bits = bitcast<u32>(c) & 0x7fffffffu;
  let up = i32(bits >> 23u) - 127i + select(0i, 1i, (bits & 0x7fffffu) != 0u);
  return u32(max(-up, 0i));
}
