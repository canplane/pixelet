// pxl-render/queue_last.wgsl

// A run's last walk (walk.wgsl, ask.wgsl), over the items its kind's module names (queue.wgsl
// for the holes): over the rays the walks over the node cache left parked, from the state each
// was parked in, asking the world where no slot keeps a node. The lanes as the walks before
// (queue_walk.wgsl), off a word of the node cache's head the asks before it set back
// (`node_cache::LAST_NEXT`). Every ray ends here: one that may ask no more, a lack at the
// deepest node held.

/// The next number off the node cache's counter at `word`, and the count it holds.
fn take(word: u32) -> u32 {
  return atomicAdd(&node_cache[word], 1u);
}

fn held(word: u32) -> u32 {
  return atomicLoad(&node_cache[word]);
}

/// The next stop's number (queue.wgsl `keep_hit`), on from the walks over the node cache's
/// (`node_cache::HITS_CNT`), and the dispatch over them raised to `rows`
/// (`node_cache::HITS_ARGS`).
fn hit_number() -> u32 {
  return atomicAdd(&node_cache[HITS_CNT], 1u);
}

fn hit_rows(rows: u32) {
  atomicMax(&node_cache[HITS_ARGS], rows);
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
      item = h;
      let s = parked_start(h);
      t = s.t;
      corner = s.corner;
      zoom = s.zoom;
      roots = s.roots;
      step = s.step;
      top = true;
    }
    let aim = item_aim(item);
    let w = walk(aim, Start(t, corner, zoom, roots, parent, top, step), QUEUE_STEPS);
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
      finish(item, aim, to_lack(w));
    } else {
      finish(item, aim, w);
    }
    item = NONE;
  }
  count_visits(visits_at(), visits);
}
