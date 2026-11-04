// pxl-render/beam.wgsl

// The eye's beam rays `beam_hit_cs` left, each walked from the root asking the world
// (beam_walk.wgsl), its line left in the eye's beam rays' buffer (pxl-engine
// `beam::Grid::corners`, pxl-shader eye.rs `Eye`). A thread each at its place in their list.

const WALK_PASS: u32 = BEAM_PASS;
const VISITS_TALLY: u32 = EYE_BEAM_TALLY;
const ASKED_TALLY: u32 = EYE_ASKED_TALLY;

@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn beam_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // A row of workgroups a run of the list, a workgroup's lanes row by row (eye.rs `left_at`).
  let place = (id.x / CORNER_SIDE) * CORNER_SIDE * CORNER_SIDE + (id.y % CORNER_SIDE) * CORNER_SIDE + id.x % CORNER_SIDE;
  let at = eye_word(GRID + GRID_CORNERS_AT);
  if (place >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + place];
  let c = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let corners = vec2<u32>(eye_word(GRID + GRID_CORNERS), eye_word(GRID + GRID_CORNERS + 1u));
  if (c.x >= corners.x || c.y >= corners.y) {
    return;
  }
  // Its tangents (`Grid::corner`).
  let past = f32(eye_word(GRID + GRID_PAST));
  let s = eye_f32(GRID);
  let tx = -eye_f32(TAN) + (f32(c.x) - past) * s;
  let ty = eye_f32(TAN + 1u) - (f32(c.y) - past) * s;
  let idx = c.y * corners.x + c.x;
  let base = idx * LINE_WORDS;
  let aim = eye_aim(vec2<f32>(tx, ty));
  // Its beam's rays the lattice's side apart in angle (eye.rs `eye_origin`'s `Spacing`).
  let met = beam_walk(aim.origin, aim.dir, aim.t_max, aim.range, vec2<f32>(0.0, s), base);
  if (pyramid_word(TALLY_ON) != 0u) {
    atomicAdd(&pyramid[VISITS_TALLY], met.visits);
  }
  // How far the rays about it start (`Grid::hand_at`, `Grid::hand_dist`).
  beams[corners.x * corners.y * LINE_WORDS + idx] = bitcast<u32>(met.t * eye_f32(GRID + GRID_KEEP));
  beams[base + LINE_VISITS] = met.visits;
}
