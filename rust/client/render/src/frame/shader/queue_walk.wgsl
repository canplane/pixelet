// pxl-render/queue_walk.wgsl

// The holes' walks over the node cache alone (walk.wgsl, cached.wgsl, queue.wgsl): the first
// over the queue's holes, a hole parked before on from its parked state, the second over the
// rays the first parked. A fixed dispatch (pxl-engine `ray::QUEUE_GROUPS`); each lane takes the next ray off
// the queue's words when its own ends or is parked, walks it `QUEUE_STEPS` steps at a time,
// and holds its state between them, so a lane whose ray ends early walks another while the
// long ones go on. A ray that meets a node no slot keeps is parked in the state its walk
// left, and the node listed for the asks (queue_ask.wgsl), with what was handed its parent
// where a slot keeps that; a ray the frame's budget of visits does not reach, up to the cap
// the observer's queue counts to (`ray::caps`), is parked where it is. Nothing it reads of the node cache any thread of the pass writes.

/// The queue's words (pxl-engine `ray::QUEUE_WORDS`).
@group(0) @binding(10) var<storage, read_write> queue: array<atomic<u32>>;

/// A stop listed for the record, what lies about its face put at its pixel off it (miss_word.wgsl
/// `hit_put`, record_stops.wgsl):
/// numbered as the walks take them, the dispatch over them raised by the number that opens a
/// workgroup (`ray::QUEUE_HITS`, which the last walk goes on from as `node_cache::HITS_ARGS`),
/// and its number marked waiting (`beam::HIT_WAITS`); none past its observer's room (`hits_room`).
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&queue[QUEUE_HITS + 3u], 1u);
  if (r >= hits_room()) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&queue[QUEUE_HITS], workgroups(r + 1u, hits_room(), HIT_LANES));
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

/// Whether the frame's node visits so far reach the cap this queue counts to (pxl-engine
/// `ray::VISITS_A_FRAME`, `ray::caps`), either observer's.
fn spent() -> bool {
  return atomicLoad(&queue[QUEUE_VISITED]) >= atomicLoad(&queue[QUEUE_CAPS]);
}

/// The node hole `hole`'s walk `w` was left at, no slot keeping it, listed for the asks of the
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
  let at = QUEUE_RECORDS_AT + (select(0u, QUEUE_RECORDS_ROOM, again) + r) * QUEUE_RECORD_WORDS;
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
/// first parked: each holds one ray at a time, `hole` `NONE` where it holds none. The first
/// takes every hole the queue holds, so every hole's state is written in this cast before the
/// second reads it: parked, or not.
fn queue_walk(again: bool) {
  let n = listed();
  let taken = select(QUEUE_TAKEN, QUEUE_TAKEN_AGAIN, again);
  var hole = NONE;
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var step = 0u;
  loop {
    if (hole == NONE) {
      let k = atomicAdd(&queue[taken], 1u);
      if (k >= n) {
        break;
      }
      let h = queued(k);
      // A hole parked before goes on from where it was parked: a light's slot of an owed tile,
      // whose claim took its state (light_propagate.wgsl `light_claim_cs`); an eye's pixel
      // whose rays stand, which its cast or the frame's queue alone left parked (cast.wgsl,
      // pxl-engine `node_cache::RESUMES`). Any other hole's state holds no parked ray.
      let resumes = again || parked(h);
      if (resumes && !parked(h)) {
        continue;
      }
      hole = h;
      if (resumes) {
        let s = parked_start(h);
        t = s.t;
        corner = s.corner;
        zoom = s.zoom;
        roots = s.roots;
        step = s.step;
      } else {
        let s = hole_start(h, hole_aim(h));
        t = s.t;
        corner = s.corner;
        zoom = s.zoom;
        roots = s.roots;
        step = s.step;
      }
    }
    if (spent()) {
      park(hole, t, corner, zoom, roots, step);
      hole = NONE;
      continue;
    }
    let aim = hole_aim(hole);
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
      park(hole, w.t, w.corner, w.zoom, w.roots, w.step);
      list_ask(aim, w, again);
    } else {
      finish(hole, aim, w);
    }
    hole = NONE;
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
