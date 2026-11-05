// pxl-render/cell.wgsl

// Where each of the eye's camera grid's cells' rays start (pxl-engine `beam::CELL_WORDS`),
// a thread a cell: the least of the beam rays' hand-on distances within its window of
// corners (`beam::Grid::cell_window`), and that corner, whose line its rays resume from.
// The shadow map's are light_cell.wgsl's.
//
// The window is f32 as the Rust's, a ceiling over a square root and two divisions: where
// what it takes the ceiling of falls within a few ulps of a whole number, a GPU's rounding
// may take the next window (tests/cell.rs `on_edge`).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// The corners a side about the screen's cell `c` whose least start its rays take, from
/// the screen's left and top, less two and halved (`Grid::cell_window`, `Grid::window`): at
/// its corner farthest from the screen's axis, as the slope grows outward.
fn cell_window(c: vec2<u32>) -> u32 {
  let s = eye_f32(GRID + GRID_S);
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let at = max(abs(-tan_xy + vec2<f32>(c) * s), abs(-tan_xy + vec2<f32>(c + 1u) * s));
  let radius = eye_f32(GRID + GRID_RADIUS);
  let radius_tan = eye_f32(GRID + GRID_RADIUS_TAN);
  let edge = sqrt(at.x * at.x + at.y * at.y);
  let beyond = (edge + radius_tan) / (1.0 - edge * radius_tan);
  let stretch = 1.0 + beyond * beyond;
  return u32(ceil(FRAC_1_SQRT_2 + radius * stretch / s));
}

@compute @workgroup_size(CELL_SIDE, CELL_SIDE, 1)
fn cell_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= eye_word(GRID + GRID_SPAN) || id.y >= eye_word(GRID + GRID_SPAN + 1u)) {
    return;
  }
  let across = eye_word(GRID + GRID_CORNERS);
  // How far each corner's rays start, a word a corner past the lines (`Grid::hand_at`).
  let hands = across * eye_word(GRID + GRID_CORNERS + 1u) * LINE_WORDS;
  // No wider than the last cell's, which the grid's corners are laid for (`Grid::of`'s
  // `past`): a GPU's ceiling may take the next window for it.
  let k = min(cell_window(id.xy), eye_word(GRID + GRID_PAST));
  let first = id.xy + vec2<u32>(eye_word(GRID + GRID_PAST)) - vec2<u32>(k);
  var corner = first.y * across + first.x;
  var least = bitcast<f32>(beams[hands + corner]);
  for (var j = 0u; j < 2u + 2u * k; j += 1u) {
    for (var i = 0u; i < 2u + 2u * k; i += 1u) {
      let at = (first.y + j) * across + first.x + i;
      let t = bitcast<f32>(beams[hands + at]);
      if (t < least) {
        least = t;
        corner = at;
      }
    }
  }
  let at = cell_start(id.xy);
  beams[at + CELL_T] = bitcast<u32>(least);
  beams[at + CELL_CORNER] = corner;
  // The eye's cells and corners ask its one detail, alike everywhere (`Detail::alike`), so
  // every cell's rays resume from its corner's line.
  beams[at + CELL_RESUMES] = 1u;
}
