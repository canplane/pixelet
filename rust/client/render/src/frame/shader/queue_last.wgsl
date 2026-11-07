// pxl-render/queue_last.wgsl

// A run's last walk (walk.wgsl, ask.wgsl), over the items its kind's module names (queue.wgsl
// for the holes): over the rays the walks over the node cache left parked, from the state each
// was parked in, asking the world where no slot keeps a node. The lanes as the walks before
// (queue_walk.wgsl): each takes the next item when its own ray ends, off a word of the node
// cache's head the asks before it set back (`node_cache::LAST_TAKEN`), as its kind's module
// hands it (`next_last`), an item not parked passed. A run counts against the frame's budget
// where the runs before it left it, up to its kind's caps (`node_cache::VISITED`,
// `node_cache::caps_at`): a ray the budget does not reach stays parked, as its kind's module
// leaves it (`unreached`), and so does one whose start
// still waits on its cell's (queue_walk.wgsl `item_waits`), walking nothing.

/// The next number off the node cache's counter at `word` (`node_cache::LAST_TAKEN`), and the
/// count it holds: the counters a kind's module takes its items by (`next_last`).
fn take(word: u32) -> u32 {
  return atomicAdd(&node_cache[word], 1u);
}

fn held(word: u32) -> u32 {
  return atomicLoad(&node_cache[word]);
}

/// The next stop's number, as this walk numbers the stops its items keep for the record
/// (queue.wgsl `keep_hit`), on from the walks over the node cache's (`node_cache::HITS_CNT`),
/// and the dispatch over them raised to `rows` (`node_cache::HITS_ARGS`).
fn stop_number() -> u32 {
  return atomicAdd(&node_cache[HITS_CNT], 1u);
}

fn stop_rows(rows: u32) {
  atomicMax(&node_cache[HITS_ARGS], rows);
}

/// Whether a node may be asked of the world: one more within the cap this run counts the
/// frame's asks to (`node_cache::caps_at`), counted as it is taken.
fn may_ask() -> bool {
  return atomicAdd(&node_cache[ASKED], 1u) < atomicLoad(&node_cache[queue_caps() + 1u]);
}

/// Where an item's last walk goes on from: its parked state, or a fresh start where it was
/// parked waiting on its cell's start, which is there now.
fn last_start(item: u32, fresh: bool) -> Start {
  if (fresh) {
    return item_start(item, item_aim(item));
  }
  return parked_start(item);
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_last_cs() {
  var item = NONE;
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
    if (item == NONE) {
      let h = next_last();
      if (h == NONE) {
        break;
      }
      if (!parked(h)) {
        continue;
      }
      let fresh = waiting(h);
      if (fresh && item_waits(h)) {
        unreached(h, 0.0, vec3<u32>(0u), WAITS_ZOOM, vec3<i32>(0), 0u);
        continue;
      }
      item = h;
      let s = last_start(h, fresh);
      t = s.t;
      corner = s.corner;
      zoom = s.zoom;
      roots = s.roots;
      step = s.step;
      top = true;
    }
    if (atomicLoad(&node_cache[VISITED]) >= atomicLoad(&node_cache[queue_caps()])) {
      unreached(item, t, corner, zoom, roots, step);
      item = NONE;
      continue;
    }
    let aim = item_aim(item);
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
      unreached(item, w.t, w.corner, w.zoom, w.roots, w.step);
    } else if (!finish(item, aim, w)) {
      unreached(item, w.t, w.corner, w.zoom, w.roots, w.step);
    }
    item = NONE;
  }
  count_visits(visits_at(), visits);
}
