// pxl-render/miss_hit.wgsl

// The pixels `cast_cs` listed, their walk over the node cache alone (walk.wgsl).
// One it leaves stays marked, where it was left written (`write_left`), and is listed for
// `miss_list_cs` (miss.wgsl); one that ends is written as that writes it (miss_word.wgsl).

/// `miss_list_cs`'s dispatch and how many are listed, as list.wgsl's `Left` lays them; then
/// the kept hits' dispatch and how many (`node_cache::HITS_ARGS`, copied there after).
struct MissLeft {
  list: array<atomic<u32>, 3>,
  cnt: atomic<u32>,
  hits: array<atomic<u32>, 3>,
  hits_cnt: atomic<u32>,
}
@group(0) @binding(10) var<storage, read_write> args: MissLeft;

/// A hit kept for what lies about its face to be asked once a hit (`keep_hit`).
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&args.hits_cnt, 1u);
  if (r >= HITS) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&args.hits[0], workgroups(r + 1u, HITS, HIT_LANES));
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

@compute @workgroup_size(MISS_LANES, 1, 1)
fn miss_hit_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = list_place(group.xy, lane.x);
  if (k >= in_room(node_cache[MARKS_CNT], MARKS)) {
    return;
  }
  let listed = node_cache[MARKS_AT + k];
  let id = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let size = textureDimensions(rays);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let aim = px_aim(id, size);
  let w = walk(aim, px_start(aim, id, size), NONE, vec2<f32>(0.0));
  if (w.ends == 4u) {
    write_left(id, w.t, w.visits);
    let place = enlist(MARKS);
    if (place < MARKS) {
      beams[eye_word(GRID + GRID_MISSES_AT) + 1u + place] = listed;
    }
    return;
  }
  let face = face_of(aim, w);
  var faces = 0u;
  // A cell short of its cube is its box's faces; a whole one's hit is kept to ask them. Both
  // of a `select` are taken, so the hit is kept under an `if`.
  if (w.ends == 1u && hit_fill(w) != 0u) {
    faces = FACE_BOX;
  } else if (w.ends == 1u) {
    faces = keep_hit(w, face, id);
  }
  write_ray(id, ray_word(w, w.visits, face, faces));
}
