// pxl-render/grid.wgsl

// The eye's camera grid, read from its words among the eye's (pxl-engine `beam::Grid`,
// `eye::GRID`). It says where a cell's start and a sample's hit lie among the beam rays'
// words, and which cell a drawn pixel's ray goes through.

fn cell_start(c: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_CELLS_AT) + (c.y * eye_word(GRID + GRID_SPAN) + c.x) * CELL_WORDS;
}

fn sample_word(s: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_SAMPLES_AT) + (s.y * eye_word(GRID + GRID_SAMPLES) + s.x) * SAMPLE_WORDS;
}

/// The steps of the beam ray whose line the rays of cell `c` go on from (`beam::LINE_STEPS`),
/// which each of those rays counts as its own in the ray steps view (pxl-engine `eye::STEPS`).
fn beam_steps(c: vec2<u32>) -> u32 {
  return beams[beams[cell_start(c) + CELL_CORNER] * LINE_WORDS + LINE_STEPS];
}

/// The tangents across and up of the ray of drawn pixel `p`, in a target of `size` pixels with
/// row 0 at the top. The ray leaves the pixel's middle moved by the eye's jitter. This is the
/// one place a drawn pixel's ray is worked out, and every pass that walks or marks one reads it
/// here.
fn px_tan(p: vec2<u32>, size: vec2<u32>) -> vec2<f32> {
  let jitter = vec2<f32>(eye_f32(JITTER), eye_f32(JITTER + 1u));
  let at = (vec2<f32>(p) + vec2<f32>(0.5) + jitter) / vec2<f32>(size) * 2.0;
  return vec2<f32>(at.x - 1.0, 1.0 - at.y) * vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
}

/// The screen's cell that holds the point at tangents `t`, clamped to the grid (`Grid::cell`).
fn grid_cell(t: vec2<f32>) -> vec2<u32> {
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let span = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let at = floor(vec2<f32>(t.x + tan_xy.x, tan_xy.y - t.y) * GRID_CELL_INVERSE);
  return min(vec2<u32>(max(at, vec2<f32>(0.0))), span - vec2<u32>(1u));
}
