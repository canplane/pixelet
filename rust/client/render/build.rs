// pxl-render/build.rs

// The build writes the shaders the crate ships. Each WGSL file under src/frame/shader goes out
// with its comments and indentation stripped, and nothing in it is renamed. The constants and
// small functions the shaders share with pxl-engine and pxl-content are generated here from the
// Rust that defines them. Each shader is then assembled from its parts, cut to what its entry
// points reach, and validated by naga before the crate includes it.

use naga::front::wgsl;
use pxl_content::param::night;
use pxl_engine::answer;
use pxl_engine::beam::SAMPLE_SIDE;
use pxl_engine::grid;
use pxl_engine::ground;
use pxl_engine::lattice;
use pxl_engine::light::TILE as LIGHT_TILE;
use pxl_engine::param::light;
use pxl_engine::WORKGROUPS_CAP;
use pxl_spec::space::layer::lattice::EFFECT_REACH;
use std::path::Path;

#[path = "src/frame/shader/debug_look.spec.rs"]
mod debug_look;
#[path = "src/param.rs"]
#[expect(
    dead_code,
    reason = "the build reads part of what the crate shares with it"
)]
mod param;
#[path = "src/frame/shader/part.rs"]
#[expect(
    dead_code,
    reason = "the build reads part of what the crate shares with it"
)]
mod part;
#[path = "src/frame/shader/word.rs"]
#[expect(
    dead_code,
    reason = "the build reads part of what the crate shares with it"
)]
mod word;
use word::screen;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
    // The directory is emptied first, so a part deleted from src/frame/shader cannot be read
    // from an earlier build's copy.
    match std::fs::remove_dir_all(&out) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => panic!("the shader directory: {e}"),
        _ => {}
    }
    std::fs::create_dir_all(out.join("assembled")).expect("the shader directory");
    for entry in std::fs::read_dir(dir).expect("the shaders") {
        let path = entry.expect("a shader").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a shader's text");
        let name = path.file_name().expect("a shader's name");
        let mut shipped = strip_comments(&text);
        if name == "constant.wgsl" {
            shipped += &words();
            shipped += &params();
        }
        shipped += &accessors(&text);
        std::fs::write(out.join(name), shipped).expect("the stripped shader");
    }
    // The cast paints a ray by comparing its cell's zoom with the unit's zoom and with the
    // ground's leaf cell zoom, and places that cell from the eye's position in Q3 steps, whose
    // fraction has `FRAC_W` bits.
    let lattice = format!(
        "const UNIT_ZOOM: u32 = {}u;\nconst FRAC_W: u32 = {}u;\nconst GROUND_CELL_ZOOM: u32 = {}u;\n",
        pxl_engine::UNIT_ZOOM,
        pxl_engine::FRAC_W,
        lattice::GROUND.leaf_cell_zoom()
    );
    std::fs::write(out.join("lattice.wgsl"), lattice).expect("the lattice's shader");
    std::fs::write(out.join("walk_word.wgsl"), walk_word()).expect("the walk's words");
    std::fs::write(out.join("side.wgsl"), side_word()).expect("a box's side");
    std::fs::write(out.join("world_word.wgsl"), world_word()).expect("the world's words");
    std::fs::write(out.join("decor_word.wgsl"), decor_word()).expect("decor's words");
    // Accessors for the fields of a ray's first word, generated from the layout pxl-engine
    // ray.rs defines, so every shader reads a field exactly as the cast writes it.
    use pxl_engine::ray::*;
    let field = |name: &str, shift: u32, bits: u32| {
        format!(
            "fn ray_{name}(w: u32) -> u32 {{ return (w >> {shift}u) & {}u; }}\n",
            (1u32 << bits) - 1
        )
    };
    // A ray's faces word records what lies around the face it hit, one bit for each edge and
    // each corner, as pxl-engine face.rs lays it out.
    use pxl_engine::face::{CORNER_SHIFT, LINE_SHIFT, RISE_SHIFT};
    let faces = format!(
        "const RAY_FACES: u32 = {FACES}u;\nconst FACES_LINE: u32 = {LINE_SHIFT}u;\nconst FACES_RISE: u32 = {RISE_SHIFT}u;\nconst FACES_CORNER: u32 = {CORNER_SHIFT}u;\n"
    );
    // `fill_side` gives the side of the box drawn for a cell that fills only part of its cube,
    // by the number of eighths the cell keeps (pxl-engine answer.rs `side`). A fill of 0 eighths
    // means all eight are kept, which the default case answers.
    let sides: String = (1..answer::EIGHTHS)
        .map(|k| format!("case {k}u: {{ return {:?}; }} ", answer::side(k)))
        .collect();
    let fill = format!(
        "fn fill_side(k: u32) -> f32 {{ switch k {{ {sides}default: {{ return 1.0; }} }} }}\n"
    );
    // A coarse cell's ray carries the cell's four palette indices in its third word, packed as
    // pxl-engine ground.rs `palette_idxs` packs them.
    let b = ground::PALETTE_BITS;
    let m = (1u32 << b) - 1;
    let palette_idxs = format!(
        "fn ray_palette_idxs(w: u32) -> vec4u {{ return vec4u(w & {m}u, (w >> {b}u) & {m}u, (w >> {}u) & {m}u, w >> {}u); }}\n",
        2 * b,
        3 * b
    );
    let ray = [
        field("val", 0, VAL_BITS),
        field("face", FACE_SHIFT, FACE_BITS),
        field("zoom", ZOOM_SHIFT, ZOOM_BITS),
        field("fill", FILL_SHIFT, FILL_BITS),
        fill,
        faces,
        palette_idxs,
    ];
    std::fs::write(out.join("ray.wgsl"), ray.concat()).expect("the ray's shader");
    // The corners of a box's vertices, first as pxl-engine raster.rs `corner` winds them and
    // then wound the other way, are packed three bits a corner and ten corners a word, and
    // `raster_corner` reads them back. The Galaxy S23's Vulkan driver refuses a render pipeline
    // whose vertex stage indexes a constant array of vectors at a place computed as it runs, and
    // takes the same table packed into u32 words. A hit's box that holds the eye is drawn at a depth just short of 1, the
    // depth of a lack. Every float written here is finite, as Tint refuses a literal past f32's
    // range.
    {
        use pxl_engine::raster::{corner, EYED, ID_MASK, LACK, NEAR, STALE, STALE_DEPTH};
        let n = word::BOX_VERTICES;
        let bits: Vec<u32> = (0..2 * n)
            .map(|k| corner(k % n, k >= n))
            .map(|c| {
                assert!(
                    c.iter().all(|&v| v == 0 || v == 1),
                    "a corner's a 0 or a 1 a lane"
                );
                c[0] | c[1] << 1 | c[2] << 2
            })
            .collect();
        let corners: Vec<String> = bits
            .chunks(10)
            .map(|w| {
                let word = w.iter().enumerate().fold(0, |a, (j, &b)| a | b << (3 * j));
                format!("{word}u")
            })
            .collect();
        let eyed_depth = 1.0f32 - 1.0 / (1u32 << 20) as f32;
        let raster = format!(
            "const RASTER_CORNERS = array<u32, {}>({});\nfn raster_corner(k: u32) -> vec3f {{\n  let b = (RASTER_CORNERS[k / 10u] >> (3u * (k % 10u))) & 7u;\n  return vec3f(f32(b & 1u), f32((b >> 1u) & 1u), f32(b >> 2u));\n}}\nconst RASTER_ID_MASK: u32 = {ID_MASK}u;\nconst RASTER_EYED: u32 = {EYED}u;\nconst RASTER_STALE: u32 = {STALE}u;\nconst RASTER_STALE_DEPTH: f32 = {STALE_DEPTH:?};\nconst RASTER_LACK: u32 = {LACK}u;\nconst RASTER_EYED_DEPTH: f32 = {eyed_depth:?};\nconst RASTER_NEAR: f32 = {NEAR:?};\n",
            corners.len(),
            corners.join(", "),
        );
        std::fs::write(out.join("raster_box.wgsl"), raster).expect("the raster's boxes' shader");
    }
    // Where each of the eye's words lies in the eye uniform the screen pass reads (pxl-engine
    // eye.rs), and how many vec4s the pass binds to reach the last of them.
    use pxl_engine::eye::{FAR, FORWARD, JITTER, ORIGIN, RIGHT, STEPS, TAN, UP};
    let last = (ORIGIN + 2)
        .max(STEPS)
        .max(FAR)
        .max(FORWARD + 2)
        .max(RIGHT + 2)
        .max(UP + 2)
        .max(TAN + 1)
        .max(JITTER + 1);
    let eye = format!(
        "const EYE_ORIGIN: u32 = {ORIGIN}u;\nconst EYE_STEPS: u32 = {STEPS}u;\nconst EYE_FAR: u32 = {FAR}u;\nconst EYE_FORWARD: u32 = {FORWARD}u;\nconst EYE_RIGHT: u32 = {RIGHT}u;\nconst EYE_UP: u32 = {UP}u;\nconst EYE_TAN: u32 = {TAN}u;\nconst EYE_JITTER: u32 = {JITTER}u;\nconst EYE_READ_VEC4S: u32 = {}u;\n",
        last / 4 + 1
    );
    std::fs::write(out.join("eye.wgsl"), eye).expect("the eye's shader");
    // A light slot's word is read as a depth, a share of the light box's depth, as pxl-engine
    // light.rs `word` lays it out, and every shader reads a depth through `light_depth`.
    use pxl_engine::light::{DEPTH_ONE, LAY_VERTICES};
    let light = format!(
        "fn light_depth(w: u32) -> f32 {{ return f32(w) / {DEPTH_ONE}.0; }}\nconst LAY_TILE: u32 = {LIGHT_TILE}u;\nconst LAY_VERTICES: u32 = {}u;\n",
        LAY_VERTICES
    );
    std::fs::write(out.join("light_word.wgsl"), light).expect("the light word's shader");
    std::fs::write(out.join("tile_word.wgsl"), tile_word()).expect("the tile word's shader");
    // The hash is the one the CPU's cumulus uses (pxl-content cloud.rs `lattice_hash`), written
    // from the same multipliers, rounds and output bits, so the clouds the GPU draws agree with
    // the CPU's.
    use pxl_content::cloud::{HASH_IN, HASH_LAST, HASH_ROUNDS, HASH_SHARE_BITS};
    use pxl_content::param::cloud::LATTICE_SHAMT;
    let [hx, hy, hs] = HASH_IN;
    let rounds: String = HASH_ROUNDS
        .iter()
        .map(|[shift, mul]| format!("h ^= h >> {shift}u; h *= {mul}u; "))
        .collect();
    let cloud = format!(
        "const CLOUD_LATTICE_CELLS: u32 = {}u;\nfn lattice_hash(i: vec2u, salt: u32) -> f32 {{ var h = (i.x * {hx}u) ^ (i.y * {hy}u) ^ (salt * {hs}u); {rounds}h ^= h >> {HASH_LAST}u; return f32(h >> {}u) / {}.0; }}\n",
        1u32 << LATTICE_SHAMT,
        32 - HASH_SHARE_BITS,
        1u32 << HASH_SHARE_BITS
    );
    std::fs::write(out.join("cloud_hash.wgsl"), cloud).expect("the cloud hash's shader");
    std::fs::write(out.join("debug_look.wgsl"), debug_looks()).expect("the debug views' looks");
    assemble(&out);
    println!("cargo:rerun-if-changed=src/frame/shader");
    println!("cargo:rerun-if-changed=src/frame/shader/word.rs");
}

