// pxl-render/beam.eye.wgsl

// The eye as its beam rays' walks take it (eye_bind.wgsl): a beam ray a corner of the camera
// grid (pxl-engine `beam::Grid::corners`), every one needed, each its thread's, its line in the
// eye's beam rays' buffer.

const WALK_PASS: u32 = BEAM_PASS;
const VISITS_COUNT: u32 = EYE_BEAM_COUNT;
const ASKED_COUNT: u32 = EYE_ASKED_COUNT;

fn beam_lattice() -> vec2<u32> {
  return vec2<u32>(eye_word(GRID + GRID_CORNERS), eye_word(GRID + GRID_CORNERS + 1u));
}

fn beam_item(id: vec2<u32>) -> vec2<u32> {
  return id;
}

fn beam_idx(c: vec2<u32>) -> u32 {
  return c.y * beam_lattice().x + c.x;
}

/// The first word of corner `c`'s line (`beam::LINE_WORDS`).
fn beam_line(c: vec2<u32>) -> u32 {
  return beam_idx(c) * LINE_WORDS;
}

/// The word of how far the rays about corner `c` start (`Grid::hand_at`).
fn beam_hand_at(c: vec2<u32>) -> u32 {
  return eye_word(GRID + GRID_HANDS_AT) + beam_idx(c);
}

/// The list of the corners the walk over the node cache left (`Grid::corners_at`).
fn corners_left_at() -> u32 {
  return eye_word(GRID + GRID_CORNERS_AT);
}

/// Corner `c`'s ray, at its tangents (`Grid::corner`).
fn beam_aim(c: vec2<u32>) -> Aim {
  let past = f32(eye_word(GRID + GRID_PAST));
  let s = eye_f32(GRID + GRID_S);
  return eye_aim(vec2<f32>(-eye_f32(TAN) + (f32(c.x) - past) * s, eye_f32(TAN + 1u) - (f32(c.y) - past) * s));
}

/// Its beam's rays the lattice's side apart in angle (`cast::Spacing`, `beam::Grid`'s `s`).
fn beam_spacing() -> vec2<f32> {
  return vec2<f32>(0.0, eye_f32(GRID + GRID_S));
}

/// Every line is held from the eye's one origin: none kept in it.
fn beam_lay_origin(base: u32, origin: vec3<i32>) {
}

/// How far the rays about a beam ray that stopped at `t` start (`Grid::hand_dist`).
fn beam_hand(t: f32) -> f32 {
  return t * eye_f32(GRID + GRID_KEEP);
}
