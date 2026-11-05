// pxl-render/ask.wgsl

// A ray's walk, each node off the observed pyramid or asked of the world there and then and
// kept (pxl-engine `cast::walk`, `pyramid`). No stack: a node asked of the
// world finds what is handed it from the top (`world_root`). The pyramid's words all atomic, as
// a slot's key is exchanged to claim it: no compare-and-swap, which Safari's WGSL compiler
// writes Metal for that does not compile (doc/experiment/20261103-shaders-on-two-phones). A
// pass names what its walks count and are tagged by (`WALK_PASS`, `VISITS_COUNT`, `ASKED_COUNT`).

@group(0) @binding(9) var<storage, read_write> pyramid: array<atomic<u32>>;

fn pyramid_word(k: u32) -> u32 {
  return atomicLoad(&pyramid[k]);
}

/// This pass's tag (`pyramid::tag`).
fn walk_tag() -> u32 {
  return PASSES * pyramid_word(FRAME) + WALK_PASS;
}

/// Whether the entry at word `w` is the node (`is_node`).
fn is_node(w: u32, zoom: u32, c: vec3<i32>) -> bool {
  let n = w + W_NODE;
  return pyramid_word(n) == zoom
    && pyramid_word(n + 1u) == u32(c.x)
    && pyramid_word(n + 2u) == u32(c.y)
    && pyramid_word(n + 3u) == u32(c.z);
}

/// Whether this walk claims the slot whose key is word `w`, the key `was` when it read it
/// (`pyramid_claim`).
fn claim(w: u32, was: u32) -> bool {
  let got = atomicExchange(&pyramid[w], CLAIMED);
  if (got != was && got != CLAIMED) {
    atomicStore(&pyramid[w], got);
  }
  return got == was;
}

/// `pyramid::stamp` of an entry asked at `zooms`, far then near over `ZOOM_BITS`: a walk reads
/// and writes at once here, so takes a slot only where the words it read hash to it
/// (`pyramid::stamped`).
fn entry_stamp(kind: u32, zooms: u32, asset: vec2<u32>, handed: vec4<u32>, zoom: u32, c: vec3<i32>) -> u32 {
  var h = node_hash(zoom, c);
  h = mix_hash(h, zooms);
  h = mix_hash(h, kind);
  h = mix_hash(h, asset.x);
  h = mix_hash(h, asset.y);
  h = mix_hash(h, handed.x);
  h = mix_hash(h, handed.y);
  h = mix_hash(h, handed.z);
  h = mix_hash(h, handed.w);
  return (h ^ (h >> 16u)) & ~KIND_MASK;
}

/// What is in the node, as a walk reads it (`answer::In`).
struct In {
  kind: u32,
  asset: vec2<u32>,
}

/// No node entered: what the walk reads as no answer.
const KIND_NONE = 0xffffffffu;

/// The node `c` at `zoom`, asked at `zooms`, what is handed it found from the top (`ask_handed`).
fn ask(zoom: u32, c: vec3<i32>, zooms: u32) -> In {
  let a = ask_handed(world_root(zoom, c), zoom, c, zooms, true);
  return In(a.kind, a.asset);
}

