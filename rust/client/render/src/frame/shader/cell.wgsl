// pxl-render/cell.wgsl

// Finds where the rays of each cell of the eye's camera grid start, one thread a cell
// (pxl-engine `beam::CELL_WORDS`). A cell's rays start at the least distance that the beam rays
// in its window of corners hand on (`beam::Grid::cell_window`, window.wgsl), and they resume from
// that corner's line. light_cell.wgsl does the same for the shadow map.
//
// The window is computed in f32, as the Rust computes it: a ceiling over a square root and a
// division by what varies from cell to cell, with the cell taken by its reciprocal
// (`grid::CELL_INVERSE`). Where the value under the ceiling falls within a few ulps of a whole
// number, a GPU's rounding may take the next window.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// The window about the screen's cell `c`, counted from the screen's left and top, whose least
/// start the cell's rays take. It is given as the window's side in corners, less two and halved
/// (`Grid::cell_window`, `Grid::window`). It is measured at the cell's corner farthest from the
/// screen's axis, as the slope grows outward.
fn cell_window(c: vec2<u32>) -> u32 {
  let tan_xy = vec2<f32>(eye_f32(EYE_TAN), eye_f32(EYE_TAN + 1u));
  let at = max(abs(-tan_xy + vec2<f32>(c) * GRID_CELL), abs(-tan_xy + vec2<f32>(c + 1u) * GRID_CELL));
  let radius = eye_f32(EYE_GRID + GRID_RADIUS);
  let radius_tan = eye_f32(EYE_GRID + GRID_RADIUS_TAN);
  let edge = sqrt(at.x * at.x + at.y * at.y);
  let beyond = (edge + radius_tan) / (1.0 - edge * radius_tan);
  let stretch = 1.0 + beyond * beyond;
  return u32(ceil(FRAC_1_SQRT_2 + radius * stretch * GRID_CELL_INVERSE));
}

@compute @workgroup_size(CELL_SIDE, CELL_SIDE, 1)
fn cell_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= eye_word(EYE_GRID + GRID_SPAN) || id.y >= eye_word(EYE_GRID + GRID_SPAN + 1u)) {
    return;
  }
  // The window is held to the last cell's, for which the grid's corners are laid (`Grid::of`,
  // `past`), as a GPU's ceiling may take the next window for that cell.
  let k = min(cell_window(id.xy), eye_word(EYE_GRID + GRID_PAST));
  let first = id.xy + vec2<u32>(eye_word(EYE_GRID + GRID_PAST)) - vec2<u32>(k);
  // How far each corner's rays start lies in one word a corner, past the lines
  // (`Grid::hand_at`).
  let least = window_least(eye_word(EYE_GRID + GRID_HANDS_AT), eye_word(EYE_GRID + GRID_CORNERS), first, 2u + 2u * k);
  let at = cell_start(id.xy);
  beams[at + CELL_T] = least.x;
  beams[at + CELL_CORNER] = least.y;
  // The eye's cells and corners all ask at the eye's one detail (`Detail::alike`), so the rays
  // of every cell resume from its corner's line.
  beams[at + CELL_RESUMES] = 1u;
}
