// pxl-render/parked.wgsl

// How a ray is parked between a run's walks (queue_walk.wgsl, queue_last.wgsl), for every kind
// of item alike. A parked ray keeps the bits of its distance, its node's corner, and one word
// that holds its zoom, its roots and the steps it took, under the `PARKED` bit (pxl-engine
// `ray::STATE_WORDS`). Each kind keeps these words where its own part says (corner.wgsl,
// sample.wgsl, queue.wgsl).

/// The next number below `n` from the counter at `word`, or `NONE` where none is left. A lane
/// past the end reads the counter and does not write it.
fn take_below(word: u32, n: u32) -> u32 {
  if (load(word) >= n) {
    return NONE;
  }
  let k = take(word);
  return select(NONE, k, k < n);
}

/// The word of a ray parked at a node of `zoom` with `roots`, after `step` steps. The zoom
/// takes the low bits, then come three bits for each root, then the steps above them, all
/// under the `PARKED` bit.
fn parked_word(zoom: u32, roots: vec3<i32>, step: u32) -> u32 {
  let r = vec3<u32>(roots + vec3<i32>(3));
  return PARKED | zoom | (r.x << 6u) | (r.y << 9u) | (r.z << 12u) | (step << 15u);
}

/// The state from which a ray parked in node `corner` goes on, where `t` holds the bits of its
/// distance and `p` its parked word. Its node is asked from the top, as the words its parent
/// handed it are not parked.
fn parked_from(t: u32, corner: vec3<u32>, p: u32) -> Start {
  let r = vec3<i32>(vec3<u32>(p >> 6u, p >> 9u, p >> 12u) & vec3<u32>(7u)) - vec3<i32>(3);
  return Start(bitcast<f32>(t), corner, p & 63u, r, vec4<u32>(0u), true, (p & ~PARKED) >> 15u);
}
