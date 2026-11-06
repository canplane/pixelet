// pxl-render/sample.eye.wgsl

// The eye as the samples' walks take it (eye_bind.wgsl): each sample its thread's,
// every one needed, a ray at the screen's tangents.

const WALK_PASS: u32 = SAMPLES_PASS;
const VISITS_COUNT: u32 = EYE_SAMPLE_COUNT;

/// Where its asks are counted (pxl-engine `count::ASKED`).
/// Its walks ask whatever they meet: no budget is theirs.
fn may_ask() -> bool {
  return true;
}

fn asked_at() -> u32 {
  return EYE_ASKED_COUNT;
}

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(GRID + GRID_SAMPLES), eye_word(GRID + GRID_SAMPLES + 1u));
}

fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return id;
}

fn left_at() -> u32 {
  return eye_word(GRID + GRID_LEFTS_AT);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  return eye_aim(vec2<f32>(
    -eye_f32(TAN) + (f32(s.x) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
    eye_f32(TAN + 1u) - (f32(s.y) - f32(NEAR_SAMPLES)) * SAMPLE_SPACING,
  ));
}

/// The first word of its cell's start (`Grid::sample_cell`, `Grid::cell_at`).
fn sample_cell_word(s: vec2<u32>) -> u32 {
  let span_n = vec2<u32>(eye_word(GRID + GRID_SPAN), eye_word(GRID + GRID_SPAN + 1u));
  let cell = min(
    (max(s, vec2<u32>(NEAR_SAMPLES)) - vec2<u32>(NEAR_SAMPLES)) / (SAMPLE * eye_word(GRID + GRID_LATTICE)),
    span_n - vec2<u32>(1u),
  );
  return cell_start(cell);
}

/// The first word of the line its cell's rays resume from.
fn sample_line(s: vec2<u32>) -> u32 {
  return beams[sample_cell_word(s) + CELL_CORNER] * LINE_WORDS;
}

/// The band its ray asked: the eye's every ray asks a node by its distance from the eye
/// alone, so none (`record::Key`'s band 0).
fn sample_band(s: vec2<u32>) -> u32 {
  return 0u;
}

/// Off its cell.
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return resumed(aim, sample_cell_word(s), LINE_WORDS, NONE);
}

/// Off its cell at `t`, where its walk over the node cache left it.
fn sample_start_at(aim: Aim, s: vec2<u32>, t: f32) -> Start {
  return resumed_at(aim, sample_cell_word(s), t, LINE_WORDS, NONE);
}
