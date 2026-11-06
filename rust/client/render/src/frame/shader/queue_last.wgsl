// pxl-render/queue_last.wgsl

// The holes' last walk (walk.wgsl, ask.wgsl, queue.wgsl): over the rays the walks over the
// node cache left parked, from the state each was parked in, asking the world where no slot
// keeps a node. The lanes as the walks before (queue_walk.wgsl): each takes the next hole when
// its own ray ends, off a word of the node cache's head the asks before it set back
// (`node_cache::LAST_TAKEN`), a hole not parked passed. Either observer's counts against the
// frame's budget where the walks before left it, up to its queue's caps (`node_cache::VISITED`
// to `node_cache::ASKS_CAP`): a ray the budget does not reach stays parked and is owed
// (`unreached`).

const WALK_PASS: u32 = MISSED_PASS;

/// A stop listed for the record, what lies about its face put at its pixel off it (miss_word.wgsl
/// `hit_put`, record_stops.wgsl),
/// numbered on from the walks over the node cache's (`node_cache::HITS_CNT`), the dispatch over
/// them raised by the number that opens a workgroup (`node_cache::HITS_ARGS`), and its number
/// marked waiting (`beam::HIT_WAITS`); none past its observer's room (`hits_room`).
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&node_cache[HITS_CNT], 1u);
  if (r >= hits_room()) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&node_cache[HITS_ARGS], workgroups(r + 1u, hits_room(), HIT_LANES));
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Whether a node may be asked of the world: one more within the cap this queue counts the
/// frame's asks to (`node_cache::ASKS_CAP`), counted as it is taken.
fn may_ask() -> bool {
  return atomicAdd(&node_cache[ASKED], 1u) < atomicLoad(&node_cache[ASKS_CAP]);
}

/// A hole's ray the frame's budget does not reach, at the node `corner`, `zoom`, `roots`, `t`
/// along it, `step` steps taken: parked where its state lies, its word left as the cast wrote
/// it, and counted as its observer's owed work (`node_cache::OWED_AT`), which the renderer
/// reads back and wakes the next frame by. An eye's pixel keeps what the record drew there
/// (cast.wgsl), and the frame's queue alone takes it on while the rays stand. A light's slot's
/// tile is owed (pxl-engine `light::READ_OWED`), so the next claim takes it nearest first and
/// the ray goes on from there; its word written when it ends, which that cast's lay of the
/// tiles read takes (gpu/shadow.rs `begin`). The tile's read word is the claim's and the
/// slide's to write but for this bit, which every slot of the tile it sets sets alike.
fn unreached(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(hole, t, corner, zoom, roots, step);
  if (!light_run()) {
    // Listed for the queue that goes on next, at its number among the owed, which the host
    // copies in as that list's count (pxl-engine `ray::owed_list_at`). A hole is taken once a
    // walk, so the list holds each parked pixel once.
    let r = atomicAdd(&node_cache[EYE_OWED], 1u);
    if (r < room()) {
      beams[eye_owed_at() + WALKED_LIST + r] = hole;
    }
    return;
  }
  let tile = slot_texel(hole_at(hole)) / LIGHT_TILE;
  let w = head_word(LIGHT_HEAD_READS) + tile.y * head_word(LIGHT_HEAD_TILES) + tile.x;
  if (w < arrayLength(&beams)) {
    beams[w] = beams[w] | LIGHT_READ_OWED;
  }
  atomicAdd(&node_cache[LIGHT_OWED], 1u);
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_last_cs() {
  let n = listed();
  var hole = NONE;
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var step = 0u;
  // What was handed the node's parent, held from one run of steps to the next.
  var parent = vec4<u32>(0u);
  var top = true;
  var visits = 0u;
  loop {
    if (hole == NONE) {
      let k = atomicAdd(&node_cache[LAST_TAKEN], 1u);
      if (k >= n) {
        break;
      }
      let h = queued(k);
      if (!parked(h)) {
        continue;
      }
      hole = h;
      let s = parked_start(h);
      t = s.t;
      corner = s.corner;
      zoom = s.zoom;
      roots = s.roots;
      step = s.step;
      top = true;
    }
    if (atomicLoad(&node_cache[VISITED]) >= atomicLoad(&node_cache[VISITS_CAP])) {
      unreached(hole, t, corner, zoom, roots, step);
      hole = NONE;
      continue;
    }
    let aim = hole_aim(hole);
    let w = walk(aim, Start(t, corner, zoom, roots, parent, top, step), QUEUE_STEPS);
    atomicAdd(&node_cache[VISITED], w.visits);
    visits += w.visits;
    if (w.ends == 5u) {
      t = w.t;
      corner = w.corner;
      zoom = w.zoom;
      roots = w.roots;
      step = w.step;
      parent = w.parent;
      top = w.top;
      continue;
    }
    if (w.ends == 4u) {
      unreached(hole, w.t, w.corner, w.zoom, w.roots, w.step);
    } else {
      finish(hole, aim, w);
    }
    hole = NONE;
  }
  count_visits(visits_at(), visits);
}
