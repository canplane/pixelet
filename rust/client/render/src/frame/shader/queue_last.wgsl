// pxl-render/queue_last.wgsl

// A run's last walk (walk.wgsl, ask.wgsl), over the rays the walks over the node cache left
// parked, each from the state it was parked in. It asks the world where no slot keeps a node.
// The part for the run's kind names its items (queue.wgsl for the holes). The lanes take rays
// as the walks before do (queue_walk.wgsl), from a counter and a count in the node cache's head
// that the asks before set (`node_cache::LAST_NEXT`, `LAST_CNT`). Every ray ends here. A ray that may ask
// no more ends as a lack at the deepest node held.

/// What the ray that ended as `w` shows (queue.wgsl `finish`): what it met, or, where it ended
/// as a lack, the deepest node held, since what a frame defers is detail, never its answer.
/// The world is asked for the node the ray stands at as a cell of its own zoom, and where it
/// lacks that, for each node over it in turn, up to the root: a ray asks at most its zoom and
/// one more, and only as it ends as a lack. The frame's asks (`may_ask`) do not bound these:
/// a frame whose asks are spent still shows each lack's deepest node held, and the rays that
/// ask are bounded by those the walk takes: the holes' pool (`ray::POOL_CAP`), or the run's
/// items for the samples and the beam rays (item_list.wgsl). The ray stops where it stood. A
/// cell of nothing, or nothing held at any zoom, shows nothing.
fn shows(aim: Aim, w: WalkEnd) -> WalkEnd {
  if (w.ends != ENDS_LACK) {
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
      if (v == VAL_VOID || v == VAL_AIR) {
        return w;
      }
      return WalkEnd(ENDS_CELL, zoom, corner, w.roots, a.asset, w.axis, w.t, w.visits, w.parent, w.top, w.step);
    }
    if (a.kind != KIND_LACKS || zoom == 0u) {
      break;
    }
    // What is over a node is found from the top (`world_root`), as nothing holds what it inherits.
    zoom -= 1u;
    corner &= vec3<u32>(~under(zoom));
    parent = vec3<u32>(0u);
    top = true;
  }
  return w;
}

/// Takes the next number from the node cache's counter at `word`. `load` reads the counter
/// without taking.
fn take(word: u32) -> u32 {
  return atomicAdd(&node_cache[word], 1u);
}

fn load(word: u32) -> u32 {
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
  var visits = 0u;
  loop {
    let item = next_last();
    if (item == NONE) {
      break;
    }
    if (!parked(item)) {
      continue;
    }
    let aim = item_aim(item);
    let s = parked_start(item);
    var t = s.t;
    var corner = s.corner;
    var zoom = s.zoom;
    var roots = s.roots;
    var step = s.step;
    // What was handed the node's parent, held from one run of steps to the next.
    var parent = vec3<u32>(0u);
    var top = true;
    loop {
      let w = walk(aim, Start(t, corner, zoom, roots, parent, top, step), QUEUE_STEPS);
      visits += w.visits;
      // Out of its steps, or where it may ask no more (walk.wgsl `walk`).
      if (w.ends == ENDS_OUT) {
        t = w.t;
        corner = w.corner;
        zoom = w.zoom;
        roots = w.roots;
        step = w.step;
        parent = w.parent;
        top = w.top;
        continue;
      }
      if (w.ends == ENDS_LEFT) {
        finish(item, aim, to_lack(w));
      } else {
        finish(item, aim, w);
      }
      break;
    }
  }
  count_visits(visits_at(), visits);
}