/// The debug views' looks (debug_look.json) as WGSL, each decoded to linear light, as the
/// world's color and the canvas's blend it before it is encoded once (finish.wgsl), so each
/// shows as its sRGB; the ramp read straight between its stops.
fn debug_looks() -> String {
    use debug_look::{share, steps};
    // A world's cell spans (1, `SPANS`] of the view's cells where the rays stop on it (pxl-engine
    // grid.rs `cell_zoom_at`), so its share of a cell squared lies within (1, `SPANS`²]: the
    // tints' bounds are those.
    let spans = grid::SPANS;
    assert!(
        share::finer::most == 1.0 && share::within::most == spans * spans,
        "debug_look.json's share bounds are the split's"
    );
    let light = |c: u32| {
        [c >> 16, c >> 8, c].map(|v| {
            let v = (v & 255) as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                libm::powf((v + 0.055) / 1.055, 2.4)
            }
        })
    };
    let parts = |v: [f32; 3]| format!("{:?}, {:?}, {:?}", v[0], v[1], v[2]);
    let vec3 = |v: [f32; 3]| format!("vec3f({})", parts(v));
    let ramp: Vec<String> = steps::ramp.iter().map(|&c| vec3(light(c))).collect();
    let last = steps::ramp.len() - 1;
    format!(
        "const GRID_COLOR = vec4f({}, {:?});\nconst SHARE_FINER = {};\nconst SHARE_FINER_MOST: f32 = {:?};\nconst SHARE_WITHIN = {};\nconst SHARE_WITHIN_MOST: f32 = {:?};\nconst SHARE_COARSER = {};\nconst STEPS_MOST: f32 = {:?};\nconst STEPS_NONE = {};\nconst STEPS_RAMP = array<vec3f, {}>({});\nfn steps_ramp(x: f32) -> vec3f {{ let f = clamp(x, 0.0, 1.0) * {last}.0; let i = min(u32(f), {}u); return mix(STEPS_RAMP[i], STEPS_RAMP[i + 1u], f - f32(i)); }}\n",
        parts(light(debug_look::grid::rgb)),
        debug_look::grid::alpha,
        vec3(light(share::finer::rgb)),
        share::finer::most,
        vec3(light(share::within::rgb)),
        share::within::most,
        vec3(light(share::coarser::rgb)),
        steps::most,
        vec3(light(steps::none)),
        ramp.len(),
        ramp.join(", "),
        last - 1
    )
}

/// `text` without its line comments, its indentation and its blank lines. WGSL has no string
/// literals, so `//` always starts a comment. Lines are never joined, so two tokens cannot run
/// together. Block comments are kept.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let code = match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        };
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        out.push_str(code);
        out.push('\n');
    }
    out
}

// `light_item` (light_list.wgsl) reads a workgroup's lanes as a `SAMPLE_SIDE` by `SAMPLE_SIDE`
// square over one entry of the list. Each pass that calls it declares its workgroup with a side
// of its own: light_cell.wgsl with `SAMPLE_SIDE`, the light's sample box (sample.light.wgsl
// `sample_item`) with `SQUARE_SIDE`, and light.wgsl with its tile. The cast sizes each pass's
// dispatch by that pass's side (gpu/ray/cast.rs `light_args`), so each side must be SAMPLE_SIDE.
const _: () = assert!(
    LIGHT_TILE == SAMPLE_SIDE && word::SQUARE_SIDE == SAMPLE_SIDE,
    "light_item reads a workgroup as SAMPLE_SIDE x SAMPLE_SIDE lanes: a light tile and \
     SQUARE_SIDE are each that side"
);

