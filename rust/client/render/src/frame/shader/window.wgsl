// pxl-render/window.wgsl

// Finds the least of the beam rays' starts within a cell's window, for the eye (cell.wgsl,
// pxl-engine `beam::Grid::cell_window`) and for the light (light_cell.wgsl, `light::cell`). It
// takes the least of each row, then the first row whose least is less than every earlier
// row's, so of equal starts it takes the first in row-major order.

/// The least hand over the square `side` a side from `first`, of the hands laid from word
/// `hands`, `across` to a row. A hand is how far a beam ray hands its start on (pxl-engine
/// `beam::Grid::hand_dist`). It returns the least hand's bits, then the index of the first hand
/// that holds it.
fn window_least(hands: u32, across: u32, first: vec2<u32>, side: u32) -> vec2<u32> {
  var least_at = first.y * across + first.x;
  var least = bitcast<f32>(beams[hands + least_at]);
  for (var j = 0u; j < side; j += 1u) {
    let row = (first.y + j) * across + first.x;
    var row_at = row;
    var row_least = bitcast<f32>(beams[hands + row]);
    for (var i = 1u; i < side; i += 1u) {
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
  return vec2<u32>(bitcast<u32>(least), least_at);
}
