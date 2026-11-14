// pxl-render/frame/shader/beam.eye.wgsl

// The eye's beam rays, as the beam rays' walks read them (corner.wgsl, eye_bind.wgsl). A beam ray
// goes through each corner of the camera grid (pxl-engine `beam::Grid::corner_lattice`), and every
// corner is walked, one thread a corner. A ray keeps its line in the eye's beam rays' buffer.

fn corner_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(EYE_GRID + GRID_CORNER_LATTICE), eye_word(EYE_GRID + GRID_CORNER_LATTICE + 1u));
}

/// How many beam rays a run takes: every corner of the grid. A corner's place in the run is
/// its index, row by row.
fn corner_cnt() -> u32 {
  let n = corner_lattice();
  return n.x * n.y;
}

fn corner_queued(k: u32) -> vec2<u32> {
  let n = corner_lattice();
  return vec2<u32>(k % n.x, k / n.x);
}

fn beam_idx(c: vec2<u32>) -> u32 {
  return c.y * corner_lattice().x + c.x;
}

fn beam_line(c: vec2<u32>) -> u32 {
  return beam_idx(c) * LINE_WORDS;
}

/// The word that holds how far the rays about corner `c` start (`Grid::start_dist_at`).
fn beam_start_dist_at(c: vec2<u32>) -> u32 {
  return eye_word(EYE_GRID + GRID_START_DISTS_AT) + beam_idx(c);
}

fn corner_state_at(c: vec2<u32>) -> u32 {
  return eye_word(EYE_GRID + GRID_CORNER_STATES_AT) + beam_idx(c) * STATE_WORDS;
}

/// Corner `c`'s ray, aimed at the corner's tangents (`Grid::corner`). It keeps its line at
/// `beam_line`.
fn beam_aim(c: vec2<u32>) -> Aim {
  let past = f32(eye_word(EYE_GRID + GRID_PAST));
  let aim = eye_aim(vec2<f32>(-eye_f32(EYE_TAN) + (f32(c.x) - past) * GRID_CELL, eye_f32(EYE_TAN + 1u) - (f32(c.y) - past) * GRID_CELL));
  return beam_ray(aim, beam_line(c), beam_spacing());
}

/// The eye's beam rays spread from one origin, a cell apart in angle and none across
/// (`cast::Spacing`, `grid::CELL`).
fn beam_spacing() -> vec2<f32> {
  return vec2<f32>(0.0, GRID_CELL);
}

/// Every line is held from the eye's one origin, so no line keeps an origin.
fn beam_lay_origin(base: u32, origin: vec3<i32>) {
}

/// How far the rays about a beam ray that stopped at `t` start (`Grid::start_dist`).
fn beam_start_dist(t: f32) -> f32 {
  return t * eye_f32(EYE_GRID + GRID_KEEP);
}
