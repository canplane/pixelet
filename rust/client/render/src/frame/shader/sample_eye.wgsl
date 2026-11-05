// pxl-render/sample_eye.wgsl

// The eye as the samples' walks take it (eye_bind.wgsl): each sample its thread's,
// every one needed, a ray at the screen's tangents.

const WALK_PASS: u32 = SAMPLES_PASS;
const VISITS_COUNT: u32 = EYE_SAMPLE_COUNT;
const ASKED_COUNT: u32 = EYE_ASKED_COUNT;

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(GRID + GRID_SAMPLES), eye_word(GRID + GRID_SAMPLES + 1u));
}

fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return id;
}

fn sample_needed(s: vec2<u32>) -> bool {
  return true;
}

fn sample_word(s: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_SAMPLES_AT) + (s.y * sample_lattice().x + s.x) * SAMPLE_WORDS;
}

fn left_at() -> u32 {
  return eye_word(GRID + GRID_LEFTS_AT);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  let cell_samples = SAMPLE * eye_word(GRID + GRID_LATTICE);
  let spacing = eye_f32(GRID + GRID_S) / f32(cell_samples);
  return eye_aim(vec2<f32>(
    -eye_f32(TAN) + (f32(s.x) - f32(NEAR_SAMPLES)) * spacing,
    eye_f32(TAN + 1u) - (f32(s.y) - f32(NEAR_SAMPLES)) * spacing,
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

/// Off its cell.
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return resumed(aim, sample_cell_word(s), LINE_WORDS, NONE);
}
