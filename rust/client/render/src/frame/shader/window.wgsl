// pxl-render/window.wgsl

// The least of the beam rays' starts within a cell's window, the eye's (cell.wgsl,
// `beam::Grid::cell_window`) and the light's (light_cell.wgsl, `light::cell`): the least of
// each row, then the first row whose least is less than every row's before it, so the
// window's first least in row-major order is taken.

/// The least of the hands from word `hands`, `across` a row, over the square `side` a side
/// from `first`, and the index of the first that holds it: its bits, then its index; then 1
/// where a hand of the window is a beam ray's not yet ended (`beam::HAND_PARKED`), its cone
/// not walked, so the cell's rays start from the root (cell.wgsl, light_cell.wgsl).
fn window_least(hands: u32, across: u32, first: vec2<u32>, side: u32) -> vec3<u32> {
  var least_at = first.y * across + first.x;
  var least = bitcast<f32>(beams[hands + least_at]);
  var parked = false;
  for (var j = 0u; j < side; j += 1u) {
    let row = (first.y + j) * across + first.x;
    var row_at = row;
    var row_least = bitcast<f32>(beams[hands + row]);
    parked = parked || beams[hands + row] == HAND_PARKED;
    for (var i = 1u; i < side; i += 1u) {
      parked = parked || beams[hands + row + i] == HAND_PARKED;
      let t = bitcast<f32>(beams[hands + row + i]);
      if (t < row_least) {
        row_least = t;
        row_at = row + i;
      }
    }
    if (row_least < least) {
      least = row_least;
      least_at = row_at;
    }
  }
  return vec3<u32>(bitcast<u32>(least), least_at, select(0u, 1u, parked));
}
