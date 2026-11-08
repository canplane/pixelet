// pxl-render/queue_last.wgsl

// A run's last walk (walk.wgsl, ask.wgsl), over the rays the walks over the node cache left
// parked, each from the state it was parked in. It asks the world where no slot keeps a node.
// The part for the run's kind names its items (queue.wgsl for the holes). The lanes take rays
// as the walks before do (queue_walk.wgsl), from a counter in the node cache's head that the
// asks before set back to 0 (`node_cache::LAST_NEXT`). Every ray ends here. A ray that may ask
// no more ends as a lack at the deepest node held.

/// What the ray that ended as `w` shows (queue.wgsl `finish`): what it met, or, where it ended
/// as a lack, the deepest node held, since what a frame defers is detail, never its answer.
/// The world is asked for the node the ray stands at as a cell of its own zoom, and where it
/// does not hold that, for each node over it in turn. The ray stops where it stood. The frame's
/// asks (`may_ask`) do not bound this, as a ray asks it only once, as it ends. A cell of
/// nothing, or nothing held at any zoom, shows nothing.
fn shows(aim: Aim, w: WalkEnd) -> WalkEnd {
  if (w.ends != 2u) {
    return w;
  }
  let r0 = root_of(aim.origin);
  var zoom = w.zoom;
  var corner = w.corner;
  var parent = w.parent;
  var top = w.top;
  loop {
    let c = idx_of(corner, zoom, w.roots, r0);
    let a = world_answer(parent, top, zoom, c, zoom | (zoom << ZOOM_BITS), NONE);
    if (a.kind == KIND_CELL) {
      let v = a.asset.x & VAL_MASK;
      if (v == VALUE_VOID || v == VALUE_AIR) {
        return w;
      }
      return WalkEnd(1u, zoom, corner, w.roots, a.asset, w.axis, w.t, w.visits, w.parent, w.top, w.step);
    }
    if (a.kind != KIND_LACKS || zoom == 0u) {
      break;
    }
    // What is over a node is found from the top (`world_root`), as nothing holds what it inherits.
    zoom -= 1u;
    corner &= vec3<u32>(~under(zoom));
    parent = vec4<u32>(0u);
    top = true;
  }
  return w;
}

/// Takes the next number from the node cache's counter at `word`. `held` reads the counter
/// without taking.
fn take(word: u32) -> u32 {
  return atomicAdd(&node_cache[word], 1u);
}

fn held(word: u32) -> u32 {
  return atomicLoad(&node_cache[word]);
}

/// Takes the next stop's number (queue.wgsl `keep_hit`), counting on from the stops the walks
/// over the node cache listed (`node_cache::HITS_CNT`). `hit_rows` raises the dispatch over the
/// stops to `rows` (`node_cache::HITS_ARGS`).
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
    // `ends` is 5 where the walk ran out of its steps, and 4 where it may ask no more
    // (walk.wgsl `walk`).
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