/// The node `c` at `zoom`, handed `handed`, asked at `zooms`: off the slot that keeps it, unless
/// this pass is still writing it; else of the world, and kept where this walk claims a slot
/// (pxl-engine pyramid.rs, its claim `claim` here). What it hands its children: the slot's where one kept it. One asked of
/// the world counts in `ASKED_COUNT` where `counts`.
fn ask_handed(handed: vec4<u32>, zoom: u32, c: vec3<i32>, zooms: u32, counts: bool) -> Answer {
  let tag = walk_tag();
  let want = zooms << TAG_BITS;
  // The slot it would write and the key it read there: the node's own, else a free one,
  // else the one it takes over; none where a walk of this pass keeps it.
  var to = NONE;
  var key = CLAIMED;
  if (keeps(zoom, c)) {
    let first = bucket_of(zoom, c);
    var free = NONE;
    var k = 0u;
    for (; k < BUCKET; k += 1u) {
      let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
      let slot_key = pyramid_word(w);
      if (slot_key != CLAIMED && slot_key != 0u && is_node(w, zoom, c)) {
        if ((slot_key & KEY_ZOOMS) == want) {
          if ((slot_key & ~KEY_ZOOMS) != tag) {
            let at = w + W_INHERITANCE;
            let kept = vec4<u32>(pyramid_word(at), pyramid_word(at + 1u), pyramid_word(at + 2u), pyramid_word(at + 3u));
            let word = pyramid_word(w + W_KIND);
            let asset = vec2<u32>(pyramid_word(w + W_ASSET), pyramid_word(w + W_ASSET + 1u));
            if ((word & ~KIND_MASK) == entry_stamp(word & KIND_MASK, zooms, asset, kept, zoom, c)) {
              return Answer(word & KIND_MASK, asset, kept);
            }
          }
          to = NONE;
          free = NONE;
          break;
        }
        // Asked at other zooms by an earlier frame: written over. One this frame asked so
        // stands beside it.
        if ((slot_key & ~KEY_ZOOMS) / PASSES < tag / PASSES) {
          to = first + k;
          key = slot_key;
        }
      } else if (slot_key == 0u && free == NONE) {
        free = first + k;
      }
    }
    if (to == NONE && free != NONE) {
      to = free;
      key = 0u;
    } else if (to == NONE && k == BUCKET) {
      to = first + ((node_hash(zoom, c) >> 28u) & (BUCKET - 1u));
      key = pyramid_word(ENTRIES_AT + to * ENTRY_WORDS);
    }
  }
  if (counts && pyramid_word(COUNT_ON) != 0u) {
    atomicAdd(&pyramid[ASKED_COUNT], 1u);
  }
  let a = world_node(handed, zoom, c, zooms & ((1u << ZOOM_BITS) - 1u), zooms >> ZOOM_BITS, eye_cut(CARVE), eye_cut(HIDE));
  if (to != NONE && key != CLAIMED) {
    let w = ENTRIES_AT + to * ENTRY_WORDS;
    if (claim(w, key)) {
      atomicStore(&pyramid[w + W_KIND], a.kind | entry_stamp(a.kind, zooms, a.asset, a.handed, zoom, c));
      atomicStore(&pyramid[w + W_ASSET], a.asset.x);
      atomicStore(&pyramid[w + W_ASSET + 1u], a.asset.y);
      atomicStore(&pyramid[w + W_INHERITANCE], a.handed.x);
      atomicStore(&pyramid[w + W_INHERITANCE + 1u], a.handed.y);
      atomicStore(&pyramid[w + W_INHERITANCE + 2u], a.handed.z);
      atomicStore(&pyramid[w + W_INHERITANCE + 3u], a.handed.w);
      atomicStore(&pyramid[w + W_NODE], zoom);
      atomicStore(&pyramid[w + W_NODE + 1u], u32(c.x));
      atomicStore(&pyramid[w + W_NODE + 2u], u32(c.y));
      atomicStore(&pyramid[w + W_NODE + 3u], u32(c.z));
      atomicStore(&pyramid[w], tag | want);
    }
  }
  return a;
}

/// The zooms the ray, where it is at `t`, asks of the node, far then near over `ZOOM_BITS`
/// (`ask_zooms`, `node_dist_sq`); `NONE` where it has no span in it: touched at a face or an
/// edge alone, it is not entered.
fn zooms_asked(
  corner: vec3<u32>,
  zoom: u32,
  roots: vec3<i32>,
  og: vec3<u32>,
  dir: vec3<f32>,
  inv: vec3<f32>,
  t: f32,
  t_max: f32,
  range: vec2<u32>,
) -> u32 {
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(zoom), og, roots);
  let s = span_of(dir, inv, lo, hi, t, t_max);
  if (min(t_max, smallest(s.far)) <= max(t, largest(s.near))) {
    return NONE;
  }
  let far_v = max(-lo, hi);
  let near_v = min(max(lo, vec3<f32>(0.0)), hi);
  return zoom_at(squared(far_v), range) | (zoom_at(squared(near_v), range) << ZOOM_BITS);
}

