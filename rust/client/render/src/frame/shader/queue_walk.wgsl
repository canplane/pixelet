// pxl-render/frame/shader/queue_walk.wgsl

// A run's two walks over the node cache alone (walk.wgsl, cached.wgsl): the first over the
// run's items, and the second over the rays the first parked. Each walk is a module of its own:
// its side (queue_walk.first.wgsl, queue_walk.parked.wgsl) names the items it takes, where each
// starts and where its asks are counted. The part for the run's kind names its items and gives
// their rays, starts and ends (queue.wgsl for the holes). The walks
// are a fixed dispatch (pxl-engine `ray::QUEUE_GROUP_CNT`). Each lane takes the next item when its
// own ray ends or is parked, walks it `QUEUE_STEP_CNT` steps at a time, and holds its state
// between them, so a lane whose ray ends early walks another while the long ones go on. A ray
// that meets a node no slot keeps is parked in the state its walk left, and the node is listed
// for the asks (queue_ask.wgsl). No thread of the pass writes what it reads of the node cache.

/// The run's queue, bound from its head (pxl-engine `ray::QUEUE_WORDS`, `ray::queue_at`). Past
/// the heads lie the asks' records, which every queue shares (`queue_recs_at`).
@group(0) @binding(10) var<storage, read_write> queue: array<atomic<u32>>;

/// What the ray that ended as `w` shows (queue.wgsl `finish`): what it met. These walks read the
/// node cache alone and cannot ask the world, so a ray they end as a lack, which happens only
/// where it finds no room to park, shows nothing, where the last walk shows a lack's deepest node held
/// (queue_last.wgsl `shows`). The cache does not hold that node as a cell: the walk passed it
/// as a split. To find it these walks would take the world's query whole, and they are kept to
/// the cache so that their pipelines stay small: an Android phone refuses a pipeline with too many
/// words live across its loop (cached.wgsl), and a driver that inlines writes a query asked from
/// two places twice. A pool runs out only where a frame parks more than `ray::POOL_CAP` holes.
fn shows(aim: Aim, w: WalkEnd) -> WalkEnd {
  return w;
}

/// Takes the next number from the queue's counter at `word`. `load` reads the counter without
/// taking.
fn take(word: u32) -> u32 {
  return atomicAdd(&queue[word], 1u);
}

fn load(word: u32) -> u32 {
  return atomicLoad(&queue[word]);
}

/// Takes the next hit's number (queue.wgsl `keep_hit`). The count and the dispatch over the
/// hits, which `hit_rows` raises to `rows`, lie in the queue's words, laid out as
/// `node_cache::HITS_ARGS` (`ray::QUEUE_HITS`). The last walk goes on numbering from them in
/// the node cache's head.
fn hit_number() -> u32 {
  return atomicAdd(&queue[QUEUE_HITS + 3u], 1u);
}

fn hit_rows(rows: u32) {
  atomicMax(&queue[QUEUE_HITS], rows);
}

/// Lists for the asks the node at which an item's walk `w` was left, which no slot keeps. It is
/// listed for the asks after this walk (`queue_asks_at`), and only where
/// the node cache keeps such a node at all (`keeps`). The record holds the node's zoom, its
/// index, the zooms asked, and what was handed to its parent. That is read from the slot that
/// keeps the parent at the zooms the ray asked it at, as the walk went through the parent.
/// Where no slot keeps the parent, the node is asked from the top.
fn list_ask(aim: Aim, w: WalkEnd) {
  let r0 = root_of(aim.origin);
  let c = idx_of(w.corner, w.zoom, w.roots, r0);
  if (!keeps(w.zoom, c)) {
    return;
  }
  let r = atomicAdd(&queue[queue_asks_at()], 1u);
  if (r >= QUEUE_RECS_CAP) {
    return;
  }
  var top = true;
  var inheritance = vec3<u32>(0u);
  if (w.zoom != 0u) {
    let pc = w.corner & vec3<u32>(~under(w.zoom - 1u));
    let p = node_answer(vec3<u32>(0u), true, w.zoom - 1u, idx_of(pc, w.zoom - 1u, w.roots, r0), node_range(aim, pc, w.zoom - 1u, w.roots), true);
    top = p.kind == KIND_LEFT;
    inheritance = p.inheritance;
  }
  let at = queue_recs_at() + r * QUEUE_REC_WORDS;
  atomicStore(&queue[at + QUEUE_REC_ZOOM], w.zoom);
  atomicStore(&queue[at + QUEUE_REC_IDX], u32(c.x));
  atomicStore(&queue[at + QUEUE_REC_IDX + 1u], u32(c.y));
  atomicStore(&queue[at + QUEUE_REC_IDX + 2u], u32(c.z));
  atomicStore(&queue[at + QUEUE_REC_ZOOMS], node_range(aim, w.corner, w.zoom, w.roots));
  atomicStore(&queue[at + QUEUE_REC_TOP], select(0u, 1u, top));
  atomicStore(&queue[at + QUEUE_REC_INHERITANCE], inheritance.x);
  atomicStore(&queue[at + QUEUE_REC_INHERITANCE + 1u], inheritance.y);
  atomicStore(&queue[at + QUEUE_REC_INHERITANCE + 2u], inheritance.z);
}

/// The lanes of the walk. Each lane holds one ray at a time. A ray that finds the pool full ends
/// as a lack where it stands.
fn queue_walk() {
  loop {
    let item = queue_next();
    if (item == NONE) {
      break;
    }
    let aim = item_aim(item);
    let s = queue_start(item, aim);
    var t = s.t;
    var corner = s.corner;
    var zoom = s.zoom;
    var roots = s.roots;
    var step = s.step;
    loop {
      let w = walk(aim, Start(t, corner, zoom, roots, vec3<u32>(0u), true, step), QUEUE_STEP_CNT);
      // Out of its steps, or left at a node no slot keeps (walk.wgsl `walk`).
      if (w.ends == ENDS_OUT) {
        t = w.t;
        corner = w.corner;
        zoom = w.zoom;
        roots = w.roots;
        step = w.step;
        continue;
      }
      if (w.ends == ENDS_AGAIN) {
        let start = item_start(item, aim);
        if (!park(item, start.t, start.corner, start.zoom, start.roots, start.step)) {
          finish(item, aim, to_lack(w));
        }
      } else if (w.ends == ENDS_LEFT) {
        if (park(item, w.t, w.corner, w.zoom, w.roots, w.step)) {
          list_ask(aim, w);
        } else {
          finish(item, aim, to_lack(w));
        }
      } else {
        finish(item, aim, w);
      }
      break;
    }
  }
}
