// pxl-render/boxes.wgsl

// A stopped sample's box written for the raster's draw (raster.wgsl), the eye's samples'
// and the light's alike, over the samples' words (resume.wgsl `beams`) and their observer
// (sample_eye.wgsl, sample_light.wgsl) (pxl-engine raster.rs): the boxes and the raster's
// args.

@group(0) @binding(14) var<storage, read_write> boxes: array<u32>;
/// The raster's args (pxl-engine `raster::ARGS_INSTANCES`, `raster::ARGS_FACES`).
@group(0) @binding(16) var<storage, read_write> args: array<atomic<u32>>;

/// No sample before it, no box written.
const NO_BOX: u32 = 0xffffffffu;

fn box_put(i: u32, v: u32) {
  if (i < arrayLength(&boxes)) {
    boxes[i] = v;
  }
}

/// `v` added to word `i` of the args: what it held, 0 past them.
fn args_add(i: u32, v: u32) -> u32 {
  if (i < arrayLength(&args)) {
    return atomicAdd(&args[i], v);
  }
  return 0u;
}

fn args_most(i: u32, v: u32) {
  if (i < arrayLength(&args)) {
    atomicMax(&args[i], v);
  }
}

/// Whether samples at words `a` and `b` stopped on the same box, alike in every word a
/// pixel reads of them (pxl-engine `raster`).
fn alike(a: u32, b: u32) -> bool {
  for (var k = 0u; k < SAMPLE_FACES; k += 1u) {
    if (beams[a + k] != beams[b + k]) {
      return false;
    }
  }
  return true;
}

/// A box in units from the eye.
struct Box {
  lo: vec3<f32>,
  hi: vec3<f32>,
}

/// The node at word `k` as drawn from the origin `o`, the cell its word `asset` keeps
/// inset as it fills (pxl-engine cast.rs `asset_box`, `bounds`, `fill_bounds`).
fn asset_box(o: vec3<i32>, k: u32, asset: u32) -> Box {
  let og = vec3<u32>(u32(o.x), u32(o.y) + Y_START, u32(o.z) + Z_START);
  let corner = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  let roots = vec3<i32>(
    i32(beams[k + NODE_ROOTS]),
    i32(beams[k + NODE_ROOTS + 1u]),
    i32(beams[k + NODE_ROOTS + 2u]),
  );
  let lo = units(corner, og, roots);
  let hi = far_of(corner, under(beams[k + NODE_ZOOM]), og, roots);
  let eighths = (asset >> FILL_SHIFT) & (EIGHTHS - 1u);
  if (eighths == 0u) {
    return Box(lo, hi);
  }
  let inset = (hi - lo) * (0.5 * (1.0 - side(eighths)));
  return Box(lo + inset, hi - inset);
}

/// Whether the box lies so near the eye that the raster's near plane may cut it, `tan`
/// the screen's tangents (pxl-engine raster.rs `is_eyed`).
fn is_eyed(b: Box, tan_xy: vec2<f32>) -> bool {
  let v = select(select(vec3<f32>(0.0), b.hi, b.hi < vec3<f32>(0.0)), b.lo, b.lo > vec3<f32>(0.0));
  let k = 2.0 * RASTER_NEAR;
  return v.x * v.x + v.y * v.y + v.z * v.z < k * k * (1.0 + tan_xy.x * tan_xy.x + tan_xy.y * tan_xy.y);
}

/// The sample at word `k`, numbered `number`, written as a box from `origin`, appended
/// to the boxes and counted in the args, its number and its axis, `RASTER_EYED` where it
/// lies about the eye at the screen's tangents `tan` where `eyed` asks; none where it went
/// on to its reach, or where a sample at `before`, the one across or up, `NO_BOX` where
/// there is none, stopped alike. Its place among the boxes, `NO_BOX` where it wrote none
/// (pxl-engine `raster`, `cast::asset_box`).
fn write_box(k: u32, before: vec2<u32>, origin: vec3<i32>, number: u32, eyed: bool, tan_xy: vec2<f32>) -> u32 {
  if (beams[k + SAMPLE_NODE + NODE_ZOOM] == SAMPLE_NONE) {
    return NO_BOX;
  }
  if (before.x != NO_BOX && alike(k, before.x)) {
    return NO_BOX;
  }
  if (before.y != NO_BOX && alike(k, before.y)) {
    return NO_BOX;
  }
  let b = asset_box(origin, k + SAMPLE_NODE, beams[k + SAMPLE_ASSET]);
  let lacks = beams[k + SAMPLE_LACKS] != 0u;
  let near = eyed && is_eyed(b, tan_xy);
  let box_at = args_add(RASTER_ARGS_INSTANCES, 1u);
  let at = box_at * RASTER_BOX_WORDS;
  box_put(at + RASTER_BOX_LO, bitcast<u32>(b.lo.x));
  box_put(at + RASTER_BOX_LO + 1u, bitcast<u32>(b.lo.y));
  box_put(at + RASTER_BOX_LO + 2u, bitcast<u32>(b.lo.z));
  box_put(at + RASTER_BOX_HI, bitcast<u32>(b.hi.x));
  box_put(at + RASTER_BOX_HI + 1u, bitcast<u32>(b.hi.y));
  box_put(at + RASTER_BOX_HI + 2u, bitcast<u32>(b.hi.z));
  box_put(at + RASTER_BOX_ID, (number + 1u) | select(0u, RASTER_LACK, lacks));
  box_put(at + RASTER_BOX_AXIS, beams[k + SAMPLE_AXIS] | select(0u, RASTER_EYED, near));
  return box_at;
}

/// The sample the thread at `id` takes written as a box, the one across and up before it
/// each where the cast takes it, so it was sampled this cast: its place among the boxes,
/// `NO_BOX` where it wrote none.
fn sample_box(id: vec2<u32>, origin: vec3<i32>, eyed: bool, tan_xy: vec2<f32>) -> u32 {
  let n = sample_lattice();
  let s = sample_taken(id);
  if (s.x >= n.x || s.y >= n.y) {
    return NO_BOX;
  }
  var before = vec2<u32>(NO_BOX);
  if (s.x > 0u && sample_needed(s - vec2<u32>(1u, 0u))) {
    before.x = sample_word(s - vec2<u32>(1u, 0u));
  }
  if (s.y > 0u && sample_needed(s - vec2<u32>(0u, 1u))) {
    before.y = sample_word(s - vec2<u32>(0u, 1u));
  }
  return write_box(sample_word(s), before, origin, s.y * n.x + s.x, eyed, tan_xy);
}
