// pxl-render/sample.wgsl

// The samples `sample_hit_cs` left, either observer's (sample_eye.wgsl, sample_light.wgsl),
// walked on from where it left them asking the world (ask.wgsl, `beam::SAMPLE_LEFT_T`), a thread
// each at its place in their list (sample_eye.wgsl, sample_light.wgsl).

@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn sample_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // A row of workgroups a run of the list, a workgroup's lanes row by row, as sample_hit.wgsl
  // lists them.
  let place = (id.x / SAMPLE_SIDE) * SAMPLE_SIDE * SAMPLE_SIDE + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  let at = left_at();
  if (place >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + place];
  let s = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let aim = sample_aim(s);
  let base = sample_word(s);
  let start = sample_start_at(aim, s, bitcast<f32>(beams[base + SAMPLE_LEFT_T]));
  let w = walk_asking(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, start);
  sample_put(base, w, w.visits + beams[base + SAMPLE_VISITS]);
}
