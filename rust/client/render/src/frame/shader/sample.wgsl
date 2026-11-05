// pxl-render/sample.wgsl

// The samples `sample_hit_cs` left, either observer's (sample.eye.wgsl, sample.light.wgsl),
// walked on from where it left them asking the world (ask.wgsl, `beam::SAMPLE_LEFT_T`), a thread
// each at its place in their list (list.wgsl).

@compute @workgroup_size(MISS_LANES, 1, 1)
fn sample_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let place = list_place(group.xy, lane.x);
  let at = left_at();
  let lattice = sample_lattice();
  if (place >= in_room(beams[at], lattice.x * lattice.y)) {
    return;
  }
  let listed = beams[at + 1u + place];
  let s = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let aim = sample_aim(s);
  let base = sample_word(s);
  let start = sample_start_at(aim, s, bitcast<f32>(beams[base + SAMPLE_LEFT_T]));
  let w = walk_asking(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, start);
  sample_put(base, w);
}
