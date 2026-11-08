// pxl-render/light_aim.wgsl

// The light's head, a cell's start, a texel's ray and band and the word a slot's walk leaves,
// as the light's passes by hand take them (pxl-engine light.rs `cell`, `ray`, `Light::band`,
// `depth`, `word`).

@group(0) @binding(11) var<uniform> header: array<vec4<i32>, LIGHT_HEADER_VEC4S>;

fn header_word(k: u32) -> u32 {
  return u32(header[k / 4u][k % 4u]);
}

/// Whether the pass is the light's: a head naming a map (`light::HEADER_SIZE`); the eye's head,
/// all zero, names none.
fn light_run() -> bool {
  return header_word(LIGHT_HEADER_SIZE) != 0u;
}

fn light_v3(k: u32) -> vec3<f32> {
  return eye_v3(LIGHT + k);
}

/// Where the ray of world texel `w` starts (`Starts::start`): its steps summed in fixed
/// point, exactly, each split into its whole steps and its fraction.
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

/// The texels from the box's middle, `half` texels in, to tile `k`'s span on one axis, a whole
/// number (`light::band_texels2`).
fn gap(k: i32, half: i32) -> i32 {
  let t = i32(LIGHT_TILE);
  return max(max(k * t - half, half - (k + 1i) * t), 0i);
}

/// The first word of cell `c`'s start in the light's list (`light::cell`).
fn light_cell_at(c: vec2<u32>) -> u32 {
  return header_word(LIGHT_HEADER_CELLS) + (c.y * header_word(LIGHT_HEADER_CELLS_ACROSS) + c.x) * CELL_WORDS;
}

/// The slot's texel of the box (`Light::texel`).
fn slot_texel(slot: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(vec2<i32>(slot) - light_corner()) & vec2<u32>(header_word(LIGHT_HEADER_SIZE) - 1u);
}

fn light_corner() -> vec2<i32> {
  return vec2<i32>(i32(eye_word(LIGHT + LIGHT_CORNER_X)), i32(eye_word(LIGHT + LIGHT_CORNER_Y)));
}

/// The band of box tile `tile`, which may lie past the box (`Light::band`): its texels squared, a
/// whole number under 2^24, times the host's scale, one product, so every pass gives a tile
/// the same band.
fn tile_band(tile: vec2<i32>) -> u32 {
  return tile_band_in(tile, vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)));
}

/// `tile_band` under the eye's zooms `range` (`Light::band`).
fn tile_band_in(tile: vec2<i32>, range: vec2<u32>) -> u32 {
  let half = i32(header_word(LIGHT_HEADER_SIZE) / 2u);
  let x = gap(tile.x, half);
  let y = gap(tile.y, half);
  let s2 = f32(x * x + y * y) * eye_f32(LIGHT + LIGHT_BAND_SCALE);
  return zoom_in_splits(s2, range);
}

/// Whether every tile from `lo` to `hi`, each axis's both included, asks band `band`
/// (`tile_band`). A tile's band falls as its texels squared grow, a positive scale of them
/// taking the zoom off their exponent, and they are the sum of its axes' gaps squared, each
/// axis's gap nothing at the middle's tile and growing either way from it: so each tile
/// between asks a band between those of the one nearest the middle, the middle's clamped
/// on each axis, and the one farthest from it, each axis's end of the larger gap.
fn tiles_ask(lo: vec2<i32>, hi: vec2<i32>, band: u32) -> bool {
  let half = i32(header_word(LIGHT_HEADER_SIZE) / 2u);
  let near = clamp(vec2<i32>(half / i32(LIGHT_TILE)), lo, hi);
  let far = select(lo, hi, vec2<bool>(gap(hi.x, half) > gap(lo.x, half), gap(hi.y, half) > gap(lo.y, half)));
  return tile_band(near) == band && tile_band(far) == band;
}

/// The band of the tile a texel lies on (`Light::band`).
fn texel_band(texel: vec2<u32>) -> u32 {
  return tile_band(vec2<i32>(texel / LIGHT_TILE));
}

/// The word of a walk that stopped at `t` on `axis`: a share of the box's depth, its t times
/// the reach's reciprocal, the host's, pushed back as its face's axis is (`light::depth`,
/// `light::word`).
fn hit_word(t: f32, axis: u32) -> u32 {
  return depth_word(min(t * eye_f32(LIGHT + LIGHT_T_MAX_INVERSE) + light_v3(LIGHT_BIAS)[axis], 1.0));
}

/// The reciprocal of the light's rays' direction on each axis, the host's (`Light::inverse`).
fn light_inverse() -> vec3<f32> {
  return light_v3(LIGHT_INVERSE);
}

/// The ray of box texel `p`, which may lie past the box, asked at `band` (`light::ray`,
/// walk_node.wgsl `Aim`).
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

/// The ray of box texel `texel`, at its tile's band.
fn light_aim(texel: vec2<u32>) -> Aim {
  return light_ray(vec2<i32>(texel), texel_band(texel));
}

fn depth_word(d: f32) -> u32 {
  return min(u32(d * f32(LIGHT_DEPTH_ONE) + 0.5), LIGHT_DEPTH_ONE);
}

/// The word a slot's ended walk leaves: the far plane where it stopped at nothing.
fn walked_word(w: WalkEnd) -> u32 {
  return select(depth_word(1.0), hit_word(w.t, w.axis), w.ends == 1u);
}
