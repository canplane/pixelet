// pxl-render/beam.wgsl

// The beam rays `beam_hit_cs` left, either observer's (beam.eye.wgsl, beam.light.wgsl), each
// walked on from where that left it asking the world (walk.wgsl, ask.wgsl). A thread each at
// its place in their list (list.wgsl).

@compute @workgroup_size(MISS_LANES, 1, 1)
fn beam_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let place = list_place(group.xy, lane.x);
  let at = corners_left_at();
  let lattice = beam_lattice();
  if (place >= in_room(beams[at], lattice.x * lattice.y)) {
    return;
  }
  let listed = beams[at + 1u + place];
  let c = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let hand = beam_hand_at(c);
  let aim = beam_aim(c);
  // Where `beam_hit_cs` left it: its line's node at how far it came (resume.wgsl).
  let met = walk(aim, line_resumed(aim, aim.line, bitcast<f32>(beams[hand]), NONE), STEPS_MOST);
  count_visits(met);
  beams[hand] = bitcast<u32>(beam_hand(met.t));
}
