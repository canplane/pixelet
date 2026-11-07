// pxl-render/grid.wgsl

// The eye's camera grid as its words lay it among the eye's (pxl-engine `beam::Grid`,
// `eye::GRID`): where a cell's rays start and where a sample's hit lies among the beam rays'
// words, and the cell a drawn pixel's ray goes through.

/// The first word of the screen's cell `c`'s start (`Grid::cell_at`).
fn cell_start(c: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_CELLS_AT) + (c.y * eye_word(GRID + GRID_SPAN) + c.x) * CELL_WORDS;
}

/// The first word of sample `s`'s hit (`Grid::sample_at`).
fn sample_word(s: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_SAMPLES_AT) + (s.y * eye_word(GRID + GRID_SAMPLES) + s.x) * SAMPLE_WORDS;
}

/// The tangents across and up where drawn pixel `p` of the `size` the rays are takes its
/// ray, its middle off by the eye's jitter, row 0 at the top: the one place a drawn pixel's
/// ray is worked out, every pass that walks or marks one reads it here.
fn px_tan(p: vec2<u32>, size: vec2<u32>) -> vec2<f32> {
  let jitter = vec2<f32>(eye_f32(JITTER), eye_f32(JITTER + 1u));
  let at = (vec2<f32>(p) + vec2<f32>(0.5) + jitter) / vec2<f32>(size) * 2.0;
  return vec2<f32>(at.x - 1.0, 1.0 - at.y) * vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
}

/// The screen's cell holding the point at the tangents `t` (`Grid::cell`).
fn grid_cell(t: vec2<f32>) -> vec2<u32> {
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let span = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let at = floor(vec2<f32>(t.x + tan_xy.x, tan_xy.y - t.y) * GRID_CELL_INVERSE);
  return min(vec2<u32>(max(at, vec2<f32>(0.0))), span - vec2<u32>(1u));
}
