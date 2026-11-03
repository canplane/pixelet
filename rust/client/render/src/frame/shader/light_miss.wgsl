// pxl-render/light_miss.wgsl

// The slots `light_miss_hit_cs` left, their walk from the root, each node off the observed
// pyramid or asked of the world there and then and kept (pxl-engine `cast::walk`,
// pxl-shader pyramid.rs `Writing`). No stack: a node asked of the world finds what is handed
// it from the top (`world_root`). The pyramid's words all
// atomic, as a slot's key is exchanged to claim it: no compare-and-swap, which Safari's
// WGSL compiler writes Metal for that does not compile
// (doc/experiment/20261103-shaders-on-two-phones).

@group(0) @binding(9) var<storage, read_write> pyramid: array<atomic<u32>>;
@group(0) @binding(12) var<storage, read> list: array<u32>;

fn pyramid_word(k: u32) -> u32 {
  return atomicLoad(&pyramid[k]);
}

/// This pass's tag (`pyramid::tag`).
fn walk_tag() -> u32 {
  return PASSES * pyramid_word(FRAME) + LIGHT_MISSED_PASS;
}

/// A cut as the eye holds it: its index, then its zoom.
fn eye_cut(k: u32) -> vec4<i32> {
  return vec4<i32>(i32(eye_word(k)), i32(eye_word(k + 1u)), i32(eye_word(k + 2u)), i32(eye_word(k + 3u)));
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

/// `pyramid::stamp` of an entry asked at `band`, far and near: a walk reads and writes at once
/// here, so takes a slot only where the words it read hash to it (`pyramid::stamped`).
fn entry_stamp(kind: u32, band: u32, asset: vec2<u32>, handed: vec4<u32>, zoom: u32, c: vec3<i32>) -> u32 {
  var h = node_hash(zoom, c);
  h = mix_hash(h, band | (band << ZOOM_BITS));
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

/// The node `c` at `zoom`, asked at `band`: off the slot that keeps it, unless this pass
/// is still writing it; else of the world, and kept where this walk claims a slot
/// (`Writing::node`).
fn ask(zoom: u32, c: vec3<i32>, band: u32) -> In {
  let tag = walk_tag();
  let want = (band << TAG_BITS) | (band << (TAG_BITS + ZOOM_BITS));
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
            let handed = vec4<u32>(pyramid_word(at), pyramid_word(at + 1u), pyramid_word(at + 2u), pyramid_word(at + 3u));
            let word = pyramid_word(w + W_KIND);
            let asset = vec2<u32>(pyramid_word(w + W_ASSET), pyramid_word(w + W_ASSET + 1u));
            if ((word & ~KIND_MASK) == entry_stamp(word & KIND_MASK, band, asset, handed, zoom, c)) {
              return In(word & KIND_MASK, asset);
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
  if (pyramid_word(TALLY_ON) != 0u) {
    atomicAdd(&pyramid[LIGHT_ASKED_TALLY], 1u);
  }
  let a = world_node(world_root(zoom, c), zoom, c, band, band, eye_cut(CARVE), eye_cut(HIDE));
  if (to != NONE && key != CLAIMED) {
    let w = ENTRIES_AT + to * ENTRY_WORDS;
    if (claim(w, key)) {
      atomicStore(&pyramid[w + W_KIND], a.kind | entry_stamp(a.kind, band, a.asset, a.handed, zoom, c));
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
  return In(a.kind, a.asset);
}

/// The ray's way, its axes it hardly moves along none (`Ray::new`).
fn ray_dir() -> vec3<f32> {
  let raw = light_v3(LIGHT_DIR);
  return select(raw, vec3<f32>(0.0), abs(raw) < vec3<f32>(1.17549435e-38));
}

fn ray_inv(dir: vec3<f32>) -> vec3<f32> {
  return select(vec3<f32>(1.0) / dir, vec3<f32>(0.0), dir == vec3<f32>(0.0));
}

/// Whether the ray, where it is at `t`, has a span in the node (`cast::walk`).
fn enters(corner: vec3<u32>, zoom: u32, roots: vec3<i32>, og: vec3<u32>, t: f32) -> bool {
  let dir = ray_dir();
  let t_max = eye_f32(LIGHT + LIGHT_T_MAX);
  let s = span_of(dir, ray_inv(dir), units(corner, og, roots), far_of(corner, under(zoom), og, roots), t, t_max);
  return min(t_max, smallest(s.far)) > max(t, largest(s.near));
}

/// The word the ray from `origin` leaves, its walk at `band` (`cast::walk`, `Until::Opaque`,
/// `light::depth`, `light::word`). What it does not hold from a node to the next it reads
/// again: the ray's way and reach off the eye.
fn slot_word(origin: vec3<i32>, band: u32) -> u32 {
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var visits = 0u;
  var word = depth_word(1.0);
  // A float gone wrong ends here, at the far plane; the walks of tests/light_miss.rs take
  // far fewer.
  for (var step = 0u; step < STEPS_MOST; step += 1u) {
    // Asked first, holding across it only what no node gives again; a node touched at a
    // face or an edge alone is not entered.
    var kind = KIND_NONE;
    var asset = vec2<u32>(0u);
    if (enters(corner, zoom, roots, og, t)) {
      // The root it starts in (`roots_of`), X's wrapped away as the world names it.
      let r0 = vec3<i32>(0i, 0i, i32(og.z) >> 31u);
      let a = ask(zoom, idx_of(corner, zoom, roots, r0), band);
      kind = a.kind;
      asset = a.asset;
      visits += 1u;
    }
    let dir = ray_dir();
    let inv = ray_inv(dir);
    let t_max = eye_f32(LIGHT + LIGHT_T_MAX);
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
          let axis = crossing(dir, near, true);
          word = stop_word(max(near[axis], 0.0), axis);
          break;
        }
      }
    } else if (kind == KIND_LACKS) {
      // What lacks tells nothing past it: the far plane.
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
  if (pyramid_word(TALLY_ON) != 0u) {
    atomicAdd(&pyramid[LIGHT_MISS_TALLY], visits);
  }
  return word;
}

@compute @workgroup_size(MISS_LANES)
fn light_miss_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let misses = misses_cnt();
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  if (k >= list[misses]) {
    return;
  }
  let listed = list[misses + 1u + k];
  if (listed == LIGHT_MISS_STRUCK) {
    return;
  }
  let slot = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let texel = slot_texel(slot);
  let word = slot_word(texel_start(light_corner() + vec2<i32>(texel)), texel_band(texel));
  textureStore(light, slot, 0, vec4<u32>(word, 0u, 0u, 0u));
}
