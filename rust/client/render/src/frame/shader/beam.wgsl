// pxl-render/beam.wgsl

// The beam rays `beam_hit_cs` left, either observer's (beam.eye.wgsl, beam.light.wgsl), each
// walked on from where that left it asking the world (beam_walk.wgsl, ask.wgsl). A thread each
// at its place in their list.

@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn beam_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // A row of workgroups a run of the list, a workgroup's lanes row by row, as beam_hit.wgsl
  // lists them.
  let place = (id.x / CORNER_SIDE) * CORNER_SIDE * CORNER_SIDE + (id.y % CORNER_SIDE) * CORNER_SIDE + id.x % CORNER_SIDE;
  let at = corners_left_at();
  if (place >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + place];
  let c = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let base = beam_line(c);
  let hand = beam_hand_at(c);
  let aim = beam_aim(c);
  // Where `beam_hit_cs` left it: its line's node at how far it came (resume.wgsl).
  let start = line_resumed(aim, base, bitcast<f32>(beams[hand]), NONE);
  let met = beam_walk(aim, beam_spacing(), base, start);
  if (node_cache_word(COUNT_ON) != 0u) {
    atomicAdd(&node_cache[VISITS_COUNT], met.visits);
  }
  beams[hand] = bitcast<u32>(beam_hand(met.t));
  beams[base + LINE_VISITS] = met.visits;
}
