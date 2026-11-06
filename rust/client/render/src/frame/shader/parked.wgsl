// pxl-render/parked.wgsl

// A ray parked by the queue (queue_walk.wgsl, queue_last.wgsl), every item kind's alike: its
// distance's bits, its node's corner, and one word of its zoom, roots and steps taken under
// `PARKED` (pxl-engine `ray::STATE_WORDS`), each kind keeping them where its module says.

/// What an item's walk takes on its first walk of a run (`taken`): none, as it ended or is not
/// the run's; a fresh start; or the state it was parked in.
const TAKEN_NONE = 0u;
const TAKEN_FRESH = 1u;
const TAKEN_PARKED = 2u;

/// The word of a ray parked at node `zoom` with `roots`, `step` steps taken: the zoom over
/// `ZOOM_BITS`, three bits a root, the steps above, under `PARKED`.
fn parked_word(zoom: u32, roots: vec3<i32>, step: u32) -> u32 {
  let r = vec3<u32>(roots + vec3<i32>(3));
  return PARKED | zoom | (r.x << 6u) | (r.y << 9u) | (r.z << 12u) | (step << 15u);
}

/// The state a ray parked at `t` (its bits) in node `corner` under word `p` goes on from: its
/// node asked from the top, as no parent's handed words are parked.
fn parked_from(t: u32, corner: vec3<u32>, p: u32) -> Start {
  let r = vec3<i32>(vec3<u32>(p >> 6u, p >> 9u, p >> 12u) & vec3<u32>(7u)) - vec3<i32>(3);
  return Start(bitcast<f32>(t), corner, p & 63u, r, vec4<u32>(0u), true, (p & ~PARKED) >> 15u);
}
