// pxl-render/sample.eye.wgsl

// The eye's samples (eye_bind.wgsl): every one of its camera grid's lattice needed, a ray at
// the screen's tangents off its place by the offset the rolled lattice casts (roll.wgsl).

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

/// Whether this cast's samples go on from where the cast before parked them, at the same
/// offset (`node_cache::ROLL_RESUMES`).
fn samples_resume() -> bool {
  return node_cache_word(ROLL_RESUMES) != 0u;
}

/// The offset this cast casts the samples at, across and down (`beam::roll`,
/// `node_cache::ROLL_AT`).
fn rolled() -> vec2<f32> {
  return vec2<f32>(bitcast<f32>(node_cache_word(ROLL_AT)), bitcast<f32>(node_cache_word(ROLL_AT + 1u)));
}

/// Off its place by this cast's offset of the rolled lattice (`Grid::sample`).
fn sample_aim(s: vec2<u32>) -> Aim {
  let roll = rolled();
  return eye_aim(vec2<f32>(
    -eye_f32(TAN) + (f32(s.x) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING + roll.x,
    eye_f32(TAN + 1u) - (f32(s.y) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING - roll.y,
  ));
}

/// The first word of its cell's start, the cell it lies in where its offset moves it back
/// past its place (`Grid::sample_cell`, `Grid::cell_at`).
fn sample_cell_word(s: vec2<u32>) -> u32 {
  let span_n = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let first = vec2<u32>(NEAR_SAMPLES) + select(vec2<u32>(0u), vec2<u32>(1u), rolled() < vec2<f32>(0.0));
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