/// The ray from `origin` aimed `aimed`, its reciprocal `inv` (walk_node.wgsl `Aim`), reaching `t_max` at the zooms `range` holds it to, from
/// `start` (`cast::walk`, `Until::Opaque`): its nodes asked here, not counting the nodes its
/// beam ray visited. Holding across a query only what no node gives again.
fn walk_asking(origin: vec3<i32>, aimed: vec3<f32>, inv: vec3<f32>, t_max: f32, range: vec2<u32>, start: Start) -> Walked {
  let dir = ray_dir(aimed);
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  var t = start.t;
  var corner = start.corner;
  var zoom = start.zoom;
  var roots = start.roots;
  var visits = 0u;
  var ends = 3u;
  var asset = vec2<u32>(0u);
  var axis = 0u;
  var hit_t = t_max;
  // A float gone wrong ends here, at its reach, as `cast::walk_at_most` ends one; the walks of
  // tests/sample.rs take far fewer.
  for (var step = 0u; step < STEPS_MOST; step += 1u) {
    // Asked first, holding across it only what no node gives again.
    var kind = KIND_NONE;
    asset = vec2<u32>(0u);
    let zooms = zooms_asked(corner, zoom, roots, og, dir, inv, t, t_max, range);
    if (zooms != NONE) {
      // The root it starts in (`roots_of`), X's wrapped away as the world names it.
      let r0 = vec3<i32>(0i, 0i, i32(og.z) >> 31u);
      let a = ask(zoom, idx_of(corner, zoom, roots, r0), zooms);
      kind = a.kind;
      asset = a.asset;
      visits += 1u;
    }
    let u = under(zoom);
    let lo = units(corner, og, roots);
    let hi = far_of(corner, u, og, roots);
    let s = span_of(dir, inv, lo, hi, t, t_max);
    let t0 = max(t, largest(s.near));
    let t1 = min(t_max, smallest(s.far));
    var into = false;
    if (kind == KIND_SPLIT) {
      // Whether its span meets the box its content lies in (`meets_box`).
      let past = box_past(asset);
      let box_lo = units(corner + box_step(box_first(asset), u, 0u), og, roots);
      let box_hi = select(
        units(corner + box_step(past, u, BOX_STEPS - 1u), og, roots),
        hi,
        past >= vec3<u32>(BOX_STEPS),
      );
      let b = span_of(dir, inv, box_lo, box_hi, t0, t_max);
      into = max(t0, largest(b.near)) < min(t1, smallest(b.far));
    } else if (kind == KIND_CELL) {
      // What the ray starts inside it does not stop at.
      if (fills(asset.x) && !(all(lo <= vec3<f32>(0.0)) && all(vec3<f32>(0.0) < hi))) {
        let k = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
        // A cell keeping less than its cube is the box its share fills (`fill_bounds`).
        let inset = (hi - lo) * (0.5 * (1.0 - side(k)));
        let b = span_of(dir, inv, lo + inset, hi - inset, t, t_max);
        if (k == 0u || min(t_max, smallest(b.far)) > max(t, largest(b.near))) {
          let near = select(b.near, s.near, k == 0u);
          axis = crossing(dir, near, true);
          hit_t = past_origin(near[axis]);
          ends = 1u;
          break;
        }
      }
    } else if (kind == KIND_LACKS) {
      axis = crossing(dir, s.near, true);
      hit_t = past_origin(s.near[axis]);
      ends = 2u;
      break;
    }
    if (into) {
      // PUSH: the child holding where it is at `t0`, past each mid plane crossed.
      let half = (u >> 1u) + 1u;
      let mid = units(corner + vec3<u32>(half), og, roots);
      let crossed = select(
        select(mid <= vec3<f32>(0.0), mid * inv > vec3<f32>(t0), dir < vec3<f32>(0.0)),
        mid * inv <= vec3<f32>(t0),
        dir > vec3<f32>(0.0),
      );
      corner = corner | select(vec3<u32>(0u), vec3<u32>(half), crossed);
      zoom += 1u;
      t = t0;
    } else {
      if (t1 >= t_max) {
        hit_t = t1;
        break;
      }
      // ADVANCE: the corner stepped on the axis it leaves by, then POP.
      let e = crossing(dir, s.far, false);
      let up = dir[e] > 0.0;
      let old = corner[e];
      let new_c = select(old - u - 1u, old + u + 1u, up);
      t = max(t, t1);
      if ((up && old + u == 0xffffffffu) || (!up && old == 0u)) {
        roots[e] += select(-1i, 1i, up);
        corner = vec3<u32>(0u);
        zoom = 0u;
      } else {
        corner[e] = new_c;
        let h = 31u - countLeadingZeros(old ^ new_c);
        corner = corner & vec3<u32>(~((1u << h) - 1u));
        zoom = FINEST_ZOOM - h;
      }
    }
  }
  if (pyramid_word(COUNT_ON) != 0u) {
    atomicAdd(&pyramid[VISITS_COUNT], visits);
  }
  return Walked(ends, zoom, corner, roots, asset, axis, hit_t, visits);
}
