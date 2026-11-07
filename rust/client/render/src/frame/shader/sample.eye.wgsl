// pxl-render/sample.eye.wgsl

// The eye's samples (eye_bind.wgsl): every one of its camera grid's lattice needed, a ray at
// the screen's tangents of its place.

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(GRID + GRID_SAMPLES), eye_word(GRID + GRID_SAMPLES + 1u));
}

/// The sample the thread at `id` of a pass over the lattice takes (sample_box.wgsl).
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return id;
}

/// The samples a run takes: the whole lattice, a sample's place its index row by row.
fn samples_listed() -> u32 {
  let n = sample_lattice();
  return n.x * n.y;
}

fn sample_queued(k: u32) -> vec2<u32> {
  let n = sample_lattice();
  return vec2<u32>(k % n.x, k / n.x);
}

/// At its place (`Grid::sample`).
fn sample_aim(s: vec2<u32>) -> Aim {
  return eye_aim(vec2<f32>(
    -eye_f32(TAN) + (f32(s.x) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
    eye_f32(TAN + 1u) - (f32(s.y) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
  ));
}

/// The first word of its cell's start (`Grid::sample_cell`, `Grid::cell_at`).
fn sample_cell_word(s: vec2<u32>) -> u32 {
  let span_n = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let first = vec2<u32>(NEAR_SAMPLES);
  let cell = min(
    (max(s, first) - first) / (SAMPLE * eye_word(GRID + GRID_LATTICE)),
    span_n - vec2<u32>(1u),
  );
  return cell_start(cell);
}

/// The first word of the line its cell's rays resume from.
fn sample_line(s: vec2<u32>) -> u32 {
  return beams[sample_cell_word(s) + CELL_CORNER] * LINE_WORDS;
}

/// Off its cell.
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return resumed(aim, sample_cell_word(s), LINE_WORDS, NONE);
}