/// The constants the walks read (walk.wgsl), each written from the pxl-engine layout or rule
/// that defines it.
fn walk_word() -> String {
    use pxl_engine::area::Shard;
    use pxl_engine::clipmap::{self, Lookup};
    use pxl_engine::object::{NodeSlot, RECTS};
    use pxl_engine::{
        answer, beam, cast, count, eye, face, light, node_cache, param, raster, ray, record, value,
        SHARD_ZOOM,
    };
    let words: &[(&str, u32)] = &[
        ("EYE_VEC4S", eye::WORDS as u32 / 4),
        ("ORIGIN", eye::ORIGIN as u32),
        ("STEPS", eye::STEPS as u32),
        ("FORWARD", eye::FORWARD as u32),
        ("FAR", eye::FAR as u32),
        ("RIGHT", eye::RIGHT as u32),
        ("UP", eye::UP as u32),
        ("TAN", eye::TAN as u32),
        ("RANGE", eye::RANGE as u32),
        ("GRID", eye::GRID as u32),
        ("FRAC_W", pxl_engine::FRAC_W),
        ("UNIT_ZOOM", pxl_engine::UNIT_ZOOM),
        ("FINEST_ZOOM", pxl_engine::FINEST_ZOOM),
        ("ZOOM_BITS", pxl_engine::ZOOM_BITS),
        ("SAMPLE", param::beam::SAMPLE),
        ("NEAR_SAMPLES", param::beam::NEAR),
        ("CELL_T", beam::CELL_T),
        ("CELL_CORNER", beam::CELL_CORNER),
        ("CELL_RESUMES", beam::CELL_RESUMES),
        ("CELL_WORDS", beam::CELL_WORDS),
        ("LINE_NODE", beam::LINE_NODE),
        ("LINE_STEPS", beam::LINE_STEPS),
        ("LINE_WORDS", beam::LINE_WORDS),
        ("NODE_ZOOM", beam::NODE_ZOOM),
        ("NODE_CORNER", beam::NODE_CORNER),
        ("NODE_ROOTS", beam::NODE_ROOTS),
        ("SAMPLE_NODE", beam::SAMPLE_NODE),
        ("SAMPLE_ASSET", beam::SAMPLE_ASSET),
        ("SAMPLE_PARKED", beam::SAMPLE_PARKED),
        ("SAMPLE_PARKED_T", beam::SAMPLE_PARKED_T),
        ("SAMPLE_PARKED_STATE", beam::SAMPLE_PARKED_STATE),
        ("SAMPLE_LACKS", beam::SAMPLE_LACKS),
        ("SAMPLE_AXIS", beam::SAMPLE_AXIS),
        ("SAMPLE_LACK", beam::SAMPLE_LACK),
        ("SAMPLE_STEPS_SHIFT", beam::SAMPLE_STEPS_SHIFT),
        ("NODE_WORDS", beam::NODE_WORDS),
        ("SAMPLE_WORDS", beam::SAMPLE_WORDS),
        ("SAMPLE_NONE", beam::SAMPLE_NONE),
        ("SAMPLE_SIDE", SAMPLE_SIDE),
        ("ENTRIES", node_cache::ENTRIES),
        ("ENTRIES_AT", node_cache::ENTRIES_AT),
        ("ENTRY_WORDS", node_cache::ENTRY_WORDS),
        ("BUCKET", node_cache::BUCKET),
        ("W_KIND", node_cache::W_KIND),
        ("KIND_MASK", node_cache::KIND_MASK),
        ("W_ASSET", node_cache::W_ASSET),
        ("W_NODE", node_cache::W_NODE),
        ("NONE", node_cache::NONE),
        ("CLAIMED", node_cache::CLAIMED),
        ("CLAIM_MASK", node_cache::CLAIM_MASK),
        ("CLAIM_FAR", node_cache::CLAIM_FAR),
        ("CELL_FAR", node_cache::CELL_FAR),
        ("TAG_BITS", node_cache::TAG_BITS),
        ("KEY_ZOOMS", !((1u32 << node_cache::TAG_BITS) - 1)),
        ("KIND_SPLIT", answer::Kind::Split as u32),
        ("KIND_CELL", answer::Kind::Cell as u32),
        ("KIND_LACKS", answer::Kind::Lacks as u32),
        ("VAL_MASK", answer::PLACED_ASSET - 1),
        ("VAL_VOID", value::VOID),
        ("VAL_AIR", value::AIR),
        ("FILL_SHIFT", answer::FILL_SHIFT),
        ("EIGHTHS", answer::EIGHTHS),
        ("BOX_BITS", answer::BOX_BITS),
        ("BOX_STEPS", answer::BOX_STEPS),
        ("STEPS_CAP", cast::STEPS_CAP),
        ("JITTER", eye::JITTER as u32),
        ("LIGHT", eye::LIGHT as u32),
        ("LIGHT_HEADER_VEC4S", light::HEADER_VEC4S as u32),
        ("LIGHT_HEADER_HOLES", light::HEADER_HOLES as u32),
        ("LIGHT_STARTS_AT", light::STARTS_AT as u32),
        ("LIGHT_ANCHOR", light::ANCHOR as u32),
        ("LIGHT_STEP_BITS", light::STEP_BITS),
        ("LIGHT_TILE", light::TILE),
        ("LIGHT_DEPTH_ONE", light::DEPTH_ONE),
        ("LIGHT_HEADER_SIZE", light::HEADER_SIZE as u32),
        ("MISSED_PASS", node_cache::MISSED_PASS),
        (
            "EYE_MISS_CNT",
            node_cache::lane_at(count::EYE, count::HOLE_WALK),
        ),
        ("HITS_ARGS", node_cache::HITS_ARGS),
        ("HITS_CNT", node_cache::HITS_CNT),
        (
            "LIGHT_ASKED_CNT",
            node_cache::lane_at(count::LIGHT, count::ASKS),
        ),
        ("LIGHT_T_MAX", light::W_T_MAX as u32),
        ("LIGHT_ACROSS", light::W_ACROSS as u32),
        ("LIGHT_DOWN", light::W_DOWN as u32),
        ("LIGHT_DIR", light::W_DIR as u32),
        ("LIGHT_BIAS", light::W_BIAS as u32),
        ("LIGHT_WIDTH", light::W_WIDTH as u32),
        ("LIGHT_INVERSE", light::W_INVERSE as u32),
        ("LIGHT_T_MAX_INVERSE", light::W_T_MAX_INVERSE as u32),
        ("LIGHT_ACROSS2_INVERSE", light::W_ACROSS2_INVERSE as u32),
        ("LIGHT_DOWN2_INVERSE", light::W_DOWN2_INVERSE as u32),
        ("LIGHT_BAND_SCALE", light::W_BAND_SCALE as u32),
        ("LIGHT_CORNER_X", light::CORNER[0] as u32),
        ("LIGHT_CORNER_Y", light::CORNER[1] as u32),
        ("LIGHT_HEADER_CAST", light::HEADER_CAST as u32),
        ("LIGHT_HEADER_EPOCH", light::HEADER_EPOCH as u32),
        ("LIGHT_HEADER_CELLS", light::HEADER_CELLS as u32),
        (
            "LIGHT_HEADER_CELLS_ACROSS",
            light::HEADER_CELLS_ACROSS as u32,
        ),
        ("LIGHT_HEADER_SAMPLES", light::HEADER_SAMPLES as u32),
        (
            "LIGHT_HEADER_SAMPLES_ACROSS",
            light::HEADER_SAMPLES_ACROSS as u32,
        ),
        ("LIGHT_HEADER_READS", light::HEADER_READS as u32),
        ("LIGHT_HEADER_TILES", light::HEADER_TILES as u32),
        ("LIGHT_HEADER_HANDS", light::HEADER_HANDS as u32),
        ("LIGHT_HEADER_CARRY", light::HEADER_CARRY as u32),
        ("LIGHT_HEADER_SLIDE", light::HEADER_SLIDE as u32),
        ("LIGHT_HEADER_WORLD", light::HEADER_WORLD as u32),
        ("LIGHT_HEADER_SLIDE_CAST", light::HEADER_SLIDE_CAST as u32),
        (
            "LIGHT_HEADER_WORLD_RECEIVERS",
            light::HEADER_WORLD_RECEIVERS as u32,
        ),
        ("LIGHT_HEADER_POOL", light::HEADER_POOL as u32),
        ("LIGHT_HEADER_HITS", light::HEADER_HITS as u32),
        ("LIGHT_SLIDE_RAN", light::SLIDE_RAN),
        ("LIGHT_READS_FROM", light::READS_FROM),
        ("LIGHT_HEADER_BEAM_ACROSS", light::HEADER_BEAM_ACROSS as u32),
        (
            "LIGHT_HEADER_CORNER_STATES",
            light::HEADER_CORNER_STATES as u32,
        ),
        (
            "LIGHT_HEADER_PARKED_ITEMS",
            light::HEADER_PARKED_ITEMS as u32,
        ),
        ("LIGHT_HEADER_LAST_RANGE", light::HEADER_LAST_RANGE as u32),
        ("LIGHT_HEADER_MARKS", light::HEADER_MARKS as u32),
        ("LIGHT_HEADER_LISTS", light::HEADER_LISTS as u32),
        ("LIGHT_LIST_ACROSS", light::LIST_ACROSS),
        ("LIGHT_LIST_LANES", light::LIST_LANES),
        ("LIGHT_BEAM", light::BEAM),
        ("LIGHT_SAMPLE", param::light::SAMPLE),
        ("LIGHT_LINE_WORDS", light::LINE_WORDS),
        ("LIGHT_LINE_ORIGIN", light::LINE_ORIGIN),
        ("LIGHT_BEAM_WINDOW", light::BEAM_WINDOW),
        ("LIGHT_LIST_CORNERS", light::LIST_CORNERS),
        ("LIGHT_LIST_SOURCES", light::LIST_SOURCES),
        ("LIGHT_LIST_DAMAGE", light::LIST_DAMAGE),
        ("LIGHT_LIST_RECEIVERS", light::LIST_RECEIVERS),
        ("LIGHT_LIST_READS", light::LIST_READS),
        ("LIGHT_HEADER_COUNTED", light::HEADER_COUNTED as u32),
        ("LIGHT_BEAM_PASS", node_cache::LIGHT_BEAM_PASS),
        (
            "LIGHT_BEAM_CNT",
            node_cache::lane_at(count::LIGHT, count::BEAM),
        ),
        ("VISIBLE_LACK", raster::LACK),
        ("CNT_RAYS", count::RAYS),
        ("CNT_VOXEL", count::VOXEL),
        ("CNT_SKY", count::SKY),
        ("CNT_HOLE", count::HOLE),
        ("SAMPLES_PASS", node_cache::SAMPLES_PASS),
        ("LIGHT_SAMPLES_PASS", node_cache::LIGHT_SAMPLES_PASS),
        (
            "EYE_SAMPLE_CNT",
            node_cache::lane_at(count::EYE, count::SAMPLE),
        ),
        (
            "EYE_ASKED_CNT",
            node_cache::lane_at(count::EYE, count::ASKS),
        ),
        (
            "LIGHT_SAMPLE_CNT",
            node_cache::lane_at(count::LIGHT, count::SAMPLE),
        ),
        ("GRID_KEEP", beam::GRID_KEEP as u32),
        ("GRID_RADIUS", beam::GRID_RADIUS as u32),
        ("GRID_RADIUS_TAN", beam::GRID_RADIUS_TAN as u32),
        ("GRID_PAST", beam::GRID_PAST as u32),
        ("GRID_SPAN", beam::GRID_SPAN as u32),
        ("GRID_CORNERS", beam::GRID_CORNERS as u32),
        ("GRID_SAMPLES", beam::GRID_SAMPLES as u32),
        ("GRID_CELLS_AT", beam::GRID_CELLS_AT as u32),
        ("GRID_SAMPLES_AT", beam::GRID_SAMPLES_AT as u32),
        ("GRID_HITS_AT", beam::GRID_HITS_AT as u32),
        ("GRID_ASKS_AT", beam::GRID_ASKS_AT as u32),
        ("GRID_HIT_ASKS_AT", beam::GRID_HIT_ASKS_AT as u32),
        ("GRID_WALKED_AT", beam::GRID_WALKED_AT as u32),
        ("GRID_CORNER_STATES_AT", beam::GRID_CORNER_STATES_AT as u32),
        ("GRID_HANDS_AT", beam::GRID_HANDS_AT as u32),
        ("GRID_SLOPE", beam::GRID_SLOPE as u32),
        ("STARTS_ACROSS", light::STARTS_ACROSS as u32),
        ("STARTS_DOWN", light::STARTS_DOWN as u32),
        ("STARTS_ANCHORS", light::STARTS_ANCHORS as u32),
        ("STARTS_FRACS", light::STARTS_FRACS as u32),
        (
            "LIGHT_MISS_CNT",
            node_cache::lane_at(count::LIGHT, count::HOLE_WALK),
        ),
        ("CARVE", eye::CARVE as u32),
        ("HIDE", eye::HIDE as u32),
        ("FRAME", node_cache::FRAME),
        ("PASSES", node_cache::PASSES),
        ("CNT_ON", node_cache::CNT_ON),
        ("CNT_EYE", count::EYE),
        ("CNT_WALKS", count::WALKS),
        ("Y_START", cast::Y_START),
        ("Z_START", cast::Z_START),
        ("SHARD_ZOOM", SHARD_ZOOM),
        ("SHARD_UNKNOWN", Shard::UNKNOWN.word()),
        ("SHARD_INSIDE", Shard::INSIDE.word()),
        ("LOOKUP_NONE", Lookup::NONE.word()),
        ("SLOT_NOTHING", NodeSlot::NOTHING.word()),
        ("LINE_INHERITANCE", beam::LINE_INHERITANCE),
        ("LINE_STEP", beam::LINE_STEP),
        ("W_INHERITANCE", node_cache::W_INHERITANCE),
        ("MISS_ACROSS", ray::MISS_ACROSS),
        ("MISS_LANES", ray::MISS_LANES),
        ("SQUARE_WORDS", ray::SQUARE_WORDS),
        ("POOL_CAP", ray::POOL_CAP),
        ("PARK_HOLE", ray::PARK_HOLE),
        ("PARK_WORDS", ray::PARK_WORDS),
        ("HITS_A_FRAME", ray::HITS_A_FRAME),
        ("HIT_LANES", beam::HIT_LANES),
        ("HIT_WORDS", beam::HIT_WORDS),
        ("HIT_NODE", beam::HIT_NODE),
        ("HIT_FACE", beam::HIT_FACE),
        ("HIT_PX", beam::HIT_PX),
        ("HIT_WAITS", beam::HIT_WAITS),
        ("HIT_LACKS", beam::HIT_LACKS),
        ("QUEUE_LANES", ray::QUEUE_LANES),
        ("QUEUE_GROUPS", ray::QUEUE_GROUPS),
        ("QUEUE_STEPS", ray::QUEUE_STEPS),
        ("QUEUE_NEXT", ray::QUEUE_NEXT),
        ("QUEUE_NEXT_PARKED", ray::QUEUE_NEXT_PARKED),
        ("QUEUE_ASKS", ray::QUEUE_ASKS),
        ("QUEUE_ASKS_PARKED", ray::QUEUE_ASKS_PARKED),
        ("QUEUE_HITS", ray::QUEUE_HITS),
        ("QUEUE_PARKED", ray::QUEUE_PARKED),
        (
            "EYE_CORNERS_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_EYE_CORNERS),
        ),
        (
            "EYE_SAMPLES_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_EYE_SAMPLES),
        ),
        (
            "EYE_HOLES_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_EYE_HOLES),
        ),
        (
            "LIGHT_CORNERS_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_LIGHT_CORNERS),
        ),
        (
            "LIGHT_SAMPLES_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_LIGHT_SAMPLES),
        ),
        (
            "LIGHT_HOLES_RECORDS_AT",
            ray::queue_records_at(ray::QUEUE_LIGHT_HOLES),
        ),
        ("QUEUE_RECORD_WORDS", ray::QUEUE_RECORD_WORDS),
        ("QUEUE_RECORDS_CAP", ray::QUEUE_RECORDS_CAP),
        ("STATE_WORDS", ray::STATE_WORDS),
        ("PARKED", ray::PARKED),
        ("LAST_NEXT", node_cache::LAST_NEXT),
        ("LAST_CNT", node_cache::LAST_CNT),
        ("ASKED", node_cache::ASKED),
        ("ASKS_A_FRAME", ray::ASKS_A_FRAME),
        ("HIT_ASSET", beam::HIT_ASSET),
        ("HIT_LINE", beam::HIT_LINE),
        ("FACE_BOX", face::BOX),
        ("RAY_FACE_SHIFT", ray::FACE_SHIFT),
        ("RAY_ZOOM_SHIFT", ray::ZOOM_SHIFT),
        ("RAY_FILL_SHIFT", ray::FILL_SHIFT),
        ("CAST_SIDE", ray::CAST_SIDE),
        ("SCREEN_TILE", screen::TILE),
        ("RASTER_ASKED_AT", raster::ASKED_AT),
        ("RASTER_HIT_ASKED_AT", raster::HIT_ASKED_AT),
        ("BEAM_PASS", node_cache::BEAM_PASS),
        ("EYE_BEAM_CNT", node_cache::lane_at(count::EYE, count::BEAM)),
        ("RASTER_ARGS_INSTANCES", raster::ARGS_INSTANCES),
        ("RASTER_ARGS_ASKS", raster::ARGS_ASKS),
        ("RASTER_ARGS_CNT", raster::ARGS_CNT),
        ("RASTER_FACES_LANES", raster::FACES_LANES),
        ("LIGHT_HEADER_LAST_CORNER", light::HEADER_LAST_CORNER as u32),
        ("LIGHT_HEADER_FOOTPRINT", light::HEADER_FOOTPRINT as u32),
        ("LIGHT_RECEIVER_BOXES_AT", light::RECEIVER_BOXES_AT as u32),
        ("LIGHT_RECEIVER_BOXES", light::RECEIVER_BOXES as u32),
        ("LIGHT_COLUMNS_AT", light::COLUMNS_AT as u32),
        ("LIGHT_COLUMNS", light::COLUMNS as u32),
        ("LIGHT_REACH_NDC", light::REACH_NDC as u32),
        ("LIGHT_ARGS_CELLS", light::ARGS_CELLS),
        ("LIGHT_ARGS_SAMPLE_BOX", light::ARGS_SAMPLE_BOX),
        ("LIGHT_ARGS_SLOT", light::ARGS_SLOT),
        ("LIGHT_RECEIVER_TILES_CNT", light::RECEIVER_TILES_CNT),
        ("LIGHT_ARGS_LAY", light::ARGS_LAY),
        ("LIGHT_ARGS_PASSES", light::ARGS_PASSES),
        ("LIGHT_LISTS", light::LISTS),
        ("ASK_NODE", beam::ASK_NODE),
        ("ASK_FACE", beam::ASK_FACE),
        ("ASK_LINE", beam::ASK_LINE),
        ("ASK_TO", beam::ASK_TO),
        ("ASK_WORDS", beam::ASK_WORDS),
        ("ASK_NONE", beam::ASK_NONE),
        ("FACE_WORD_BITS", face::BITS),
        ("FACE_EDGES", face::EDGES),
        ("FACE_RISE_SHIFT", face::RISE_SHIFT),
        ("RASTER_ARGS_FACES", raster::ARGS_FACES),
        ("NODE_CACHE_FORGETS", node_cache::FORGETS),
        ("FORGETS_ALL", node_cache::FORGETS_ALL),
        ("NODE_CACHE_CHANGED_AT", node_cache::CHANGED_AT),
        (
            "NODE_CACHE_CHANGED_BRICKS_AT",
            node_cache::CHANGED_BRICKS_AT,
        ),
        ("NODE_CACHE_RECTS_AT", node_cache::RECTS_AT),
        ("NODE_CACHE_WRITTEN_AT", node_cache::WRITTEN_AT),
        ("NODE_CACHE_WRITTEN_CAP", node_cache::WRITTEN_CAP),
        ("NODE_CACHE_RECTS", RECTS),
        ("KEYS_AT", node_cache::KEYS_AT),
        ("STATES_AT", node_cache::STATES_AT),
        ("RECORDS_AT", node_cache::RECORDS_AT),
        ("R_WORDS", node_cache::R_WORDS),
        ("R_DRAWN", node_cache::R_DRAWN),
        ("R_FACES", node_cache::R_FACES),
        ("W_PLACED", node_cache::W_PLACED),
        ("EYE_CLIP_LEVELS", clipmap::EYE_CLIP_LEVELS),
        ("NODE_CACHE_DRAWN", node_cache::DRAWN),
        ("RECORD_EYE_DRAWS", record::EYE_DRAWS),
        ("RECORD_LIGHT_DRAWS", record::LIGHT_DRAWS),
        ("RECORD_DRAW_LANES", record::DRAW_LANES),
        ("RECORD_FACES", record::FACES),
        ("RECORD_ASKED", record::ASKED),
        ("RECORD_ASKED_SHIFT", record::ASKED_SHIFT),
        ("RECORD_WHOLE", record::WHOLE),
        ("RECORD_TO", record::TO_RECORD),
    ];
    // The light's cell rays start a margin short of where its beam ray stopped: a share of the
    // beam's spacing plus a number of Q3 steps (`light::margin`). A receiver reads slots within
    // a disk grown by its taps' reach. A tile is taken to hold such a slot when the tile's middle
    // lies within that disk grown by half the tile's diagonal (`light::TILE_READ_PAD`,
    // light_propagate.wgsl).
    let tile_pad = light::TILE_READ_PAD;
    let mut out = format!(
        "const INV_SPLIT2: f32 = {:?};\nconst NARROW: f32 = {:?};\nconst NARROW_SQUARED: f32 = {:?};\nconst LIGHT_MARGIN_PER_WIDTH: f32 = {:?};\nconst LIGHT_MARGIN_STEPS: f32 = {:?};\nconst LIGHT_TILE_READ_PAD: f32 = {tile_pad:?};\nconst LIGHT_FOOTPRINT_TILES: i32 = {}i;\nconst LIGHT_READ_FIRST: i32 = {}i;\nconst LIGHT_READ_LAST: i32 = {}i;\nconst FRAC_1_SQRT_2: f32 = {:?};\nconst SAMPLE_SPACING: f32 = {:?};\nconst SAMPLE_SPACING_INVERSE: f32 = {:?};\nconst GRID_CELL_INVERSE: f32 = {:?};\nconst RECORD_VIEW_GROWTH: f32 = {:?};\n",
        grid::INV_SPLIT2,
        cast::NARROW,
        cast::NARROW_SQUARED,
        2.0 * cast::SQRT_6,
        light::margin(0.0),
        light::FOOTPRINT_TILES,
        light::READ_TILES[0],
        light::READ_TILES[1],
        core::f32::consts::FRAC_1_SQRT_2,
        beam::SAMPLE_SPACING,
        beam::SAMPLE_SPACING_INVERSE,
        grid::CELL_INVERSE,
        record::VIEW_GROWTH,
    );
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    // Where a node cache slot's words lie. Every pass over the node cache addresses a slot
    // through these, and each matches the pxl-engine `node_cache` function of the same name.
    out += concat!(
        "fn entry_at(slot: u32) -> u32 { return ENTRIES_AT + slot * ENTRY_WORDS; }\n",
        "fn key_at(slot: u32) -> u32 { return KEYS_AT + slot; }\n",
        "fn state_at(slot: u32) -> u32 { return STATES_AT + slot; }\n",
        "fn drawn_at(slot: u32) -> u32 { return RECORDS_AT + slot * R_WORDS + R_DRAWN; }\n",
        "fn face_at(slot: u32, face: u32) -> u32 { return RECORDS_AT + slot * R_WORDS + R_FACES + face; }\n",
    );
    out
}

