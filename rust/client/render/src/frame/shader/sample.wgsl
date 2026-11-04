// pxl-render/sample.wgsl

// The samples `sample_hit_cs` left, either observer's (sample_eye.wgsl, sample_light.wgsl),
// walked from their cells' starts asking the world (ask.wgsl), a thread each at its place in
// their list (pxl-shader observer.rs `sample`).

@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn sample_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // A row of workgroups a run of the list, a workgroup's lanes row by row (eye.rs `left_at`).
  let place = (id.x / SAMPLE_SIDE) * SAMPLE_SIDE * SAMPLE_SIDE + (id.y % SAMPLE_SIDE) * SAMPLE_SIDE + id.x % SAMPLE_SIDE;
  let at = left_at();
  if (place >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + place];
  let s = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let aim = sample_aim(s);
  let start = sample_start(aim, s);
  let w = walk_asking(aim.origin, aim.dir, aim.t_max, aim.range, start);
  sample_put(sample_word(s), w, w.visits + start.visits);
}
