// pxl-render/miss.wgsl

// The pixels the walk over the node cache alone left (miss_hit.wgsl), their ray asking the world
// (walk.wgsl, ask.wgsl, miss_word.wgsl): over their list, on from where that walk left them
// (`miss_list_cs`). It runs after `cast_cs` and `miss_hit_cs` in the cast's one pass, and with
// `miss_hit_cs` writes every marked pixel's words whole, so the mark never outlives the pass.

const WALK_PASS: u32 = MISSED_PASS;
const VISITS_COUNT: u32 = EYE_MISS_COUNT;
const ASKED_COUNT: u32 = EYE_ASKED_COUNT;

/// The hit at pixel `id`, entered through `face`, kept for what lies about that face to be
/// asked once a hit (miss_word.wgsl `hit_put`): numbered as the walks take them
/// (`node_cache::HITS_CNT`), the dispatch over them raised by the number that opens a workgroup
/// (`node_cache::HITS_ARGS`), and its number marked waiting (`beam::HIT_WAITS`) as its pixel's
/// last word until its faces are put there (`hit_faces_cs`); none past `beam::HITS`.
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&node_cache[HITS_CNT], 1u);
  if (r >= HITS) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&node_cache[HITS_ARGS], workgroups(r + 1u, HITS, HIT_LANES));
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Pixel `id`'s ray again, of rays `size` wide and high, on from where the walk over the node
/// cache left it (`write_left`), and its words written there (miss_word.wgsl): what lies about
/// the face it stopped on told at once where it needs no asking, a cell short of its cube
/// taking its box's (pxl-engine `face::BOX`), else its hit kept for that to be asked once a hit
/// (`keep_hit`).
fn walk_missed(id: vec2<u32>, size: vec2<u32>) {
  let aim = px_aim(id, size);
  let t = bitcast<f32>(textureLoad(rays, id, 1u).x);
  let start = px_start_at(aim, id, size, t);
  let w = walk(aim, start, STEPS_MOST);
  count_visits(w);
  let face = face_of(aim, w);
  var faces = 0u;
  // A cell short of its cube is its box's faces; a whole one's hit is kept to ask them. Both
  // of a `select` are taken, so the hit is kept under an `if`.
  if (w.ends == 1u && hit_fill(w) != 0u) {
    faces = FACE_BOX;
  } else if (w.ends == 1u) {
    faces = keep_hit(w, face, id);
  }
  write_ray(id, ray_word(w, face, faces));
}

/// Word `i` of the beam rays' words, where the walked pixels lie (walked.wgsl).
fn beam_at(i: u32) -> u32 {
  return select(0u, beams[i], i < arrayLength(&beams));
}

/// Each pixel `miss_hit_cs` left, a thread each at its place in their list (list.wgsl),
/// each past the count returning at once.
@compute @workgroup_size(MISS_LANES)
fn miss_list_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = list_place(group.xy, lane.x);
  let size = textureDimensions(rays);
  let drawn = size.x * size.y;
  let at = walked_left_at(drawn);
  if (k >= in_room(beam_at(at), drawn)) {
    return;
  }
  let listed = beam_at(at + 1u + k);
  let id = vec2<u32>(listed & 0xffffu, listed >> 16u);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  walk_missed(id, size);
}