fn side_word() -> String {
    let cases: String = (1..answer::EIGHTHS)
        .map(|k| format!("case {k}u: {{ return {:?}; }} ", answer::side(k)))
        .collect();
    format!(
        "fn side(k: u32) -> f32 {{ switch (k) {{ {cases}default: {{ return {:?}; }} }} }}\n",
        answer::side(answer::EIGHTHS)
    )
}

/// The constants the world's query reads (world.wgsl), each written from the pxl-engine or
/// pxl-content layout or rule that defines it.
fn world_word() -> String {
    use pxl_content::cave::{
        BLOCK, BLOCKS, CELL_BLOCKS, CROSS, PIPE, ROOT, ROOT_BLOCKS, SPAWN, SPLIT, WALK,
    };
    use pxl_content::cube::ground as palette;
    use pxl_content::param::{cave, strata};
    use pxl_engine::clipmap as clip;
    use pxl_engine::object::{self, Cut};
    use pxl_engine::way::tree;
    use pxl_engine::{area, seed, value};
    use xpute_kit::math::rng;
    let words: &[(&str, u32)] = &[
        ("LEAF_CELL_ZOOM", pxl_engine::LEAF_CELL_ZOOM),
        ("MIN_ZOOM", pxl_engine::MIN_ZOOM),
        ("VAL_UNKNOWN", value::UNKNOWN),
        ("GROUNDS_FIRST", value::GROUNDS[0]),
        ("GROUNDS_LAST", value::GROUNDS[1]),
        ("OBJECTS_FIRST", value::OBJECTS[0]),
        ("OBJECTS_LAST", value::OBJECTS[1]),
        ("PLACED_ASSET", answer::PLACED_ASSET),
        ("FULL", answer::FULL),
        ("FULL_BITS", answer::FULL.trailing_zeros()),
        ("FILL_BITS", answer::FILL_BITS),
        ("BRANCH", object::BRANCH),
        ("OBJECT_ROOT", object::ROOT),
        ("CHILDREN", object::CHILDREN),
        ("SLOT_FOLDED", object::FOLDED),
        ("SLOT_ZOOM_MASK", object::ZOOM_MASK),
        ("SLOT_SHIFT", object::SLOT_SHIFT),
        ("POOL_CHILD_GROUPS", tree::POOL_CHILD_GROUPS),
        ("ROOT_ID_BITS", tree::ROOT_ID_BITS),
        ("ROOT_NODES", tree::ROOT_NODES),
        ("SHARD_KNOWN", area::SHARD_KNOWN),
        ("SHARD_SERVED_SHIFT", area::SHARD_SERVED_SHIFT),
        ("SHARD_OWN", area::OWN),
        ("BRICK", clip::BRICK),
        ("BRICK_SHIFT", clip::BRICK_SHIFT),
        ("BRICK_CELLS", clip::BRICK_CELLS),
        ("BRICK_WORDS", clip::BRICK_WORDS),
        ("BRICK_MIPS", clip::BRICK_MIPS),
        ("ACROSS", clip::ACROSS),
        ("CLIP_ENTRIES", clip::ENTRIES),
        ("FIRST_CELL_ZOOM", clip::FIRST_CELL_ZOOM),
        ("ORIGIN_AT", clip::ORIGIN_AT),
        ("JOINED_AT", clip::JOINED_AT),
        ("JOINED_ENTRIES", clip::JOINED_ENTRIES),
        ("JOINED_MIPS", clip::JOINED_MIPS),
        ("COLUMNS_AT", clip::COLUMNS_AT),
        ("BOUNDS_AT", clip::BOUNDS_AT),
        ("BOUND_ENTRY_WORDS", clip::BOUND_ENTRY_WORDS),
        ("OWN_AT", clip::OWN_AT),
        ("OWN_CELLS", clip::OWN_CELLS),
        ("ROOT_AT", clip::ROOT_AT),
        ("LOOKUP_SLOT_BITS", clip::LOOKUP_SLOT_BITS),
        ("LOOKUP_FROM_SHIFT", clip::LOOKUP_FROM_SHIFT),
        ("LOOKUP_CELL_ZOOM_SHIFT", clip::LOOKUP_CELL_ZOOM_SHIFT),
        ("LOOKUP_AT", (1 << clip::LOOKUP_CELL_ZOOM_SHIFT) - 1),
        ("STRATA_SAND", palette::sand as u32),
        ("STRATA_BEACH", palette::beach as u32),
        ("STRATA_WATER", palette::water as u32),
        ("STRATA_SOIL", palette::soil as u32),
        ("STRATA_SANDSTONE", palette::sandstone as u32),
        ("STRATA_STONE", palette::stone as u32),
        ("CAVE_ROOT", ROOT),
        ("CAVE_PIPE", PIPE),
        ("CAVE_BLOCK", BLOCK),
        ("CELL_BLOCKS", CELL_BLOCKS),
        ("ROOT_BLOCKS", ROOT_BLOCKS),
        ("CAVE_NUDGE", cave::NUDGE),
        ("CAVE_TAG_SPAWN", SPAWN),
        ("CAVE_TAG_WALK", WALK),
        ("CAVE_TAG_CROSS", CROSS),
        ("CAVE_TAG_SPLIT", SPLIT),
        ("CAVE_TAG_BLOCKS", BLOCKS),
        ("MASTER_SEED", seed::MASTER_SEED),
        ("SEED_SALT", rng::SEED_SALT_32),
        ("MIX_FIRST", rng::MIX32_FIRST),
        ("MIX_LAST", rng::MIX32_LAST),
        ("TUNNELS_LAST", cave::ROOT_TUNNELS[4]),
    ];
    let signed: &[(&str, i32)] = &[
        ("SEA_LEVEL", pxl_engine::SEA_LEVEL),
        ("SEA_CELLS", ground::SEA_CELLS),
        ("EMPTY_LO", ground::EMPTY[0]),
        ("EMPTY_HI", ground::EMPTY[1]),
        ("UNBOUNDED_LO", ground::UNBOUNDED[0]),
        ("UNBOUNDED_HI", ground::UNBOUNDED[1]),
        ("STRATA_FILLER_DEPTH", strata::FILLER_DEPTH),
        ("STRATA_SANDSTONE_DEPTH", strata::SANDSTONE_DEPTH),
        ("STRATA_BEDROCK_DEPTH", strata::BEDROCK_DEPTH),
    ];
    let mut out = String::new();
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    for (name, v) in signed {
        out += &format!("const {name}: i32 = {v}i;\n");
    }
    for (name, [a, b]) in [
        ("BOX_NONE", answer::BOX_NONE),
        ("BOX_WHOLE", answer::BOX_WHOLE),
    ] {
        out += &format!("const {name} = vec2<u32>({a}u, {b}u);\n");
    }
    let [a, b, c, d, _] = cave::ROOT_TUNNELS;
    out += &format!("const TUNNELS_FIRST = vec4<u32>({a}u, {b}u, {c}u, {d}u);\n");
    let none = Cut::NONE;
    out += &format!(
        "const NO_CUT = vec4<i32>({}i, {}i, {}i, {}i);\n",
        none.at[0], none.at[1], none.at[2], none.zoom as i32
    );
    let cases: String = cave::BLOCKS_A_TUNNEL[..4]
        .iter()
        .enumerate()
        .map(|(k, b)| format!("case {k}u: {{ return {b}u; }} "))
        .collect();
    out += &format!(
        "fn tunnel_blocks(k: u32) -> u32 {{ switch k {{ {cases}default: {{ return {}u; }} }} }}\n",
        cave::BLOCKS_A_TUNNEL[4]
    );
    let cases: String = (0..=clip::BRICK_MIPS)
        .map(|j| format!("case {j}u: {{ return {}u; }} ", clip::bound_mip_at(j)))
        .collect();
    out + &format!(
        "fn bound_mip_at(j: u32) -> u32 {{ switch j {{ {cases}default: {{ return {}u; }} }} }}\n",
        clip::bound_mip_at(clip::BRICK_MIPS + 1)
    )
}

