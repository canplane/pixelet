// pxl-render/frame/shader/window.wgsl

// Finds the least of the beam rays' start distances within a cell's window, for the eye
// (cell.wgsl, pxl-engine `beam::Grid::cell_window`) and for the light (light_cell.wgsl,
// `light::cell`). It takes the least of each row, then the first row whose least is less than
// every earlier row's, so of equal start distances it takes the first in row-major order.

/// The least start distance over the square `side` a side from `first`, of those laid from
/// word `start_dists`, `across` to a row. A start distance is where the rays about a beam ray
/// start (pxl-engine `beam::Grid::start_dist`). It returns the least one's bits, then the index
/// of the first that holds it.
fn window_least(start_dists: u32, across: u32, first: vec2<u32>, side: u32) -> vec2<u32> {
  var least_at = first.y * across + first.x;
  var least = bitcast<f32>(beams[start_dists + least_at]);
  for (var j = 0u; j < side; j += 1u) {
    let row = (first.y + j) * across + first.x;
    var row_at = row;
    var row_least = bitcast<f32>(beams[start_dists + row]);
    for (var i = 1u; i < side; i += 1u) {
      let t = bitcast<f32>(beams[start_dists + row + i]);
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
