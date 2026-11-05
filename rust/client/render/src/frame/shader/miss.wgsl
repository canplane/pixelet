// pxl-render/miss.wgsl

// The pixels the walk over the observed pyramid alone left (miss_hit.wgsl), their ray again
// from their cell's start asking the world (ask.wgsl, miss_word.wgsl):
// over their list (`miss_list_cs`), and over every pixel where `cast_cs` found the list full
// (`miss_cs`). The two run after `cast_cs` in the cast's one pass and write every marked
// pixel's words whole, so the mark never outlives the pass.

const WALK_PASS: u32 = MISSED_PASS;
const VISITS_COUNT: u32 = EYE_MISS_COUNT;
const ASKED_COUNT: u32 = EYE_ASKED_COUNT;

/// The hit at pixel `id`, entered through `face`, kept for what lies about that face to be
/// asked once a hit (miss_word.wgsl `hit_put`): numbered as the walks take them
/// (`pyramid::HITS_CNT`), the dispatch over them raised by the number that opens a workgroup
/// (`pyramid::HITS_ARGS`), and its number marked waiting (`beam::HIT_WAITS`) as its pixel's
/// last word until its faces are put there (`hit_faces_cs`); none past `beam::HITS`.
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&pyramid[HITS_CNT], 1u);
  if (r >= HITS) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&pyramid[HITS_ARGS], r / HIT_LANES + 1u);
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Pixel `id`'s ray again, of rays `size` wide and high, and its words written there
/// (miss_word.wgsl): what lies about the face it stopped on told at once
/// where it needs no asking, a cell short of its cube taking its box's (pxl-engine
/// `face::BOX`), else its hit kept for that to be asked
/// once a hit (`keep_hit`).
fn walk_missed(id: vec2<u32>, size: vec2<u32>) {
  let aim = px_aim(id, size);
  let start = px_start(aim, id, size);
  let w = walk_asking(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, start);
  let face = face_of(aim, w);
  var faces = 0u;
  // A cell short of its cube is its box's faces; a whole one's hit is kept to ask them. Both
  // of a `select` are taken, so the hit is kept under an `if`.
  if (w.ends == 1u && hit_fill(w) != 0u) {
    faces = FACE_BOX;
  } else if (w.ends == 1u) {
    faces = keep_hit(w, face, id);
  }
  write_ray(id, ray_word(w, w.visits + start.visits, face, faces));
}

/// Each pixel `cast_cs` marked missed and could not list, a thread a drawn pixel, each but the
/// marked returning at once, dispatched only where the list was full
/// (`ray::MISS_ARGS_WHOLE`), after the list's walk, so a listed pixel it finds is no longer
/// marked.
@compute @workgroup_size(CAST_SIDE, CAST_SIDE, 1)
fn miss_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(rays);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  if (textureLoad(rays, id.xy, 0u).x != RAY_MISS) {
    return;
  }
  walk_missed(id.xy, size);
}

/// Each pixel `miss_hit_cs` left, a thread each, `MISS_LANES` a workgroup and `MISS_ACROSS`
/// workgroups a row, each past the count returning at once.
@compute @workgroup_size(MISS_LANES)
fn miss_list_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  let at = eye_word(GRID + GRID_MISSES_AT);
  if (k >= beams[at]) {
    return;
  }
  let listed = beams[at + 1u + k];
  let id = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let size = textureDimensions(rays);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  walk_missed(id, size);
}
