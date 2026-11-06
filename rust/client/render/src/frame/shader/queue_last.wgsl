// pxl-render/queue_last.wgsl

// The holes' last walk (walk.wgsl, ask.wgsl, queue.wgsl): over the rays the walks over the
// node cache left parked, from the state each was parked in, asking the world where no slot
// keeps a node. The lanes as the walks before (queue_walk.wgsl): each takes the next hole when
// its own ray ends, off a word of the node cache's head the asks before it set back
// (`node_cache::LAST_TAKEN`), a hole not parked passed. The eye's asks wherever no slot keeps a
// node, so every eye hole ends in its frame. The light's counts against the frame's budget
// where its walks before left it (`node_cache::VISITED`, `node_cache::ASKED`): a slot's ray
// the budget does not reach is carried (`unreached`).

const WALK_PASS: u32 = MISSED_PASS;

/// A stop listed for the record, what lies about its face put at its pixel off it (miss_word.wgsl
/// `hit_put`, record_stops.wgsl),
/// numbered on from the walks over the node cache's (`node_cache::HITS_CNT`), the dispatch over
/// them raised by the number that opens a workgroup (`node_cache::HITS_ARGS`), and its number
/// marked waiting (`beam::HIT_WAITS`); none past `beam::HITS`.
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

/// Whether a node may be asked of the world: the eye's always; the light's one more within
/// the frame's budget of asks (`light::ASKS_A_FRAME`), counted as it is taken.
fn may_ask() -> bool {
  return !light_run() || atomicAdd(&node_cache[ASKED], 1u) < ASKS_A_FRAME;
}

/// A light's slot's ray the frame's budget does not reach, at the node `corner`, `zoom`,
/// `roots`, `t` along it, `step` steps taken: its word left as the map held it, its ray parked
/// and carried to the light's next cast, first in its queue (`carried_out`), its word written
/// when it ends, which that cast's lay of the tiles read takes (gpu/shadow.rs `begin`).
fn unreached(hole: u32, t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>, step: u32) {
  park(hole, t, corner, zoom, roots, step);
  let j = atomicAdd(&node_cache[CARRIED], 1u);
  if (j < room()) {
    beams[carried_out() + 1u + j] = hole;
  }
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_last_cs() {
  let n = carried() + listed();
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
    if (light_run() && atomicLoad(&node_cache[VISITED]) >= VISITS_A_FRAME) {
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
