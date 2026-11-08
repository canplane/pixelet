// pxl-render/sample.eye.wgsl

// What a pass over samples reads of the eye's samples (eye_bind.wgsl). The eye takes every
// sample of its camera grid's lattice, and a sample's ray runs through the screen at the
// tangents of its place on the lattice (pxl-engine `beam::Grid::sample`).

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(GRID + GRID_SAMPLES), eye_word(GRID + GRID_SAMPLES + 1u));
}

/// The sample the thread at `id` takes in a pass over the lattice (sample_box.wgsl): the one
/// at `id` itself.
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return id;
}

/// The number of samples a run takes: the whole lattice. A sample's place in the run is its
/// index row by row (`sample_queued`).
fn sample_cnt() -> u32 {
  let n = sample_lattice();
  return n.x * n.y;
}

fn sample_queued(k: u32) -> vec2<u32> {
  let n = sample_lattice();
  return vec2<u32>(k % n.x, k / n.x);
}

/// The ray of sample `s`, through the screen at the tangents of its place, counted from
/// `NEAR_SAMPLES` samples beyond the screen's left and top (pxl-engine `beam::Grid::sample`).
fn sample_aim(s: vec2<u32>) -> Aim {
  return eye_aim(vec2<f32>(
    -eye_f32(TAN) + (f32(s.x) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
    eye_f32(TAN + 1u) - (f32(s.y) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
  ));
}

/// The first word of what the camera grid cell of sample `s` hands its rays (pxl-engine
/// `beam::Grid::sample_cell`, `beam::Grid::cell_at`). A sample beyond the screen takes the
/// nearest cell.
fn sample_cell_word(s: vec2<u32>) -> u32 {
  let span_n = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let first = vec2<u32>(NEAR_SAMPLES);
  let cell = min(
    (max(s, first) - first) / SAMPLE,
    span_n - vec2<u32>(1u),
  );
  return cell_start(cell);
}

/// The first word of the line from which the rays of sample `s`'s cell resume.
fn sample_line(s: vec2<u32>) -> u32 {
  return beams[sample_cell_word(s) + CELL_CORNER] * LINE_WORDS;
}

/// Where the ray of sample `s` starts its walk: at its cell's start, resumed from the cell's
/// line (resume.wgsl `resumed`).
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return resumed(aim, sample_cell_word(s), LINE_WORDS, NONE);
}