/// The constants decor's passes read beyond the world's words, each written from the
/// pxl-content layout or rule that defines it.
fn decor_word() -> String {
    use core::mem::{offset_of, size_of};
    use pxl_content::decor::{Decor, Orientation, DECORS};
    use pxl_content::generation::{self as g, drawn};
    use pxl_content::param::scatter::ALIGN_NEIGHBOR_RADIUS;
    use pxl_content::scatter;
    use pxl_engine::{cast, value, world};
    let row = |at: usize| (at / size_of::<u32>()) as u32;
    let words: &[(&str, u32)] = &[
        ("LANES", g::LANES),
        ("RECORD_WORDS", g::RECORD_WORDS),
        ("HEADER_WORDS", g::HEADER_WORDS),
        ("CNT_AT", g::CNT_AT),
        ("SLOTS_AT", g::SLOTS_AT),
        ("BRICK_RECORDS", g::BRICK_RECORDS),
        ("UNGENERATED", g::UNGENERATED),
        ("TURN_SHIFT", g::TURN_SHIFT),
        ("VARIANT_SHIFT", g::VARIANT_SHIFT),
        ("TURN_BITS", g::TURN_BITS),
        ("CELL_ZOOM_SHIFT", g::CELL_ZOOM_SHIFT),
        ("THIN_ARGS", g::THIN_ARGS),
        ("THIN_CNT", g::THIN_CNT),
        ("GENERATE_CNT", g::GENERATE_CNT),
        ("LIST_RECTS", g::LIST_RECTS),
        ("RECTS_AT", g::RECTS_AT),
        ("RECT_WORDS", g::RECT_WORDS),
        ("RECT_CAP", g::RECT_CAP),
        ("EVERY", g::EVERY),
        ("LIST_EVERY", g::LIST_EVERY),
        ("LIST_PARTS", g::LIST_PARTS),
        ("GENERATE_ARGS", g::GENERATE_ARGS),
        ("SLOT_RECORDS", g::SLOT_RECORDS),
        ("PLACED_HEADER", g::PLACED_HEADER),
        ("PLACED_HEADER_WORDS", g::PLACED_HEADER_WORDS),
        ("RUN", g::RUN),
        ("RUNS", g::RUNS),
        ("EYE_ENTRIES", g::EYE_ENTRIES),
        ("ROW_CHANCE", row(offset_of!(Decor, chance))),
        ("DRAWN_NONE", drawn::NONE),
        ("DRAWN_REGION_SHIFT", drawn::REGION_SHIFT),
        ("DRAWN_COLUMNS", drawn::COLUMNS),
        ("DRAWN_MODEL_ANGLE", drawn::MODEL_ANGLE),
        ("DRAWN_IMPOSTOR_ANGLE", drawn::IMPOSTOR_ANGLE),
        ("DRAWN_ANCHOR", drawn::ANCHOR),
        ("DRAWN_HIDE", drawn::HIDE),
        ("DRAWN_EXTENTS", drawn::EXTENTS),
        ("DRAWN_MODEL", drawn::MODEL),
        ("DRAWN_IMPOSTOR", drawn::IMPOSTOR),
        ("DRAWN_CULLED", drawn::CULLED),
        ("LIST_AT", g::LIST_AT),
        ("LIST_WORDS", g::LIST_WORDS),
        ("PAIR_DECOR_SHIFT", g::PAIR_DECOR_SHIFT),
        ("PLACED_TAG", g::PLACED_TAG),
        ("PLACED_FIRST", world::PLACED_FIRST),
        ("CNT_TAG", scatter::CNT),
        ("SPLIT_TAG", scatter::SPLIT),
        ("DECORS", DECORS.len() as u32),
        ("FIRST_VAL", DECORS[0].val),
        ("ROW_WORDS", row(size_of::<Decor>())),
        ("ROW_SEED", row(offset_of!(Decor, seed))),
        ("ROW_ORIENT", row(offset_of!(Decor, orient))),
        ("ROW_ON", row(offset_of!(Decor, on))),
        ("ROW_VARIANTS", row(offset_of!(Decor, variants))),
        ("ROW_SIZES", row(offset_of!(Decor, sizes))),
        ("ALONG_TRANSPORT", Orientation::along_transport as u32),
        ("TRANSPORTS_FIRST", value::TRANSPORTS[0]),
        ("TRANSPORTS_LAST", value::TRANSPORTS[1]),
        ("REGIONS", drawn::REGIONS),
        ("ARGS_WORDS", drawn::ARGS_WORDS),
        ("IMPOSTOR_CORNERS", drawn::IMPOSTOR_CORNERS),
        ("UNIFORM_WORDS", drawn::UNIFORM_WORDS),
        ("UNIFORM_IDX", word::instance::IDX as u32),
        ("TALLEST", g::TALLEST as u32),
        ("WIDEST", g::WIDEST as u32),
    ];
    const {
        assert!(
            g::TALLEST > 0 && g::WIDEST > 0,
            "decor's tallest and widest are a cell or more"
        );
        assert!(
            pxl_engine::UNIT_ZOOM - world::PLACED_FIRST == 3,
            "decor_pose.wgsl `overlaps_placed` descends three levels to the unit cells"
        );
    }
    let mut out = String::new();
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    let signed: &[(&str, i32)] = &[
        ("READ_REACH", g::READ_REACH),
        ("ALIGN_NEIGHBOR_RADIUS", ALIGN_NEIGHBOR_RADIUS),
    ];
    for (name, v) in signed {
        out += &format!("const {name}: i32 = {v}i;\n");
    }
    let [ex, ey, ez] = g::CLAIM_EXTENT;
    out += &format!("const CLAIM_EXTENT = vec3<i32>({ex}i, {ey}i, {ez}i);\n");
    let by_decor = |name: &str, of: &dyn Fn(usize) -> u32| {
        let last = DECORS.len() - 1;
        let cases: String = (0..last)
            .map(|k| format!("case {k}u: {{ return {}u; }} ", of(k)))
            .collect();
        format!(
            "fn {name}(k: u32) -> u32 {{ switch (k) {{ {cases}default: {{ return {}u; }} }} }}\n",
            of(last)
        )
    };
    let by_decor_i32 = |name: &str, of: &dyn Fn(usize) -> i32| {
        let last = DECORS.len() - 1;
        let cases: String = (0..last)
            .map(|k| format!("case {k}u: {{ return {}i; }} ", of(k)))
            .collect();
        format!(
            "fn {name}(k: u32) -> i32 {{ switch (k) {{ {cases}default: {{ return {}i; }} }} }}\n",
            of(last)
        )
    };
    out += &by_decor("decor_cap", &|k| DECORS[k].cap());
    out += &by_decor("decor_first_record", &|k| {
        g::first_record(&DECORS, k as u32)
    });
    out += &by_decor("decor_first_group", &|k| g::first_group(&DECORS, k as u32));
    out += &by_decor("decor_root", &|k| scatter::ROOTS[k]);
    out += &by_decor_i32("decor_widest", &|k| DECORS[k].widest() as i32);
    // Each turn's spin is written as the bits of the float the Rust computes, so
    // decor_compact.wgsl writes the same spin the CPU does.
    let last = (1u32 << g::TURN_BITS) - 1;
    let spin = |t: u32| (t as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    let spins: String = (0..last)
        .map(|t| format!("case {t}u: {{ return {}u; }} ", spin(t)))
        .collect();
    out += &format!(
        "fn turn_spin(turn: u32) -> u32 {{ switch (turn) {{ {spins}default: {{ return {}u; }} }} }}\n",
        spin(last)
    );
    out += &format!("const NEAREST_U: f32 = {:?};\n", drawn::NEAREST_U);
    // How far below its box's top a cell keeping `k` eighths leaves its own top, as a share of
    // the cell's side (`cast::top`, `generation::footing`). Each value is written as the bits of
    // the Rust's float, so the shader reads exactly what the CPU computes.
    let top_of = |k: u32| {
        let asset = (k % answer::EIGHTHS) << answer::FILL_SHIFT;
        cast::fill_bounds(([0.0; 3], [1.0; 3]), asset).1[2]
    };
    let cases: String = (1..answer::EIGHTHS)
        .map(|k| {
            format!(
                "case {k}u: {{ return bitcast<f32>({}u); }} ",
                (1.0 - top_of(k)).to_bits()
            )
        })
        .collect();
    out + &format!(
        "fn sink_share(k: u32) -> f32 {{ switch (k) {{ {cases}default: {{ return bitcast<f32>({}u); }} }} }}\n",
        (1.0 - top_of(answer::EIGHTHS)).to_bits()
    )
}

/// The tiles' uniform as every pass reads it (pxl-engine `screen::TILES_WORDS`, gpu/tile.rs
/// `TileGrid::words`), and the reading of a tile's word that word.rs's `screen::age` and
/// `screen::in_damage` define. A tile's age is the number of frames since it changed, and it
/// wraps. A tile is in damage while its age is below the uniform's `damage_frames`. Every pass
/// that reads the tiles (screen_tile.wgsl, screen_propagate.wgsl, cast_tile.wgsl) uses this
/// one rule.
fn tile_word() -> String {
    // The Rust's rule must be the one the WGSL below spells: the age is a wrapping u32
    // difference, and damage ends at `damage_frames`.
    assert_eq!(screen::age(0, u32::MAX), 1);
    assert_eq!(screen::age(5, 5), 0);
    assert!(screen::in_damage(0, 1) && !screen::in_damage(1, 1));
    assert!(screen::in_damage(u32::MAX - 1, u32::MAX) && !screen::in_damage(u32::MAX, u32::MAX));
    format!(
        concat!(
            "const CAST_TILES_LANES: u32 = {}u;\n",
            "struct TileGrid {{ cnt: vec4u, size: vec4f, }};\n",
            "fn tiles_age(t: TileGrid, changed_at: u32) -> u32 {{ return t.cnt.w - changed_at; }}\n",
            "fn tiles_in_damage(t: TileGrid, changed_at: u32) -> bool {{ return tiles_age(t, changed_at) < t.cnt.z; }}\n",
        ),
        screen::CAST_TILES_LANES
    )
}

/// Every setting in src/param.rs as a WGSL constant, named by its path under `param`: `LIMB`
/// in `sky::sun` becomes `SKY_SUN_LIMB`. An `F64` setting is the host's alone and is not
/// written.
fn params() -> String {
    use param::Value;
    let mut out = String::new();
    for &(path, name, val) in param::ALL.iter().flat_map(|m| m.iter()) {
        let topic = path.rsplit_once("param::").map_or("", |(_, t)| t);
        let named = format!("{}_{name}", topic.replace("::", "_").to_uppercase());
        out += &match val {
            Value::F32(v) => format!("const {named}: f32 = {v:?};\n"),
            Value::I32(v) => format!("const {named}: i32 = {v};\n"),
            Value::U32(v) => format!("const {named}: u32 = {v}u;\n"),
            Value::Vec2([x, y]) => format!("const {named}: vec2f = vec2f({x:?}, {y:?});\n"),
            Value::Vec3([x, y, z]) => {
                format!("const {named}: vec3f = vec3f({x:?}, {y:?}, {z:?});\n")
            }
            Value::F64(_) => continue,
        };
    }
    out
}

/// The values the host lays out and every shader reads (frame/shader/word.rs), written as WGSL:
/// each uniform's size in vec4s, and the pxl-engine values any shader may name.
fn words() -> String {
    use word::*;
    let vec4s = |name: &str, floats: usize| {
        assert!(floats.is_multiple_of(4), "{name} is whole vec4s");
        format!("const {name}_VEC4S: u32 = {}u;\n", floats / 4)
    };
    [
        vec4s("DRAW", DRAW_FLOATS),
        // A Q3 step in units. It is a power of two, so scaling a record's position by it is
        // exact (frame.wgsl `from_eye`).
        format!(
            "const Q3_STEP: f32 = {:?};\n",
            1.0f32 / (1u32 << pxl_engine::FRAC_W) as f32
        ),
        vec4s("FRAME", FRAME_FLOATS),
        vec4s("SKY", SKY_FLOATS),
        vec4s("ATMOS", ATMOS_FLOATS),
        vec4s("CLOUDS", CLOUDS_FLOATS),
        vec4s("STARS", STARS_FLOATS),
        vec4s("AO", AO_FLOATS),
        vec4s("MARKER", MARKER_FLOATS),
        vec4s("STAR_RECORD", STAR_RECORD_FLOATS),
        format!("const MARKER_INSTANCE_WORDS: u32 = {MARKER_INSTANCE_WORDS}u;\nconst LOCATION_ZONE_INSTANCE_FLOATS: u32 = {LOCATION_ZONE_INSTANCE_FLOATS}u;\n"),
        format!("const AO_DOWNSCALE: u32 = {AO_DOWNSCALE}u;\nconst AO_NOISE_SIDE: u32 = {AO_NOISE_SIDE}u;\nconst DECOR_GROUPS: u32 = {DECOR_GROUPS}u;\n"),
        // The side of a square of the depth bound, and the squares on a side of a screen tile;
        // the bound's passes take a tile as one workgroup (screen_bound.wgsl,
        // screen_propagate.wgsl). Then the number of frames the jitter sequence takes to cover a
        // pixel, over which each still frame adds an even share (pxl-engine `screen::JITTERS`,
        // screen_resolve.wgsl).
        format!(
            "const BOUND_STEP: u32 = {BOUND_STEP}u;\nconst BOUND_SQUARES: u32 = {BOUND_SQUARES}u;\nconst TILE_JITTERS: u32 = {}u;\n",
            screen::JITTERS
        ),
        [
            ("TRANSMITTANCE_SIZE", TRANSMITTANCE_SIZE),
            ("MULTISCATTER_SIZE", MULTISCATTER_SIZE),
            ("SKY_VIEW_SIZE", SKY_VIEW_SIZE),
            ("IRRADIANCE_SIZE", IRRADIANCE_SIZE),
        ]
        .map(|(name, (w, h))| format!("const {name}: vec2f = vec2f({w}.0, {h}.0);\n"))
        .concat(),
        format!("const ATLAS_FILL: u32 = {ATLAS_FILL}u;\nconst ATLAS_STROKE: u32 = {ATLAS_STROKE}u;\nconst ATLAS_TRIANGLE: u32 = {ATLAS_TRIANGLE}u;\nconst ATLAS_TEXT: u32 = {ATLAS_TEXT}u;\nconst ATLAS_HALO: u32 = {ATLAS_HALO}u;\n"),
        format!("const BOX_XY_STEPS: u32 = {BOX_XY_STEPS}u;\nconst BOX_VERTICES: u32 = {BOX_VERTICES}u;\n"),
        format!("const MOON_TEXELS: u32 = {MOON_TEXELS}u;\nconst NO_LAYER: f32 = {NO_LAYER}.0;\nconst HALF_STEP: f32 = {HALF_STEP:?};\n"),
        format!("const MOON_ANGULAR_RADIUS: f32 = {MOON_ANGULAR_RADIUS:?};\nconst MOON_ALBEDO: f32 = {MOON_ALBEDO:?};\nconst MOON_MAP_MEAN: f32 = {MOON_MAP_MEAN:?};\n"),
        format!("const SLOT_BITS: u32 = {SLOT_BITS}u;\nconst NO_SLOT: u32 = {NO_SLOT}u;\n"),
        format!("const IMAGE_TOP: u32 = {IMAGE_TOP}u;\nconst IMAGE_SIDE: u32 = {IMAGE_SIDE}u;\nconst IMAGE_BOTTOM: u32 = {IMAGE_BOTTOM}u;\n"),
        format!("const EARTH_TILE_BITS: u32 = {EARTH_TILE_BITS}u;\n"),
        format!("const WORKGROUPS_CAP: u32 = {WORKGROUPS_CAP}u;\n"),
        format!("const AIR_FROXELS: u32 = {AIR_FROXELS}u;\n"),
        format!("const SQUARE_SIDE: u32 = {SQUARE_SIDE}u;\nconst ROW_LANES: u32 = {ROW_LANES}u;\n"),
        format!("const CELL_SIDE: u32 = {CELL_SIDE}u;\n"),
        format!("const LIGHT_SLOPES: u32 = {LIGHT_SLOPES}u;\nconst GROUND_SLOPES: u32 = {GROUND_SLOPES}u;\nconst SLOPE_WORDS: u32 = {SLOPE_WORDS}u;\n"),
        format!("const CLOUD_TILE_UNITS: u32 = {CLOUD_TILE_UNITS}u;\nconst CLOUD_TILE_TEXELS: u32 = {CLOUD_TILE_TEXELS}u;\n"),
        // The bits of a Q3 Z, which spans the world's height about sea level.
        format!("const HEIGHT_BITS: u32 = {}u;\n", pxl_engine::HEIGHT_BITS),
        format!(
            "const NIGHT_FALL: vec2f = vec2f({:?}, {:?});\n",
            night::FALL[0],
            night::FALL[1]
        ),
        // The camera grid's cell is the footprint a drawn cell covers per unit of distance, seen
        // square on. Every effect shares the one reach written beside it.
        format!("const GRID_CELL: f32 = {:?};\nconst EFFECT_REACH: f32 = {:?};\n", grid::CELL, EFFECT_REACH as f32),
        // How far apart a receiver's taps lie in the shadow map. The map's read pad is sized by
        // it (pxl-engine param.rs `light::SPREAD`, light.rs `READ_PAD`).
        format!("const LIGHT_SPREAD: f32 = {:?};\n", light::SPREAD),
    ]
    .concat()
}

/// An accessor for each field of every uniform `text` reads (frame/shader/word.rs `UNIFORMS`),
/// which reads the field off the vec4s where the host wrote it. A uniform is never declared as
/// a struct, as one crashed the GPU process of Safari's WebGPU. Each field is checked to lie
/// within its vec4s and apart from the others.
fn accessors(text: &str) -> String {
    use word::{floats, Kind, UNIFORMS};
    let mut out = String::new();
    for (uniform, fields) in UNIFORMS {
        let words = format!("{uniform}_words");
        // A shader reads a uniform from its own `_words` binding, or, where the uniform lies in
        // a list, through its `_word` function. A shader that does neither gets no accessors.
        let row_of: fn(&str, &str, usize) -> String =
            if text.contains(&format!("var<uniform> {words}:")) {
                |words, _, row| format!("{words}[{row}]")
            } else if text.contains(&format!("fn {uniform}_word(")) {
                |_, uniform, row| format!("{uniform}_word({row}u)")
            } else {
                continue;
            };
        let at = |row: usize| row_of(&words, uniform, row);
        let mut taken = vec![false; floats(fields)];
        for f in fields {
            let (row, col, n) = (f.at / 4, f.at % 4, f.kind.floats());
            for t in &mut taken[f.at..f.at + n] {
                assert!(!*t, "{uniform}'s {} lies over another field", f.name);
                *t = true;
            }
            let lanes = &"xyzw"[col..(col + n).min(4)];
            let column = |k: usize| format!("{}.xyz", at(row + k));
            let vec4 = |k: usize| at(row + k);
            let lanes = format!("{}.{lanes}", at(row));
            let (ty, read) = match f.kind {
                Kind::F32 => ("f32", lanes),
                Kind::U32 => ("u32", format!("bitcast<u32>({lanes})")),
                Kind::Vec2 => ("vec2f", lanes),
                Kind::Vec2u => ("vec2u", format!("bitcast<vec2u>({lanes})")),
                Kind::Vec3 => ("vec3f", lanes),
                Kind::Vec3i => ("vec3i", format!("bitcast<vec3i>({lanes})")),
                Kind::Vec4i => ("vec4i", format!("bitcast<vec4i>({})", vec4(0))),
                Kind::Vec4u => ("vec4u", format!("bitcast<vec4u>({})", vec4(0))),
                Kind::Mat3 => (
                    "mat3x3f",
                    format!("mat3x3f({}, {}, {})", column(0), column(1), column(2)),
                ),
                Kind::Mat4 => (
                    "mat4x4f",
                    format!(
                        "mat4x4f({}, {}, {}, {})",
                        vec4(0),
                        vec4(1),
                        vec4(2),
                        vec4(3)
                    ),
                ),
            };
            let whole = matches!(f.kind, Kind::Vec4i | Kind::Vec4u | Kind::Mat3 | Kind::Mat4);
            assert!(
                if whole { col == 0 } else { col + n <= 4 },
                "{uniform}'s {} crosses a vec4",
                f.name
            );
            out += &format!(
                "fn {uniform}_{}() -> {ty} {{ return {read}; }}\n",
                f.name.to_lowercase()
            );
        }
    }
    out
}

/// `text` with only the functions its entry points reach. `text` is read as `strip_comments`
/// leaves it, one statement a line. A function runs from its `fn` line to the line where its
/// braces close. An entry point is a function whose attributes stand on its own line or on the
/// line before.
fn read_fns(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let lines: Vec<&str> = text.lines().collect();
    // Each function's name, with its first and last line.
    let mut fns = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    let mut entries = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        // Where `fn` begins on a function's first line, after its attributes when they stand
        // on that line.
        let at = if lines[k].starts_with("fn ") {
            Some(0)
        } else if lines[k].starts_with('@') {
            lines[k].find(" fn ").map(|p| p + 1)
        } else {
            None
        };
        let Some(rest) = at.map(|p| &lines[k][p + 3..]) else {
            k += 1;
            continue;
        };
        let name = &rest[..rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())];
        let (mut depth, mut end, mut opened) = (0i32, k, false);
        for (j, l) in lines.iter().enumerate().skip(k) {
            for c in l.chars() {
                match c {
                    '{' => (depth, opened) = (depth + 1, true),
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if opened && depth == 0 {
                end = j;
                break;
            }
        }
        let attrs = lines[k].starts_with('@') || (k > 0 && lines[k - 1].starts_with('@'));
        let entry = [&lines[k], if k > 0 { &lines[k - 1] } else { &"" }]
            .iter()
            .any(|l| l.contains("@compute") || l.contains("@vertex") || l.contains("@fragment"));
        if attrs && entry {
            entries.push(name);
        }
        fns.insert(name, (k, end));
        k = end + 1;
    }
    let mut reached = std::collections::BTreeSet::new();
    while let Some(f) = entries.pop() {
        if !reached.insert(f) {
            continue;
        }
        let (first, last) = fns[f];
        for l in &lines[first..=last] {
            for w in l.split(|c: char| !is_word(c)) {
                if fns.contains_key(w) && !reached.contains(w) {
                    entries.push(w);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut k = 0;
    while k < lines.len() {
        let dropped = fns
            .iter()
            .find(|(name, (first, _))| *first == k && !reached.contains(*name));
        match dropped {
            Some((_, &(_, last))) => k = last + 1,
            None => {
                out.push(lines[k]);
                k += 1;
            }
        }
    }
    out.join("\n") + "\n"
}

/// `text` with only the structs something in it names. `text` is read as `strip_comments`
/// leaves it: a struct runs from its `struct` line to the line where its brace closes.
fn read_structs(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut lines: Vec<&str> = text.lines().collect();
    loop {
        // Each struct's name, with its first and last line.
        let mut structs = Vec::new();
        let mut k = 0;
        while k < lines.len() {
            let Some(rest) = lines[k].strip_prefix("struct ") else {
                k += 1;
                continue;
            };
            let name = &rest[..rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())];
            // The brace closes on the struct's first line or at the start of a later line.
            let end = (k..lines.len())
                .find(|&j| j == k && lines[j].contains('}') || lines[j].starts_with('}'))
                .expect("a struct's close");
            structs.push((name, k, end));
            k = end + 1;
        }
        let unread = structs.iter().find(|(name, first, last)| {
            !lines.iter().enumerate().any(|(j, l)| {
                (j < *first || j > *last) && l.split(|c: char| !is_word(c)).any(|w| w == *name)
            })
        });
        let Some(&(_, first, last)) = unread else {
            return lines.join("\n") + "\n";
        };
        lines.drain(first..=last);
    }
}

/// `text` with only the module's constants something in it reads.
fn read_consts(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let name_of = |line: &str| {
        let rest = line.strip_prefix("const ")?;
        rest.find(|c: char| !is_word(c))
            .map(|end| rest[..end].to_string())
    };
    let mut lines: Vec<&str> = text.lines().collect();
    loop {
        let mut names = std::collections::BTreeMap::<&str, u32>::new();
        for w in lines.iter().flat_map(|l| l.split(|c: char| !is_word(c))) {
            *names.entry(w).or_default() += 1;
        }
        let (mut read, mut unread) = (Vec::with_capacity(lines.len()), false);
        let mut through = false;
        for l in &lines {
            if through {
                through = !l.contains(';');
            } else if name_of(l).is_some_and(|name| names[name.as_str()] == 1) {
                through = !l.contains(';');
                unread = true;
            } else {
                read.push(*l);
            }
        }
        lines = read;
        if !unread {
            return lines.join("\n") + "\n";
        }
    }
}

/// Refuses, in every assembled module, compute and render, what the Galaxy S23's driver or
/// Safari fails to compile. The S23's Vulkan driver refuses a module whose struct holds a struct,
/// directly or in an array, and one where a function's local is a struct or is or holds an array.
/// The one struct local allowed is a vertex or fragment entry point's local of the type it
/// returns. Safari writes `i32::MIN`, written or folded, as a C++ `long` that no `select` with an
/// `int` takes. Safari 27 writes the result of `atomicCompareExchangeWeak` as a struct with a
/// `thread`-qualified field, which Metal refuses.
fn refuse_faults(module: &naga::Module, name: &str) {
    use naga::TypeInner::{Array, BindingArray, Struct};
    let is_struct = |mut ty: naga::Handle<naga::Type>| loop {
        match module.types[ty].inner {
            Struct { .. } => return true,
            Array { base, .. } | BindingArray { base, .. } => ty = base,
            _ => return false,
        }
    };
    fn holds_array(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
        match &module.types[ty].inner {
            Array { .. } | BindingArray { .. } => true,
            Struct { members, .. } => members.iter().any(|m| holds_array(module, m.ty)),
            _ => false,
        }
    }
    let mut faults = Vec::new();
    for (_, ty) in module.types.iter() {
        let Struct { members, .. } = &ty.inner else {
            continue;
        };
        for m in members {
            if is_struct(m.ty) {
                faults.push(format!(
                    "struct {} holds a struct in {}",
                    ty.name.as_deref().unwrap_or("?"),
                    m.name.as_deref().unwrap_or("?")
                ));
            }
        }
    }
    let entries = module.entry_points.iter().map(|e| {
        let output = matches!(
            e.stage,
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment
        )
        .then(|| e.function.result.as_ref().map(|r| r.ty))
        .flatten();
        (&e.function, output)
    });
    let fns = module.functions.iter().map(|(_, f)| (f, None));
    for (f, output) in fns.chain(entries) {
        for (_, var) in f.local_variables.iter() {
            let ty = &module.types[var.ty];
            let local = || {
                format!(
                    "{}'s var {}: {}",
                    f.name.as_deref().unwrap_or("?"),
                    var.name.as_deref().unwrap_or("?"),
                    ty.name.as_deref().unwrap_or("array")
                )
            };
            if matches!(ty.inner, Struct { .. }) && output != Some(var.ty) {
                faults.push(format!("{} is a struct", local()));
            }
            if holds_array(module, var.ty) {
                faults.push(format!("{} holds an array", local()));
            }
        }
    }
    let i32_min =
        |e: &naga::Expression| matches!(e, naga::Expression::Literal(naga::Literal::I32(i32::MIN)));
    let functions = module.functions.iter().map(|(_, f)| f);
    let min_cnt = module
        .global_expressions
        .iter()
        .filter(|(_, e)| i32_min(e))
        .count()
        + functions
            .chain(module.entry_points.iter().map(|e| &e.function))
            .map(|f| f.expressions.iter().filter(|(_, e)| i32_min(e)).count())
            .sum::<usize>();
    if min_cnt > 0 {
        faults.push(format!("i32::MIN held {min_cnt} times"));
    }
    let compared = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .chain(module.entry_points.iter().map(|e| &e.function))
        .flat_map(|f| f.expressions.iter())
        .filter(|(_, e)| {
            matches!(
                e,
                naga::Expression::AtomicResult {
                    comparison: true,
                    ..
                }
            )
        })
        .count();
    if compared > 0 {
        faults.push(format!("atomicCompareExchangeWeak held {compared} times"));
    }
    assert!(faults.is_empty(), "{name}: {}", faults.join("; "));
}

/// Assembles every shader from its parts (part.rs `PARTS`), validates it as WebGPU takes it,
/// with no capability, and writes it where record.rs `source` includes it. For each shader it
/// also writes which stages read each binding (`STAGES`, in OUT_DIR), from which every
/// pipeline's groups take their visibility (record.rs `pipeline`), and what each compute entry
/// point does with each binding (`ACCESS`).
fn assemble(out: &Path) {
    use naga::valid::{Capabilities, GlobalUse, ValidationFlags, Validator};
    let read = |name: &str| {
        std::fs::read_to_string(out.join(format!("{name}.wgsl")))
            .unwrap_or_else(|e| panic!("{name}.wgsl: {e}"))
    };
    let constant = read("constant");
    let (mut sources, mut stages, mut access) = (String::new(), String::new(), String::new());
    let mut rastered = false;
    for (kind, parts) in part::PARTS.iter().enumerate() {
        let mut text = constant.clone() + "\n";
        for part in parts.iter() {
            text += &read(part);
            text += "\n";
        }
        text = read_consts(&read_structs(&read_fns(&text)));
        let module = wgsl::parse_str(&text)
            .unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        let raster = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Raster"));
        if let Some((_, ty)) = raster {
            rastered = true;
            let naga::TypeInner::Struct { span, .. } = ty.inner else {
                panic!("raster.wgsl's `Raster` is a struct")
            };
            assert_eq!(
                span as usize,
                4 * word::RASTER_FLOATS,
                "raster.wgsl's `Raster` against `RASTER_FLOATS`"
            );
        }
        // Each member of the lists' argument structs must begin at the word pxl-engine lays it
        // at (visible.wgsl `MissCount`, light_receiver_tile.wgsl `ReceiverDispatch`).
        {
            use pxl_engine::{light, ray};
            let words: [(&str, &[u32]); 2] = [
                ("MissCount", &[ray::MISS_ARGS_CNT, ray::MISS_ARGS_CNTS]),
                (
                    "ReceiverDispatch",
                    &[
                        light::RECEIVER_TILES_LIST,
                        light::RECEIVER_TILES_CNT,
                        light::RECEIVER_TILES_SLIDE,
                    ],
                ),
            ];
            for (name, want) in words {
                let Some((_, ty)) = module
                    .types
                    .iter()
                    .find(|(_, t)| t.name.as_deref() == Some(name))
                else {
                    continue;
                };
                let naga::TypeInner::Struct { ref members, span } = ty.inner else {
                    panic!("`{name}` is a struct")
                };
                let got: Vec<u32> = members.iter().map(|m| m.offset / 4).collect();
                assert_eq!(
                    &got[..want.len()],
                    want,
                    "`{name}`'s words against pxl-engine's"
                );
                if name == "MissCount" {
                    assert_eq!(
                        span as usize,
                        4 * ray::MISS_ARGS_WORDS,
                        "`MissCount` against `ray::MISS_ARGS_WORDS`"
                    );
                }
                if name == "ReceiverDispatch" {
                    assert_eq!(
                        span,
                        4 * light::RECEIVER_TILES_WORDS,
                        "`ReceiverDispatch` against `light::RECEIVER_TILES_WORDS`"
                    );
                }
            }
        }
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        refuse_faults(&module, &format!("shader {kind} ({parts:?})"));
        let file = format!("{kind}.wgsl");
        std::fs::write(out.join("assembled").join(&file), &text).expect("an assembled shader");
        sources += &format!(
            "        {kind} => include_str!(concat!(env!(\"OUT_DIR\"), \"/shader/assembled/{file}\")),\n"
        );
        // For each compute entry point, the bindings it uses, and whether it reads each, writes
        // it, and writes it through atomics alone (frame/shader/access.rs `READ`, `WRITE`,
        // `ATOMIC`).
        let mut accessed = Vec::new();
        for (i, entry) in module.entry_points.iter().enumerate() {
            if entry.stage != naga::ShaderStage::Compute {
                continue;
            }
            let mut by: Vec<[u32; 3]> = Vec::new();
            for (handle, var) in module.global_variables.iter() {
                let Some(b) = var.binding else { continue };
                let used = info.get_entry_point(i)[handle];
                if used.is_empty() {
                    continue;
                }
                let read = used.intersects(GlobalUse::READ | GlobalUse::QUERY);
                let write = used.contains(GlobalUse::WRITE);
                // A binding is written through atomics alone when it holds atomics and no plain
                // store the entry point reaches writes it. A store into a plain member of a
                // struct that also holds atomics is a plain store.
                let atomic = holds_atomic(&module, var.ty)
                    && !stores_plainly(&module, &entry.function, handle, &mut Vec::new());
                let bits = u32::from(read) | u32::from(write) << 1 | u32::from(atomic) << 2;
                match by.iter_mut().find(|a| a[0] == b.group && a[1] == b.binding) {
                    Some(a) => a[2] |= bits,
                    None => by.push([b.group, b.binding, bits]),
                }
            }
            by.sort();
            let by: Vec<String> = by
                .iter()
                .map(|[g, b, a]| format!("[{g}, {b}, {a}]"))
                .collect();
            accessed.push(format!("(\"{}\", &[{}])", entry.name, by.join(", ")));
        }
        access += &format!("    &[{}],\n", accessed.join(", "));
        // For each binding, its group and number, the stages whose entry points read it, the
        // layout kinds that take it as it is declared, and the vertex entries that read it.
        let mut read_by: Vec<(u32, u32, Vec<&str>, String, u32)> = Vec::new();
        for (handle, var) in module.global_variables.iter() {
            let Some(b) = var.binding else { continue };
            let takes = layout_kinds(&module, var).unwrap_or_else(|| {
                panic!(
                    "shader {kind} ({parts:?}): binding {}:{} has no layout kind",
                    b.group, b.binding
                )
            });
            let (mut by, mut vertex) = (Vec::new(), 0u32);
            for (i, entry) in module.entry_points.iter().enumerate() {
                let stage = match entry.stage {
                    naga::ShaderStage::Vertex => "VERTEX",
                    naga::ShaderStage::Fragment => "FRAGMENT",
                    _ => "COMPUTE",
                };
                if info.get_entry_point(i)[handle].is_empty() {
                    continue;
                }
                if !by.contains(&stage) {
                    by.push(stage);
                }
                if entry.stage == naga::ShaderStage::Vertex {
                    let k = part::VERTEX_ENTRY
                        .iter()
                        .position(|&n| n == entry.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "shader {kind}: vertex entry {} not in VERTEX_ENTRY",
                                entry.name
                            )
                        });
                    vertex |= 1 << k;
                }
            }
            // Two parts may declare the same binding when only one of them reads it. The entry
            // kept is the one that is read.
            match read_by
                .iter_mut()
                .find(|r| (r.0, r.1) == (b.group, b.binding))
            {
                Some(_) if by.is_empty() => {}
                Some(r) if r.2.is_empty() => *r = (b.group, b.binding, by, takes, vertex),
                Some(_) => panic!(
                    "shader {kind} ({parts:?}): binding {}:{} read twice",
                    b.group, b.binding
                ),
                None => read_by.push((b.group, b.binding, by, takes, vertex)),
            }
        }
        read_by.sort();
        let entries: Vec<String> = read_by
            .iter()
            .map(|(g, b, by, takes, vertex)| {
                format!(
                    "[{g}, {b}, {}, {takes}, {vertex}]",
                    if by.is_empty() {
                        "NONE".to_string()
                    } else {
                        by.join(" | ")
                    }
                )
            })
            .collect();
        stages += &format!("    &[{}],\n", entries.join(", "));
    }
    assert!(rastered, "no shader holds raster.wgsl's `Raster`");
    // The world's query alone, assembled as a module that no pipeline runs. tests/world.rs
    // asks it word for word against pxl-engine, and the shader test compiles it with Tint.
    let asked = [
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "answer",
        "world",
        "world_split",
        "world_asked",
    ]
    .map(read)
    .join("\n");
    let asked = read_consts(&read_structs(&read_fns(&asked)));
    let module = wgsl::parse_str(&asked)
        .unwrap_or_else(|e| panic!("world_asked: {}", e.emit_to_string(&asked)));
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|e| panic!("world_asked: {}", e.emit_to_string(&asked)));
    refuse_faults(&module, "world_asked");
    std::fs::write(out.join("assembled").join("world_asked.wgsl"), asked)
        .expect("the world's asked shader");
    let root = out.parent().expect("OUT_DIR");
    std::fs::write(
        root.join("sources.rs"),
        format!("/// Shader `kind` as build.rs assembled and validated it (part.rs).\npub fn src(kind: u32) -> &'static str {{\n    match kind {{\n{sources}        _ => \"\",\n    }}\n}}\n"),
    )
    .expect("the shaders' sources");
    std::fs::write(
        root.join("stages.rs"),
        format!("/// Each shader's bindings, [group, binding, the stages that read it, a bit a layout kind\n/// that takes it as declared, a bit a vertex entry that reads it (`VERTEX_ENTRY`)], read off it\n/// by build.rs.\npub const STAGES: [&[[u32; 5]]; SHADERS as usize] = [\n{stages}];\n/// Each shader's compute entry points, each with the bindings it uses, [group, binding, its\n/// access (access.rs `READ`, `WRITE`, `ATOMIC`)], read off it by build.rs.\npub const ACCESS: [&[EntryAccess]; SHADERS as usize] = [\n{access}];\n/// A compute entry point's name and its bindings' access, as `ACCESS` holds them.\npub type EntryAccess = (&'static str, &'static [[u32; 3]]);\n{BINDS}"),
    )
    .expect("the shaders' stages");
}

/// The source of `binds`, written into stages.rs beside `STAGES`. `binds` holds when a compute
/// group's (binding, stages, kind) words lay out exactly what the shaders of the pipelines over
/// it read. Every binding such a shader reads must be in group 0, have an entry in `group` that
/// the compute stage sees, and have a kind its declaration takes (its number, whether a buffer
/// is read-only, a texture's kind). Every entry of `group` must be read by one of the shaders.
/// Each owner of a compute group asserts it.
const BINDS: &str = "
/// Whether `group`, a compute group's (binding, stages, kind) words, lays out exactly what
/// the shaders of compute pipelines `pipelines` (`compute`) read, each binding as its
/// declaration takes it (build.rs `BINDS`).
pub const fn binds(pipelines: &[u32], group: &[u32]) -> bool {
    let mut s = 0;
    while s < pipelines.len() {
        let entries = STAGES[compute(pipelines[s]).0 as usize];
        let mut k = 0;
        while k < entries.len() {
            let [g, b, by, takes, _] = entries[k];
            if by != NONE {
                let mut found = false;
                let mut e = 0;
                while e < group.len() {
                    found |= group[e] == b
                        && group[e + 1] & COMPUTE != 0
                        && takes & 1 << group[e + 2] != 0;
                    e += 3;
                }
                if g != 0 || !found {
                    return false;
                }
            }
            k += 1;
        }
        s += 1;
    }
    let mut e = 0;
    while e < group.len() {
        let mut read = false;
        let mut s = 0;
        while s < pipelines.len() {
            read |= stages_of(compute(pipelines[s]).0, 0, group[e]) != NONE;
            s += 1;
        }
        if !read {
            return false;
        }
        e += 3;
    }
    true
}
";

/// Whether `function`, or a function it calls, makes a plain store into `global`: a `Store` of
/// a non-atomic value into the global, or into a member or element of it. `seen` holds the
/// functions already walked.
fn stores_plainly(
    module: &naga::Module,
    function: &naga::Function,
    global: naga::Handle<naga::GlobalVariable>,
    seen: &mut Vec<naga::Handle<naga::Function>>,
) -> bool {
    use naga::{Expression, Statement};
    /// The global a pointer is into and the type it points at, where the pointer is to a
    /// global or to a member or an element of one.
    fn pointee(
        module: &naga::Module,
        f: &naga::Function,
        e: naga::Handle<Expression>,
    ) -> Option<(naga::Handle<naga::GlobalVariable>, naga::Handle<naga::Type>)> {
        use naga::TypeInner;
        let element = |ty: naga::Handle<naga::Type>, idx: Option<u32>| match &module.types[ty].inner
        {
            TypeInner::Array { base, .. } => Some(*base),
            TypeInner::Struct { members, .. } => idx.map(|i| members[i as usize].ty),
            _ => None,
        };
        match f.expressions[e] {
            Expression::GlobalVariable(g) => Some((g, module.global_variables[g].ty)),
            Expression::AccessIndex { base, index } => {
                let (g, ty) = pointee(module, f, base)?;
                Some((g, element(ty, Some(index))?))
            }
            Expression::Access { base, .. } => {
                let (g, ty) = pointee(module, f, base)?;
                Some((g, element(ty, None)?))
            }
            _ => None,
        }
    }
    fn block(
        module: &naga::Module,
        f: &naga::Function,
        b: &naga::Block,
        global: naga::Handle<naga::GlobalVariable>,
        seen: &mut Vec<naga::Handle<naga::Function>>,
    ) -> bool {
        b.iter().any(|st| match st {
            // naga holds an `atomicStore` as a `Store` to an atomic, so a plain store is one to
            // anything else.
            Statement::Store { pointer, .. } => {
                pointee(module, f, *pointer).is_some_and(|(g, ty)| {
                    g == global && !matches!(module.types[ty].inner, naga::TypeInner::Atomic(_))
                })
            }
            Statement::Block(inner) => block(module, f, inner, global, seen),
            Statement::If { accept, reject, .. } => {
                block(module, f, accept, global, seen) || block(module, f, reject, global, seen)
            }
            Statement::Switch { cases, .. } => cases
                .iter()
                .any(|c| block(module, f, &c.body, global, seen)),
            Statement::Loop {
                body, continuing, ..
            } => block(module, f, body, global, seen) || block(module, f, continuing, global, seen),
            Statement::Call { function, .. } => {
                if seen.contains(function) {
                    return false;
                }
                seen.push(*function);
                let called = &module.functions[*function];
                block(module, called, &called.body, global, seen)
            }
            _ => false,
        })
    }
    block(module, function, &function.body, global, seen)
}

fn holds_atomic(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
    match &module.types[ty].inner {
        naga::TypeInner::Atomic(_) => true,
        naga::TypeInner::Array { base, .. } => holds_atomic(module, *base),
        naga::TypeInner::Struct { members, .. } => {
            members.iter().any(|m| holds_atomic(module, m.ty))
        }
        _ => false,
    }
}

/// The layout kinds (api.json's record words) that can bind `var` as it is declared, written as
/// an expression of bits over record.rs's names. None when no record word lays out its kind.
fn layout_kinds(module: &naga::Module, var: &naga::GlobalVariable) -> Option<String> {
    use naga::{
        AddressSpace, ImageClass, ImageDimension as D, ScalarKind, StorageAccess, StorageFormat,
        TypeInner,
    };
    let kinds: &[&str] = match (var.space, &module.types[var.ty].inner) {
        (AddressSpace::Uniform, _) => &["UNIFORM", "UNIFORM_DYNAMIC"],
        (AddressSpace::Storage { access }, _) if access.contains(StorageAccess::STORE) => {
            &["STORAGE_RW"]
        }
        (AddressSpace::Storage { .. }, _) => &["STORAGE"],
        (_, TypeInner::Sampler { comparison: true }) => &["SAMPLER_COMPARISON"],
        (_, TypeInner::Sampler { comparison: false }) => &["SAMPLER"],
        (
            _,
            &TypeInner::Image {
                dim,
                arrayed,
                ref class,
            },
        ) => match (dim, arrayed, class) {
            (
                D::D2,
                false,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE", "TEXTURE_UNFILTERED"],
            (
                D::D2,
                true,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE_ARRAY"],
            (
                D::D2,
                true,
                ImageClass::Sampled {
                    kind: ScalarKind::Uint,
                    multi: false,
                },
            ) => &["TEXTURE_UINT_ARRAY"],
            (
                D::D3,
                false,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE_3D"],
            (D::D2, false, ImageClass::Depth { multi: false }) => &["DEPTH"],
            (
                D::D2,
                true,
                ImageClass::Storage {
                    format: StorageFormat::R32Uint,
                    access,
                },
            ) if access.contains(StorageAccess::LOAD | StorageAccess::STORE) => {
                &["STORAGE_TEXTURE_RW"]
            }
            (
                D::D3,
                false,
                ImageClass::Storage {
                    format: StorageFormat::Rgba16Float,
                    access,
                },
            ) if *access == StorageAccess::STORE => &["STORAGE_TEXTURE_3D"],
            (
                D::D2,
                false,
                ImageClass::Storage {
                    format: StorageFormat::Rgba16Float,
                    access,
                },
            ) if *access == StorageAccess::STORE => &["STORAGE_TEXTURE_2D"],
            _ => return None,
        },
        _ => return None,
    };
    Some(
        kinds
            .iter()
            .map(|k| format!("1 << {k}"))
            .collect::<Vec<_>>()
            .join(" | "),
    )
}
