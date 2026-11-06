// pxl-render/sample_hit.wgsl

// The samples' walk over the node cache alone (walk.wgsl, cached.wgsl), either observer's
// (sample.eye.wgsl, sample.light.wgsl). One it leaves is listed for `sample_cs`, which goes on
// from where it was left (pxl-engine `beam::SAMPLE_LEFT_T`); one that ends is put as that puts it
// (sample_put.wgsl).

/// `sample_cs`'s dispatch, then how many are listed (list.wgsl `Left`): only the
/// list's own words.
@group(0) @binding(10) var<storage, read_write> args: Left;

@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn sample_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = sample_item(id.xy);
  let lattice = sample_lattice();
  if (s.x >= lattice.x || s.y >= lattice.y) {
    return;
  }
  let base = sample_word(s);
  let aim = sample_aim(s);
  let w = walk(aim, sample_start(aim, s), STEPS_MOST);
  if (w.ends == 4u) {
    beams[base + SAMPLE_LEFT_T] = bitcast<u32>(w.t);
    // The list holds a place for each sample of the lattice, each listed once.
    let place = enlist(lattice.x * lattice.y);
    if (place < lattice.x * lattice.y) {
      beams[left_at() + 1u + place] = s.x | (s.y << 16u);
    }
    return;
  }
  sample_put(base, w);
}
