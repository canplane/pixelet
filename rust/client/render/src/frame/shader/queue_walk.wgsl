// pxl-render/queue_walk.wgsl

// A run's walks over the node cache alone (walk.wgsl, cached.wgsl), over the items its kind's
// module names, their rays, starts and ends (queue.wgsl for the holes): the first over the
// run's items, an item parked before on from its parked state, the second over the rays the
// first parked. A fixed dispatch (pxl-engine `ray::QUEUE_GROUPS`); each lane takes the next item
// its kind's module hands it (`next_item`), off the queue's words, when its own ray ends or is
// parked, walks it `QUEUE_STEPS` steps at a
// time, and holds its state between them, so a lane whose ray ends early walks another while
// the long ones go on. A ray that meets a node no slot keeps is parked in the state its walk
// left, and the node listed for the asks (queue_ask.wgsl), with what was handed its parent
// where a slot keeps that; a ray the frame's budget of visits does not reach, up to its run's
// caps (`ray::caps`), is parked where it is. Nothing it reads of the node cache any thread of
// the pass writes.

/// The run's queue's words, its head first (pxl-engine `ray::QUEUE_WORDS`, `ray::queue_at`), the
/// asks' records every queue shares past the heads (`queue_records_at`).
@group(0) @binding(10) var<storage, read_write> queue: array<atomic<u32>>;

/// The next number off the queue's counter at `word`, and the count it holds: the counters a
/// kind's module takes its items by (`next_item`) and numbers its parked rays by.
fn take(word: u32) -> u32 {
  return atomicAdd(&queue[word], 1u);
}

fn held(word: u32) -> u32 {
  return atomicLoad(&queue[word]);
}

/// The next stop's number, as these walks number the stops their items keep for the record
/// (queue.wgsl `keep_hit`), and the dispatch over them raised to `rows`: in the queue's words
/// (`ray::QUEUE_HITS`), which the last walk goes on from as `node_cache::HITS_ARGS`.
fn stop_number() -> u32 {
  return atomicAdd(&queue[QUEUE_HITS + 3u], 1u);
}

fn stop_rows(rows: u32) {
  atomicMax(&queue[QUEUE_HITS], rows);
}

/// Whether the frame's node visits so far reach the cap this run counts to (pxl-engine
/// `ray::VISITS_A_FRAME`, `ray::caps`).
fn spent() -> bool {
  return node_cache[VISITED] + atomicLoad(&queue[QUEUE_VISITED]) >= node_cache[queue_caps()];
}

/// The node an item's walk `w` was left at, no slot keeping it, listed for the asks of the
/// walk `again` or the first, where it is kept at all (`keeps`): its zoom, its index, the zooms
/// asked, and what was handed its parent, off the slot that keeps the parent at the zooms the
/// ray asked it at, a node the walk went through; else it is asked from the top.
fn list_ask(aim: Aim, w: Walked, again: bool) {
  let r0 = root_of(aim.origin);
  let c = idx_of(w.corner, w.zoom, w.roots, r0);
  if (!keeps(w.zoom, c)) {
    return;
  }
  let r = atomicAdd(&queue[select(QUEUE_ASKS, QUEUE_ASKS_AGAIN, again)], 1u);
  if (r >= QUEUE_RECORDS_ROOM) {
    return;
  }
  var top = true;
  var handed = vec4<u32>(0u);
  if (w.zoom != 0u) {
    let pc = w.corner & vec3<u32>(~under(w.zoom - 1u));
    let p = node_answer(vec4<u32>(0u), true, w.zoom - 1u, idx_of(pc, w.zoom - 1u, w.roots, r0), node_zooms(aim, pc, w.zoom - 1u, w.roots), true);
    top = p.kind == KIND_LEFT;
    handed = p.handed;
  }
  let at = queue_records_at() + r * QUEUE_RECORD_WORDS;
  atomicStore(&queue[at], w.zoom);
  atomicStore(&queue[at + 1u], u32(c.x));
  atomicStore(&queue[at + 2u], u32(c.y));
  atomicStore(&queue[at + 3u], u32(c.z));
  atomicStore(&queue[at + 4u], node_zooms(aim, w.corner, w.zoom, w.roots));
  atomicStore(&queue[at + 5u], select(0u, 1u, top));
  atomicStore(&queue[at + 6u], handed.x);
  atomicStore(&queue[at + 7u], handed.y);
  atomicStore(&queue[at + 8u], handed.z);
  atomicStore(&queue[at + 9u], handed.w);
}

/// The lanes of a walk over the node cache alone, the first, or the one `again` over what the
/// first parked: each holds one ray at a time, `item` `NONE` where it holds none. The first
/// takes every item the run holds, so every item's state is written in this run before the
/// second reads it: parked, or not.
fn queue_walk(again: bool) {
  var item = NONE;
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var step = 0u;
  loop {
    if (item == NONE) {
      let h = next_item(again, spent());
      if (h == NONE) {
        break;
      }
      let how = taken(h, again);
      if (how == TAKEN_NONE) {
        continue;
      }
      item = h;
      if (how == TAKEN_FRESH) {
        let s = item_start(h, item_aim(h));
        t = s.t;
        corner = s.corner;
        zoom = s.zoom;
        roots = s.roots;
        step = s.step;
      } else {
        let s = parked_start(h);
        t = s.t;
        corner = s.corner;
        zoom = s.zoom;
        roots = s.roots;
        step = s.step;
      }
    }
    if (spent()) {
      park(item, t, corner, zoom, roots, step);
      item = NONE;
      continue;
    }
    let aim = item_aim(item);
    let w = walk(aim, Start(t, corner, zoom, roots, vec4<u32>(0u), true, step), QUEUE_STEPS);
    atomicAdd(&queue[QUEUE_VISITED], w.visits);
    if (w.ends == 5u) {
      t = w.t;
      corner = w.corner;
      zoom = w.zoom;
      roots = w.roots;
      step = w.step;
      continue;
    }
    if (w.ends == 4u) {
      park(item, w.t, w.corner, w.zoom, w.roots, w.step);
      list_ask(aim, w, again);
    } else {
      finish(item, aim, w);
    }
    item = NONE;
  }
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_cs() {
  queue_walk(false);
}

@compute @workgroup_size(QUEUE_LANES)
fn queue_parked_cs() {
  queue_walk(true);
}
