// pxl-render/light_aim.wgsl

// What the light's passes read of the light's head and of its cast: where a cell's rays start,
// a texel's ray and band, and the word a slot's walk leaves. Each is the rule pxl-engine
// light.rs defines, written again here (`cell`, `ray`, `Light::band`, `depth`, `word`).

@group(0) @binding(11) var<uniform> header: array<vec4<i32>, LIGHT_HEADER_VEC4S>;

fn header_word(k: u32) -> u32 {
  return u32(header[k / 4u][k % 4u]);
}

fn light_v3(k: u32) -> vec3<f32> {
  return eye_v3(LIGHT + k);
}

/// Where the ray of world texel `w` starts (`StartGrid::start`). The steps are summed exactly in
/// fixed point, each split into its whole steps and its fraction, so every device takes the
/// same start.
fn texel_start(w: vec2<i32>) -> vec3<i32> {
  let base = header[LIGHT_STARTS_AT].xy;
  let d = w - base;
  let k = d >= vec2<i32>(i32(LIGHT_ANCHOR));
  let ij = d - select(vec2<i32>(0), vec2<i32>(i32(LIGHT_ANCHOR)), k);
  let anchor = u32(k.x) + 2u * u32(k.y);
  let at = header[LIGHT_STARTS_AT + STARTS_ANCHORS + anchor].xyz;
  let a = header[LIGHT_STARTS_AT + STARTS_ACROSS].xyz;
  let b = header[LIGHT_STARTS_AT + STARTS_DOWN].xyz;
  let frac = header[LIGHT_STARTS_AT + STARTS_FRACS + anchor].xyz;
  let m = vec3<i32>((1i << LIGHT_STEP_BITS) - 1i);
  let s = vec3<u32>(LIGHT_STEP_BITS);
  let low = ij.x * (a & m) + ij.y * (b & m) + frac + vec3<i32>(1i << (LIGHT_STEP_BITS - 1u));
  let high = ij.x * (a >> s) + ij.y * (b >> s);
  return at + high + (low >> s);
}

/// The whole number of texels on one axis from the box's middle, `half` texels in, to the span
/// of tile `k`. It is 0 for the tile that holds the middle (`light::band_texels2`).
fn gap(k: i32, half: i32) -> i32 {
  let t = i32(LIGHT_TILE);
  return max(max(k * t - half, half - (k + 1i) * t), 0i);
}

fn light_cell_at(c: vec2<u32>) -> u32 {
  return header_word(LIGHT_HEADER_CELLS) + (c.y * header_word(LIGHT_HEADER_CELLS_ACROSS) + c.x) * CELL_WORDS;
}

fn slot_texel(slot: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(slot) - light_corner()) & vec2<u32>(header_word(LIGHT_HEADER_SIZE) - 1u);
}

fn light_corner() -> vec2<i32> {
  return vec2<i32>(i32(eye_word(LIGHT + LIGHT_CORNER_X)), i32(eye_word(LIGHT + LIGHT_CORNER_Y)));
}

/// The band of box tile `tile`, which may lie past the box (`Light::band`). A tile's band is
/// the zoom every walk over it asks. It is taken from a whole number of texels squared, under
/// 2^24, times the host's scale, in one product, so every pass gives a tile the same band.
fn tile_band(tile: vec2<i32>) -> u32 {
  return tile_band_in(tile, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
}

fn tile_band_in(tile: vec2<i32>, range: vec2<u32>) -> u32 {
  let half = i32(header_word(LIGHT_HEADER_SIZE) / 2u);
  let x = gap(tile.x, half);
  let y = gap(tile.y, half);
  let s2 = f32(x * x + y * y) * eye_f32(LIGHT + LIGHT_BAND_SCALE);
  return zoom_in_splits(s2, range);
}

/// Whether every tile from `lo` to `hi`, both ends included on each axis, asks band `band`
/// (`tile_band`). A tile's band falls as its texels squared grow, as the band is taken off the
/// exponent of a positive scale of them. The texels squared are the sum of the squared gaps on
/// the two axes, and each gap is 0 at the middle's tile and grows either way from it. So every
/// tile in the range asks a band between the bands of two tiles: the one nearest the middle,
/// which is the middle clamped on each axis, and the one farthest from it, at the end of the
/// larger gap on each axis. Where those two ask `band`, every tile does.
fn tiles_ask(lo: vec2<i32>, hi: vec2<i32>, band: u32) -> bool {
  let half = i32(header_word(LIGHT_HEADER_SIZE) / 2u);
  let near = clamp(vec2<i32>(half / i32(LIGHT_TILE)), lo, hi);
  let far = select(lo, hi, vec2<bool>(gap(hi.x, half) > gap(lo.x, half), gap(hi.y, half) > gap(lo.y, half)));
  return tile_band(near) == band && tile_band(far) == band;
}

fn texel_band(texel: vec2<u32>) -> u32 {
  return tile_band(vec2<i32>(texel / LIGHT_TILE));
}

/// The word of a walk that stopped at `t` on a face across `axis`. The depth is a share of the
/// box's depth: `t` times the reciprocal of the reach that the host rounded, pushed back by the
/// bias of the face's axis (`light::depth`, `light::word`).
fn hit_word(t: f32, axis: u32) -> u32 {
  return depth_word(min(t * eye_f32(LIGHT + LIGHT_T_MAX_INVERSE) + light_v3(LIGHT_BIAS)[axis], 1.0));
}

/// The reciprocal of the light's rays' direction on each axis. The host rounds it once, so the
/// CPU and the GPU multiply by the same value (`Light::inverse`).
fn light_inverse() -> vec3<f32> {
  return light_v3(LIGHT_INVERSE);
}

/// The ray of box texel `p`, which may lie past the box, asking every node at `band`
/// (`light::ray`, walk_node.wgsl `Aim`).
fn light_ray(p: vec2<i32>, band: u32) -> Aim {
  return Aim(
    texel_start(light_corner() + p),
    light_v3(LIGHT_DIR),
    light_inverse(),
    eye_f32(LIGHT + LIGHT_T_MAX),
    vec2<u32>(band),
    UNTIL_OPAQUE,
    NONE,
    vec2<f32>(0.0),
  );
}

fn light_aim(texel: vec2<u32>) -> Aim {
  return light_ray(vec2<i32>(texel), texel_band(texel));
}

fn depth_word(d: f32) -> u32 {
  return min(u32(d * f32(LIGHT_DEPTH_ONE) + 0.5), LIGHT_DEPTH_ONE);
}

/// The word a slot's ended walk leaves. A walk that stopped at nothing leaves the far plane.
fn walked_word(w: WalkEnd) -> u32 {
  return select(depth_word(1.0), hit_word(w.t, w.axis), w.ends == ENDS_CELL);
}
