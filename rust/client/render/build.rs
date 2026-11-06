// pxl-render/build.rs

// The shipped WGSL is the shader files with comments and indentation stripped; nothing is renamed.

use naga::front::wgsl;
use pxl_engine::answer;
use pxl_engine::beam::{CORNER_SIDE, SAMPLE_SIDE};
use pxl_engine::grid;
use pxl_engine::ground;
use pxl_engine::lattice;
use pxl_engine::light::TILE as LIGHT_TILE;
use pxl_engine::param::light;
use pxl_engine::screen;
use std::path::Path;

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
mod word;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
    // Written afresh each run: a part no longer in src/frame/shader is not
    // found in an older run's copy.
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
    // A unit's zoom, a unit's fraction bits in Q3 and the ground's cell, from
    // pxl-engine, where the cast reads them: a ray is painted by its
    // cell's zoom against these, its cell placed off the eye's Q3 steps.
    let lattice = format!(
        "const UNIT_ZOOM: u32 = {}u;\nconst FRAC_W: u32 = {}u;\nconst GROUND_CELL: u32 = {}u;\n",
        pxl_engine::UNIT_ZOOM,
        pxl_engine::FRAC_W,
        lattice::GROUND.cell()
    );
    std::fs::write(out.join("lattice.wgsl"), lattice).expect("the lattice's shader");
    std::fs::write(out.join("walk_word.wgsl"), walk_word()).expect("the walk's words");
    std::fs::write(out.join("side.wgsl"), side_word()).expect("a box's side");
    std::fs::write(out.join("world_word.wgsl"), world_word()).expect("the world's words");
    std::fs::write(out.join("way_word.wgsl"), way_word()).expect("the ways' words");
    std::fs::write(out.join("decor_word.wgsl"), decor_word()).expect("decor's words");
    std::fs::write(out.join("fill_word.wgsl"), fill_word()).expect("the fill's words");
    // A ray's first word, its fields as the cast writes them (pxl-engine
    // ray.rs), and each field read as the cast reads it.
    use pxl_engine::ray::*;
    let field = |name: &str, shift: u32, bits: u32| {
        format!(
            "fn ray_{name}(w: u32) -> u32 {{ return (w >> {shift}u) & {}u; }}\n",
            (1u32 << bits) - 1
        )
    };
    // Its word of what lies about its face, and that word's fields, a bit an
    // edge or a corner (pxl-engine face.rs).
    use pxl_engine::face::{CORNER_SHIFT, LINE_SHIFT, RISE_SHIFT};
    let faces = format!(
        "const RAY_FACES: u32 = {FACES}u;\nconst FACES_LINE: u32 = {LINE_SHIFT}u;\nconst FACES_RISE: u32 = {RISE_SHIFT}u;\nconst FACES_CORNER: u32 = {CORNER_SHIFT}u;\n"
    );
    // The side of the box a cell filled short of its cube is, by the eighths
    // it keeps, 0 for all eight (pxl-engine answer.rs `side`).
    let sides: String = (1..answer::EIGHTHS)
        .map(|k| format!("case {k}u: {{ return {:?}; }} ", answer::side(k)))
        .collect();
    let fill = format!(
        "fn fill_side(k: u32) -> f32 {{ switch k {{ {sides}default: {{ return 1.0; }} }} }}\n"
    );
    // A coarse cell's four palette indices, its ray's third word, as a cell's word
    // holds them (pxl-engine ground.rs `palette_indices`).
    let b = ground::PALETTE_BITS;
    let m = (1u32 << b) - 1;
    let palette_indices = format!(
        "fn ray_palette_indices(w: u32) -> vec4u {{ return vec4u(w & {m}u, (w >> {b}u) & {m}u, (w >> {}u) & {m}u, w >> {}u); }}\n",
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
        palette_indices,
    ];
    std::fs::write(out.join("ray.wgsl"), ray.concat()).expect("the ray's shader");
    // How the samples' boxes lie in their buffer (pxl-engine raster.rs),
    // as the raster reads them, and a box's 36 corners as `corner` winds
    // them, then wound the other way, three bits a corner and ten a word
    // (`raster_corner`): Adreno's driver refuses a table of vectors read at
    // a place found as it runs (doc/experiment/20261103-shaders-on-two-phones).
    // The depth a hit's box about the eye
    // is drawn at, short of a lack's 1: finite, as every literal here is.
    {
        use pxl_engine::raster::{
            corner, BOX_FACES, BOX_HI, BOX_ID, BOX_LO, BOX_WORDS, EYED, KEYS, KEY_WORDS, LACK, NEAR,
        };
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
            "const RASTER_CORNERS = array<u32, {}>({});\nfn raster_corner(k: u32) -> vec3f {{\n  let b = (RASTER_CORNERS[k / 10u] >> (3u * (k % 10u))) & 7u;\n  return vec3f(f32(b & 1u), f32((b >> 1u) & 1u), f32(b >> 2u));\n}}\nconst RASTER_BOX_WORDS: u32 = {BOX_WORDS}u;\nconst RASTER_BOX_LO: u32 = {BOX_LO}u;\nconst RASTER_BOX_HI: u32 = {BOX_HI}u;\nconst RASTER_BOX_ID: u32 = {BOX_ID}u;\nconst RASTER_BOX_FACES: u32 = {BOX_FACES}u;\nconst RASTER_FACES_MASK: u32 = {}u;\nconst RASTER_KEYS: u32 = {KEYS}u;\nconst RASTER_KEY_WORDS: u32 = {KEY_WORDS}u;\nconst RASTER_EYED: u32 = {EYED}u;\nconst RASTER_LACK: u32 = {LACK}u;\nconst RASTER_EYED_DEPTH: f32 = {eyed_depth:?};\nconst RASTER_NEAR: f32 = {NEAR:?};\n",
            corners.len(),
            corners.join(", "),
            EYED - 1
        );
        std::fs::write(out.join("raster_box.wgsl"), raster).expect("the raster's boxes' shader");
    }
    // The eye's words the screen pass reads (pxl-engine eye.rs), each its
    // word, and the vec4s it binds to reach the last of them.
    use pxl_engine::eye::{FAR, FORWARD, JITTER, ORIGIN, RIGHT, TAN, UP};
    let last = (ORIGIN + 2)
        .max(FAR)
        .max(FORWARD + 2)
        .max(RIGHT + 2)
        .max(UP + 2)
        .max(TAN + 1)
        .max(JITTER + 1);
    let eye = format!(
        "const EYE_ORIGIN: u32 = {ORIGIN}u;\nconst EYE_FAR: u32 = {FAR}u;\nconst EYE_FORWARD: u32 = {FORWARD}u;\nconst EYE_RIGHT: u32 = {RIGHT}u;\nconst EYE_UP: u32 = {UP}u;\nconst EYE_TAN: u32 = {TAN}u;\nconst EYE_JITTER: u32 = {JITTER}u;\nconst EYE_READ_VEC4S: u32 = {}u;\n",
        last / 4 + 1
    );
    std::fs::write(out.join("eye.wgsl"), eye).expect("the eye's shader");
    // A light slot's word read as its depth, a share of the light box's, as
    // pxl-engine light.rs `word` lays it: the one place a word's depth is
    // read; and a box tile's slots a side, which the lay reads a tile's read word by.
    use pxl_engine::light::DEPTH_ONE;
    let light = format!(
        "fn light_depth(w: u32) -> f32 {{ return f32(w) / {DEPTH_ONE}.0; }}\nconst LAY_TILE: u32 = {LIGHT_TILE}u;\n"
    );
    std::fs::write(out.join("light_word.wgsl"), light).expect("the light word's shader");
    std::fs::write(out.join("tile_word.wgsl"), tile_word()).expect("the tile word's shader");
    // The cloud field's lattice unit and the hash its cells draw from, the
    // cumulus's own on the CPU (pxl-content cloud.rs `lattice_hash`): its
    // multipliers, its rounds and its share's bits as the Rust takes them.
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
    assemble(&out);
    println!("cargo:rerun-if-changed=src/frame/shader");
    println!("cargo:rerun-if-changed=src/frame/shader/word.rs");
}

/// WGSL has no string literals, so `//` always starts a comment. Lines are never
/// joined, so tokens cannot run together; block comments are left alone.
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

// light_list.wgsl's `light_item` reads a workgroup's ids as `SAMPLE_SIDE` × `SAMPLE_SIDE` lanes
// of one entry of the list, and every pass that calls it declares its workgroup by a side of its
// own: beam_hit.wgsl (through `beam_item`) by `CORNER_SIDE`, light_cell.wgsl by `SAMPLE_SIDE`,
// light_sample_box.wgsl (through `sample_item`) by `SQUARE_SIDE`, light.wgsl by its tile. The cast sizes each pass's
// whole dispatch by that pass's own side (gpu/ray/cast.rs `light_args`), so each is that side.
const _: () = assert!(
    LIGHT_TILE == SAMPLE_SIDE && CORNER_SIDE == SAMPLE_SIDE && word::SQUARE_SIDE == SAMPLE_SIDE,
    "light_item reads a workgroup as SAMPLE_SIDE x SAMPLE_SIDE lanes: a light tile, CORNER_SIDE \
     and SQUARE_SIDE are each that side"
);

/// What the walk over the node cache reads by hand (sample_hit.wgsl), from
/// pxl-engine, whose layouts they are.
fn walk_word() -> String {
    use pxl_engine::area::Shard;
    use pxl_engine::clipmap::{self, Lookup};
    use pxl_engine::overwrite::{NodeSlot, BOXES};
    use pxl_engine::{
        answer, beam, cast, count, eye, face, light, node_cache, param, raster, ray, value,
        SHARD_ZOOM,
    };
    let words: &[(&str, u32)] = &[
        ("EYE_VEC4S", eye::WORDS as u32 / 4),
        ("ORIGIN", eye::ORIGIN as u32),
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
        ("LINE_WORDS", beam::LINE_WORDS),
        ("NODE_ZOOM", beam::NODE_ZOOM),
        ("NODE_CORNER", beam::NODE_CORNER),
        ("NODE_ROOTS", beam::NODE_ROOTS),
        ("SAMPLE_NODE", beam::SAMPLE_NODE),
        ("SAMPLE_ASSET", beam::SAMPLE_ASSET),
        ("SAMPLE_LEFT_T", beam::SAMPLE_LEFT_T),
        ("SAMPLE_LACKS", beam::SAMPLE_LACKS),
        ("SAMPLE_AXIS", beam::SAMPLE_AXIS),
        ("NODE_WORDS", beam::NODE_WORDS),
        ("VOXEL_NODE", beam::VOXEL_NODE),
        ("VOXEL_ASSET", beam::VOXEL_ASSET),
        ("VOXEL_LACKS", beam::VOXEL_LACKS),
        ("VOXEL_BAND", beam::VOXEL_BAND),
        ("VOXEL_FACES", beam::VOXEL_FACES),
        ("VOXEL_WORDS", beam::VOXEL_WORDS),
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
        ("TAG_BITS", node_cache::TAG_BITS),
        ("KEY_ZOOMS", !((1u32 << node_cache::TAG_BITS) - 1)),
        ("KIND_SPLIT", answer::Kind::Split as u32),
        ("KIND_CELL", answer::Kind::Cell as u32),
        ("VAL_MASK", answer::PLACED_ASSET - 1),
        ("VALUE_VOID", value::VOID),
        ("VALUE_AIR", value::AIR),
        ("FILL_SHIFT", answer::FILL_SHIFT),
        ("EIGHTHS", answer::EIGHTHS),
        ("BOX_BITS", answer::BOX_BITS),
        ("BOX_STEPS", answer::BOX_STEPS),
        ("STEPS_MOST", cast::STEPS_MOST),
        ("JITTER", eye::JITTER as u32),
        ("LIGHT", eye::LIGHT as u32),
        ("LIGHT_CLAIM_BUDGET", light::CLAIM_BUDGET),
        ("LIGHT_HEAD_VEC4S", light::HEAD_VEC4S as u32),
        ("LIGHT_HEAD_MISSES_CNT", light::HEAD_MISSES_CNT as u32),
        ("LIGHT_STARTS_AT", light::STARTS_AT as u32),
        ("LIGHT_ANCHOR", light::ANCHOR as u32),
        ("LIGHT_STEP_BITS", light::STEP_BITS),
        ("LIGHT_TILE", light::TILE),
        ("LIGHT_DEPTH_ONE", light::DEPTH_ONE),
        ("LIGHT_HEAD_SIZE", light::HEAD_SIZE as u32),
        ("LIGHT_MISSED_PASS", node_cache::LIGHT_MISSED_PASS),
        ("MISSED_PASS", node_cache::MISSED_PASS),
        (
            "EYE_MISS_COUNT",
            node_cache::lane_at(count::EYE, count::MISS),
        ),
        ("HITS_ARGS", node_cache::HITS_ARGS),
        ("HITS_CNT", node_cache::HITS_CNT),
        ("RAY_MISS", ray::MISS),
        (
            "LIGHT_ASKED_COUNT",
            node_cache::lane_at(count::LIGHT, count::ASKED),
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
        ("LIGHT_WIDTH_INVERSE", light::W_WIDTH_INVERSE as u32),
        ("LIGHT_BAND_SCALE", light::W_BAND_SCALE as u32),
        ("LIGHT_CORNER_X", light::CORNER[0] as u32),
        ("LIGHT_CORNER_Y", light::CORNER[1] as u32),
        ("LIGHT_HEAD_CAST", light::HEAD_CAST as u32),
        ("LIGHT_HEAD_EPOCH", light::HEAD_EPOCH as u32),
        ("LIGHT_HEAD_CELLS", light::HEAD_CELLS as u32),
        ("LIGHT_HEAD_CELLS_ACROSS", light::HEAD_CELLS_ACROSS as u32),
        ("LIGHT_HEAD_SAMPLES", light::HEAD_SAMPLES as u32),
        (
            "LIGHT_HEAD_SAMPLES_ACROSS",
            light::HEAD_SAMPLES_ACROSS as u32,
        ),
        ("LIGHT_HEAD_LEFTS", light::HEAD_LEFTS as u32),
        ("LIGHT_HEAD_READS", light::HEAD_READS as u32),
        ("LIGHT_HEAD_TILES", light::HEAD_TILES as u32),
        ("LIGHT_HEAD_HANDS", light::HEAD_HANDS as u32),
        ("LIGHT_HEAD_STANDS", light::HEAD_STANDS as u32),
        ("LIGHT_HEAD_BEAM_ACROSS", light::HEAD_BEAM_ACROSS as u32),
        ("LIGHT_HEAD_MISSES", light::HEAD_MISSES as u32),
        ("LIGHT_HEAD_MISSES_LEFT", light::HEAD_MISSES_LEFT as u32),
        ("LIGHT_HEAD_CORNERS_LEFT", light::HEAD_CORNERS_LEFT as u32),
        ("LIGHT_HEAD_VOXELS", light::HEAD_VOXELS as u32),
        ("LIGHT_HEAD_MARKS", light::HEAD_MARKS as u32),
        ("LIGHT_HEAD_LISTED", light::HEAD_LISTED as u32),
        ("LIGHT_LIST_ACROSS", light::LIST_ACROSS),
        ("LIGHT_LISTED_LANES", light::LISTED_LANES),
        ("LIGHT_BEAM", light::BEAM),
        ("LIGHT_SAMPLE", param::light::SAMPLE),
        ("LIGHT_LINE_WORDS", light::LINE_WORDS),
        ("LIGHT_LINE_ORIGIN", light::LINE_ORIGIN),
        ("LIGHT_BEAM_WINDOW", light::BEAM_WINDOW),
        ("LIGHT_LISTED_CORNERS", light::LISTED_CORNERS),
        ("LIGHT_LISTED_FED", light::LISTED_FED),
        ("LIGHT_LISTED_CLAIMED", light::LISTED_CLAIMED),
        ("LIGHT_HEAD_COUNTED", light::HEAD_COUNTED as u32),
        ("LIGHT_BEAM_PASS", node_cache::LIGHT_BEAM_PASS),
        (
            "LIGHT_BEAM_COUNT",
            node_cache::lane_at(count::LIGHT, count::BEAM),
        ),
        ("VISIBLE_LACK", raster::LACK),
        ("MISS_ARGS_LIST", ray::MISS_ARGS_LIST),
        ("MISS_ARGS_CNT", ray::MISS_ARGS_CNT),
        ("COUNT_RAYS", count::RAYS),
        ("MISS_ARGS_COUNT", ray::MISS_ARGS_COUNT),
        ("COUNT_REUSED", count::REUSED),
        ("COUNT_SKY", count::SKY),
        ("COUNT_WALKED", count::WALKED),
        ("SAMPLES_PASS", node_cache::SAMPLES_PASS),
        ("LIGHT_SAMPLES_PASS", node_cache::LIGHT_SAMPLES_PASS),
        (
            "EYE_SAMPLE_COUNT",
            node_cache::lane_at(count::EYE, count::SAMPLE),
        ),
        (
            "EYE_ASKED_COUNT",
            node_cache::lane_at(count::EYE, count::ASKED),
        ),
        (
            "LIGHT_SAMPLE_COUNT",
            node_cache::lane_at(count::LIGHT, count::SAMPLE),
        ),
        ("GRID_S", beam::GRID_S as u32),
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
        ("GRID_LATTICE", beam::GRID_LATTICE as u32),
        ("GRID_LEFTS_AT", beam::GRID_LEFTS_AT as u32),
        ("GRID_MISSES_AT", beam::GRID_MISSES_AT as u32),
        ("GRID_CORNERS_AT", beam::GRID_CORNERS_AT as u32),
        ("GRID_HANDS_AT", beam::GRID_HANDS_AT as u32),
        ("GRID_VOXELS_AT", beam::GRID_VOXELS_AT as u32),
        ("GRID_S_INVERSE", beam::GRID_S_INVERSE as u32),
        ("GRID_SLOPE", beam::GRID_SLOPE as u32),
        ("STARTS_ACROSS", light::STARTS_ACROSS as u32),
        ("STARTS_DOWN", light::STARTS_DOWN as u32),
        ("STARTS_ANCHORS", light::STARTS_ANCHORS as u32),
        ("STARTS_FRACS", light::STARTS_FRACS as u32),
        (
            "LIGHT_MISS_COUNT",
            node_cache::lane_at(count::LIGHT, count::MISS),
        ),
        ("CARVE", eye::CARVE as u32),
        ("HIDE", eye::HIDE as u32),
        ("FRAME", node_cache::FRAME),
        ("PASSES", node_cache::PASSES),
        ("COUNT_ON", node_cache::COUNT_ON),
        ("Y_START", cast::Y_START),
        ("Z_START", cast::Z_START),
        ("SHARD_ZOOM", SHARD_ZOOM),
        ("SHARD_UNKNOWN", Shard::UNKNOWN.word()),
        ("LOOKUP_NONE", Lookup::NONE.word()),
        ("SLOT_NOTHING", NodeSlot::NOTHING.word()),
        ("LINE_INHERITANCE", beam::LINE_INHERITANCE),
        ("LINE_STEP", beam::LINE_STEP),
        ("W_INHERITANCE", node_cache::W_INHERITANCE),
        ("MISS_ACROSS", ray::MISS_ACROSS),
        ("MISS_LANES", ray::MISS_LANES),
        ("MARKS", node_cache::MARKS),
        ("MARKS_AT", node_cache::MARKS_AT),
        ("MARKS_CNT", node_cache::MARKS_CNT),
        ("HITS", beam::HITS),
        ("HIT_LANES", beam::HIT_LANES),
        ("HIT_WORDS", beam::HIT_WORDS),
        ("HIT_NODE", beam::HIT_NODE),
        ("HIT_FACE", beam::HIT_FACE),
        ("HIT_PX", beam::HIT_PX),
        ("HIT_WAITS", beam::HIT_WAITS),
        ("HIT_OUT", beam::HIT_OUT),
        ("HIT_SQUARE", beam::HIT_SQUARE),
        ("HIT_FOLLOWS", beam::HIT_FOLLOWS),
        ("MISS_ARGS_ROOM", ray::MISS_ARGS_ROOM),
        ("FACE_BOX", face::BOX),
        ("RAY_FACE_SHIFT", ray::FACE_SHIFT),
        ("RAY_ZOOM_SHIFT", ray::ZOOM_SHIFT),
        ("RAY_FILL_SHIFT", ray::FILL_SHIFT),
        ("CAST_SIDE", ray::CAST_SIDE),
        ("MISS_ARGS_WHOLE", ray::MISS_ARGS_WHOLE),
        ("SCREEN_TILE", screen::TILE),
        ("RASTER_ASKED_AT", raster::ASKED_AT),
        ("CORNER_SIDE", CORNER_SIDE),
        ("BEAM_PASS", node_cache::BEAM_PASS),
        (
            "EYE_BEAM_COUNT",
            node_cache::lane_at(count::EYE, count::BEAM),
        ),
        ("RASTER_ARGS_INSTANCES", raster::ARGS_INSTANCES),
        ("RASTER_ARGS_ASKS", raster::ARGS_ASKS),
        ("RASTER_ARGS_CNT", raster::ARGS_CNT),
        ("RASTER_FACES_LANES", raster::FACES_LANES),
        ("LIGHT_HEAD_CORNER_WAS", light::HEAD_CORNER_WAS as u32),
        ("LIGHT_HEAD_FOOTPRINT", light::HEAD_FOOTPRINT as u32),
        ("LIGHT_ACTORS_AT", light::ACTORS_AT as u32),
        ("LIGHT_ACTORS", light::ACTORS as u32),
        ("LIGHT_COLUMNS_AT", light::COLUMNS_AT as u32),
        ("LIGHT_COLUMNS", light::COLUMNS as u32),
        ("LIGHT_REACH_NDC", light::REACH_NDC as u32),
        ("LIGHT_ARGS_BEAM", light::ARGS_BEAM),
        ("LIGHT_ARGS_CELLS", light::ARGS_CELLS),
        ("LIGHT_ARGS_SAMPLE", light::ARGS_SAMPLE),
        ("LIGHT_ARGS_SAMPLE_BOX", light::ARGS_SAMPLE_BOX),
        ("LIGHT_ARGS_SLOT", light::ARGS_SLOT),
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
        ("NODE_CACHE_MOVED_AT", node_cache::MOVED_AT),
        ("NODE_CACHE_MOVED_BRICKS_AT", node_cache::MOVED_BRICKS_AT),
        ("NODE_CACHE_BOXES_AT", node_cache::BOXES_AT),
        ("NODE_CACHE_WRITTEN_AT", node_cache::WRITTEN_AT),
        ("NODE_CACHE_WRITTEN_CAP", node_cache::WRITTEN_CAP),
        ("NODE_CACHE_BOXES", BOXES),
        ("W_KEY", node_cache::W_KEY),
        ("W_PLACED", node_cache::W_PLACED),
        ("EYE_CLIP_LEVELS", clipmap::EYE_CLIP_LEVELS),
    ];
    // How far short of where a beam ray of the light stopped its cells' rays start: a share of
    // its spacing, and Q3 steps (`light::margin`). A slot a receiver reads lies within the disk
    // grown by its taps' reach, and a tile holds one where its middle lies within it grown by
    // half the tile's diagonal (`light::TILE_READ_PAD`, light_propagate.wgsl).
    let tile_pad = light::TILE_READ_PAD;
    let mut out = format!(
        "const INV_SPLIT2: f32 = {:?};\nconst NARROW: f32 = {:?};\nconst NARROW_SQUARED: f32 = {:?};\nconst LIGHT_MARGIN_PER_WIDTH: f32 = {:?};\nconst LIGHT_MARGIN_STEPS: f32 = {:?};\nconst LIGHT_TILE_READ_PAD: f32 = {tile_pad:?};\nconst LIGHT_FOOTPRINT_TILES: i32 = {}i;\nconst LIGHT_READ_FIRST: i32 = {}i;\nconst LIGHT_READ_LAST: i32 = {}i;\nconst FRAC_1_SQRT_2: f32 = {:?};\nconst SAMPLE_SPACING: f32 = {:?};\nconst SAMPLE_SPACING_INVERSE: f32 = {:?};\n",
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
    );
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    out
}

/// `answer::side`, the side of the box a cell keeping `k` eighths is drawn.
fn side_word() -> String {
    let cases: String = (1..answer::EIGHTHS)
        .map(|k| format!("case {k}u: {{ return {:?}; }} ", answer::side(k)))
        .collect();
    format!(
        "fn side(k: u32) -> f32 {{ switch (k) {{ {cases}default: {{ return {:?}; }} }} }}\n",
        answer::side(answer::EIGHTHS)
    )
}

/// What the world's one query by hand reads (world.wgsl), from pxl-engine and
/// pxl-content, whose layouts and rules they are.
fn world_word() -> String {
    use pxl_content::cave::{
        BLOCK, BLOCKS, CELL_BLOCKS, CROSS, PIPE, ROOT, ROOT_BLOCKS, SPAWN, SPLIT, WALK,
    };
    use pxl_content::cube::ground as palette;
    use pxl_content::param::{cave, strata};
    use pxl_engine::clipmap as clip;
    use pxl_engine::overwrite::{self, Cut};
    use pxl_engine::way::tree;
    use pxl_engine::{area, seed, value};
    use xpute_kit::math::rng;
    let words: &[(&str, u32)] = &[
        ("LEAF_CELL", pxl_engine::LEAF_CELL),
        ("TOP_ZOOM", pxl_engine::TOP_ZOOM),
        ("VALUE_UNKNOWN", value::UNKNOWN),
        ("GROUNDS_FIRST", value::GROUNDS[0]),
        ("GROUNDS_LAST", value::GROUNDS[1]),
        ("OBJECTS_FIRST", value::OBJECTS[0]),
        ("OBJECTS_LAST", value::OBJECTS[1]),
        ("KIND_LACKS", answer::Kind::Lacks as u32),
        ("PLACED_ASSET", answer::PLACED_ASSET),
        ("FULL", answer::FULL),
        ("FULL_BITS", answer::FULL.trailing_zeros()),
        ("FILL_BITS", answer::FILL_BITS),
        ("BRANCH", overwrite::BRANCH),
        ("OBJECT_ROOT", overwrite::OBJECT_ROOT),
        ("CHILDREN", overwrite::CHILDREN),
        ("SLOT_FOLDED", overwrite::FOLDED),
        ("SLOT_ZOOM_MASK", overwrite::ZOOM_MASK),
        ("SLOT_SHIFT", overwrite::SLOT_SHIFT),
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
        ("BRICK_LEVELS", clip::BRICK_LEVELS),
        ("ACROSS", clip::ACROSS),
        ("CLIP_ENTRIES", clip::ENTRIES),
        ("FIRST_CELL", clip::FIRST_CELL),
        ("ORIGIN_AT", clip::ORIGIN_AT),
        ("JOINED_AT", clip::JOINED_AT),
        ("JOINED_ENTRIES", clip::JOINED_ENTRIES),
        ("JOINED_LEVELS", clip::JOINED_LEVELS),
        ("COLUMNS_AT", clip::COLUMNS_AT),
        ("BOUNDS_AT", clip::BOUNDS_AT),
        ("BOUND_ENTRY_WORDS", clip::BOUND_ENTRY_WORDS),
        ("LAID_AT", clip::LAID_AT),
        ("LAID_CELLS", clip::LAID_CELLS),
        ("ROOT_AT", clip::ROOT_AT),
        ("LOOKUP_SLOT_BITS", clip::LOOKUP_SLOT_BITS),
        ("LOOKUP_FROM_SHIFT", clip::LOOKUP_FROM_SHIFT),
        ("LOOKUP_CELL_SHIFT", clip::LOOKUP_CELL_SHIFT),
        ("LOOKUP_AT", (1 << clip::LOOKUP_CELL_SHIFT) - 1),
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
    // Where a brick's level `j` begins among its levels, in entries (`clipmap::bound_level_at`).
    let cases: String = (0..=clip::BRICK_LEVELS)
        .map(|j| format!("case {j}u: {{ return {}u; }} ", clip::bound_level_at(j)))
        .collect();
    out + &format!(
        "fn bound_level_at(j: u32) -> u32 {{ switch j {{ {cases}default: {{ return {}u; }} }} }}\n",
        clip::bound_level_at(clip::BRICK_LEVELS + 1)
    )
}

/// What the ways' fill by hand reads (way.wgsl, way_tree.wgsl and the free stack's passes),
/// from pxl-engine, whose layouts and rules they are; what the world's words hold
/// already, theirs.
fn way_word() -> String {
    use pxl_engine::clipmap as clip;
    use pxl_engine::param::way as param;
    use pxl_engine::way::{self, tree};
    use pxl_engine::{fill, value};
    let words: &[(&str, u32)] = &[
        ("HANDED_WORDS", clip::HANDED_WORDS),
        ("FILL_BRICK", clip::FILL_BRICK),
        ("FILL_PLACE", clip::FILL_PLACE),
        ("FILL_CELL", clip::FILL_CELL),
        ("FILL_FIRST", clip::FILL_FIRST),
        ("FILL_SHARD", clip::FILL_SHARD),
        ("FILL_CAP", clip::FILL_CAP),
        ("ELE_AT", clip::ELE_AT),
        ("ELE_PAD", ground::ELE_PAD),
        ("ELE_PADDED_SPAN", ground::ELE_PADDED_SPAN),
        ("CELL_STEPS", ground::CELL_STEPS as u32),
        ("LINES_AT", clip::LINES_AT),
        ("LINES_ENTRIES_AT", clip::LINES_ENTRIES_AT),
        ("LINES_SEGMENTS_AT", clip::LINES_SEGMENTS_AT),
        ("WAYS_SHIFT", clip::WAYS_SHIFT),
        ("WAYS_BINS_ACROSS", clip::WAYS_BINS_ACROSS),
        ("HEADER_WORDS", way::HEADER_WORDS),
        ("HEADER_PER_M", way::HEADER_PER_M as u32),
        ("HEADER_STEP", way::HEADER_STEP as u32),
        ("HEADER_PHASE", way::HEADER_PHASE as u32),
        ("HEADER_LEAF", way::HEADER_LEAF as u32),
        ("SEGMENT_WORDS", way::SEGMENT_WORDS as u32),
        ("SEGMENT_DIR", way::SEGMENT_DIR as u32),
        ("DIR_BITS", way::DIR_BITS),
        ("POINT_SCALE", way::POINT_SCALE as u32),
        ("BRIDGE", way::BRIDGE),
        ("TUNNEL", way::TUNNEL),
        ("WAY_NOTHING", way::NOTHING),
        ("WAY_AIR", way::AIR),
        ("AIR_OF", way::AIR_OF),
        ("VALUE_PIER", value::PIER),
        ("TRANSPORTS_FIRST", value::TRANSPORTS[0]),
        ("TRANSPORTS_LAST", value::TRANSPORTS[1]),
        ("SPANS", way::SPANS as u32),
        ("SPANS_AT", clip::SPANS_AT),
        ("SPANS_ACROSS", clip::SPANS_ACROSS),
        ("SPANS_COLUMNS", clip::SPANS_COLUMNS),
        ("SPANS_COLUMN_WORDS", clip::SPANS_COLUMN_WORDS),
        ("SPANS_CNT", fill::SPANS_CNT),
        ("SPANS_PAST", fill::SPANS_PAST),
        ("SPANS_LANES", fill::SPANS_LANES),
        ("FREED_AT", tree::FREED_AT),
        ("RECORD_WORDS", tree::RECORD_WORDS),
        ("BRICK_CHILD_GROUPS", tree::BRICK_CHILD_GROUPS),
        ("TREE_NONE", tree::NONE),
        ("TREE_CRAMPED", tree::CRAMPED),
        ("TREE_STARVED", tree::STARVED),
        ("REL_MASK", tree::REL_MASK),
        ("Z_SHIFT", tree::Z_SHIFT),
        ("TAKEN_CNT", tree::TAKEN_CNT),
        ("WAY_LANES", tree::LANES),
    ];
    let floats: &[(&str, f32)] = &[
        ("DECK_THICKNESS_M", param::DECK_THICKNESS_M),
        ("PIER_MIN_RISE_M", param::PIER_MIN_RISE_M),
        ("LOWER_DECK_GAP_M", way::LOWER_DECK_GAP_M),
    ];
    let mut out = String::new();
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    for (name, v) in floats {
        out += &format!("const {name}: f32 = {v:?};\n");
    }
    out
}

/// What decor's passes by hand read past the world's words, from pxl-content, whose layouts
/// they are: the records, the list, a row of the catalog, and what each decor's row
/// determines, by decor (`Decor::cap`, `scatter::root`, `generation::first_record`).
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
        ("HEAD_WORDS", g::HEAD_WORDS),
        ("CNT_AT", g::CNT_AT),
        ("SLOTS_AT", g::SLOTS_AT),
        ("BRICK_RECORDS", g::BRICK_RECORDS),
        ("UNGENERATED", g::UNGENERATED),
        ("TURN_SHIFT", g::TURN_SHIFT),
        ("VARIANT_SHIFT", g::VARIANT_SHIFT),
        ("TURN_BITS", g::TURN_BITS),
        ("CELL_SHIFT", g::CELL_SHIFT),
        ("THIN_ARGS", g::THIN_ARGS),
        ("THIN_CNT", g::THIN_CNT),
        ("GENERATE_CNT", g::GENERATE_CNT),
        ("LIST_BOXES", g::LIST_BOXES),
        ("BOXES_AT", g::BOXES_AT),
        ("BOX_WORDS", g::BOX_WORDS),
        ("BOX_CAP", g::BOX_CAP),
        ("EVERY", g::EVERY),
        ("LIST_EVERY", g::LIST_EVERY),
        ("LIST_PARTS", g::LIST_PARTS),
        ("GENERATE_ARGS", g::GENERATE_ARGS),
        ("SLOT_RECORDS", g::SLOT_RECORDS),
        ("PLACED_HEAD", g::PLACED_HEAD),
        ("PLACED_HEAD_WORDS", g::PLACED_HEAD_WORDS),
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
        ("COUNT_TAG", scatter::COUNT),
        ("SPLIT_TAG", scatter::SPLIT),
        ("DECORS", DECORS.len() as u32),
        ("FIRST_VAL", DECORS[0].val),
        ("ROW_WORDS", row(size_of::<Decor>())),
        ("ROW_SEED", row(offset_of!(Decor, seed))),
        ("ROW_ORIENTATION", row(offset_of!(Decor, orientation))),
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
        ("UNIFORM_IDX", drawn::UNIFORM_IDX),
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
    // What each decor's row determines, a case a decor.
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
    // A record's turn as its spin, by the bits a turn's float has (decor_compact.wgsl), and
    // how near a thing is taken to be (`drawn::NEAREST_U`).
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
    // How far under its box's corner a cell keeping `k` eighths leaves its top, a share of
    // the cell's side (`cast::top`, `generation::footing`), by its bits: the Rust's own float.
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

/// What a handed brick's tops read by hand past the ways' words (height.wgsl), from
/// pxl-engine, whose layouts they are: where its cells' palette words lie in its record,
/// and a word's four palette indices.
fn fill_word() -> String {
    use pxl_engine::clipmap as clip;
    let mut out = format!("const PALETTE_AT: u32 = {}u;\n", clip::PALETTE_AT);
    // A cell's word's four palette indices, `dx + 2·dy` (ground.rs `palette_indices`).
    let b = ground::PALETTE_BITS;
    let m = (1u32 << b) - 1;
    out += &format!(
        "fn palette_four(w: u32) -> vec4<u32> {{ return vec4<u32>(w & {m}u, (w >> {b}u) & {m}u, (w >> {}u) & {m}u, w >> {}u); }}\n",
        2 * b,
        3 * b
    );
    out
}

/// The tiles' uniform as every pass reads it (pxl-engine
/// `screen::TILES_WORDS`, gpu/tiles.rs `Tiles::words`), and a tile's word
/// read by it as pxl-engine's `screen::age` and `screen::claimed` read it:
/// the frames since it changed, wrapping, and claimed while fewer than the
/// uniform's `claimed_for`. The one rule every pass that reads the tiles
/// claims by (screen_tiles.wgsl, screen_propagate.wgsl, cast_tiles.wgsl).
fn tile_word() -> String {
    // The Rust's rule, as the WGSL below spells it: a u32's difference that
    // wraps, a claim that ends at `claimed_for`.
    assert_eq!(screen::age(0, u32::MAX), 1);
    assert_eq!(screen::age(5, 5), 0);
    assert!(screen::claimed(0, 1) && !screen::claimed(1, 1));
    assert!(screen::claimed(u32::MAX - 1, u32::MAX) && !screen::claimed(u32::MAX, u32::MAX));
    format!(
        concat!(
            "const CAST_TILES_LANES: u32 = {}u;\n",
            "struct Tiles {{ count: vec4u, size: vec4f, }};\n",
            "fn tiles_age(t: Tiles, changed_at: u32) -> u32 {{ return t.count.w - changed_at; }}\n",
            "fn tiles_claimed(t: Tiles, changed_at: u32) -> bool {{ return tiles_age(t, changed_at) < t.count.z; }}\n",
        ),
        screen::CAST_TILES_LANES
    )
}

/// Every setting (src/param.rs), named by its path under `param` (`sky::sun`'s
/// `LIMB` is `SKY_SUN_LIMB`); the host's alone (`F64`) is not written.
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

/// What the host lays out and every shader reads (frame/shader/word.rs), as
/// WGSL: a uniform's floats as its vec4s; and the pxl-engine values every
/// shader may name, the world's height and the light's taps apart.
fn words() -> String {
    use word::*;
    let vec4s = |name: &str, floats: usize| {
        assert!(floats.is_multiple_of(4), "{name} is whole vec4s");
        format!("const {name}_VEC4S: u32 = {}u;\n", floats / 4)
    };
    [
        vec4s("DRAW", DRAW_FLOATS),
        vec4s("FRAME", FRAME_FLOATS),
        vec4s("SKY", SKY_FLOATS),
        vec4s("ATMOS", ATMOS_FLOATS),
        vec4s("CLOUDS", CLOUDS_FLOATS),
        vec4s("STARS", STARS_FLOATS),
        vec4s("AO", AO_FLOATS),
        vec4s("MARKER", MARKER_FLOATS),
        vec4s("STAR_RECORD", STAR_RECORD_FLOATS),
        format!("const MARKER_INSTANCE_FLOATS: u32 = {MARKER_INSTANCE_FLOATS}u;\nconst LOCATION_ZONE_INSTANCE_FLOATS: u32 = {LOCATION_ZONE_INSTANCE_FLOATS}u;\n"),
        format!("const AO_DOWNSCALE: u32 = {AO_DOWNSCALE}u;\nconst AO_NOISE_SIDE: u32 = {AO_NOISE_SIDE}u;\nconst DECOR_GROUPS: u32 = {DECOR_GROUPS}u;\n"),
        // The depth's bound's square and a screen tile's squares a side, the
        // workgroup its passes take a tile by (screen_bound.wgsl,
        // screen_propagate.wgsl); the frames the jitter's sequence covers a pixel
        // in, each still frame's even share of it (pxl-engine
        // `screen::JITTERS`, screen_resolve.wgsl).
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
        format!("const MAX_COMPUTE_WORKGROUPS_PER_DIMENSION: u32 = {MAX_COMPUTE_WORKGROUPS_PER_DIMENSION}u;\n"),
        format!("const AIR_FROXELS: u32 = {AIR_FROXELS}u;\n"),
        format!("const SQUARE_SIDE: u32 = {SQUARE_SIDE}u;\nconst ROW_LANES: u32 = {ROW_LANES}u;\n"),
        format!("const HEIGHT_SIDE: u32 = {HEIGHT_SIDE}u;\nconst LEVEL_SIDE: u32 = {LEVEL_SIDE}u;\nconst FOLD_LANES: u32 = {FOLD_LANES}u;\nconst CELL_SIDE: u32 = {CELL_SIDE}u;\n"),
        format!("const LIGHT_SLOPES: u32 = {LIGHT_SLOPES}u;\nconst GROUND_SLOPES: u32 = {GROUND_SLOPES}u;\nconst SLOPE_WORDS: u32 = {SLOPE_WORDS}u;\n"),
        format!("const CLOUD_TILE_UNITS: u32 = {CLOUD_TILE_UNITS}u;\nconst CLOUD_TILE_TEXELS: u32 = {CLOUD_TILE_TEXELS}u;\n"),
        // Q3's Z, the world's height about sea level (pxl-engine).
        format!("const HEIGHT_BITS: u32 = {}u;\n", pxl_engine::HEIGHT_BITS),
        // The receiver's taps apart in the shadow map, which the map's read
        // pad is sized by (pxl-engine param.rs `light::SPREAD`, light.rs
        // `READ_PAD`).
        format!("const LIGHT_SPREAD: f32 = {:?};\n", light::SPREAD),
    ]
    .concat()
}

/// Each field's accessor of the uniform whose words `text` binds
/// (`<uniform>_words`, frame/shader/word.rs `UNIFORMS`): the field read off
/// its vec4s where the host wrote it, never as a struct, which lost Safari's
/// WebGPU its GPU process. Each field is checked to lie within its vec4s and
/// apart from the others.
fn accessors(text: &str) -> String {
    use word::{floats, Kind, UNIFORMS};
    let mut out = String::new();
    for (uniform, fields) in UNIFORMS {
        let words = format!("{uniform}_words");
        if !text.contains(&format!("var<uniform> {words}:")) {
            continue;
        }
        let mut taken = vec![false; floats(fields)];
        for f in fields {
            let (row, col, n) = (f.at / 4, f.at % 4, f.kind.floats());
            for t in &mut taken[f.at..f.at + n] {
                assert!(!*t, "{uniform}'s {} lies over another field", f.name);
                *t = true;
            }
            let lanes = &"xyzw"[col..(col + n).min(4)];
            let column = |k: usize| format!("{words}[{}].xyz", row + k);
            let vec4 = |k: usize| format!("{words}[{}]", row + k);
            let (ty, read) = match f.kind {
                Kind::F32 => ("f32", format!("{words}[{row}].{lanes}")),
                Kind::U32 => ("u32", format!("bitcast<u32>({words}[{row}].{lanes})")),
                Kind::Vec2 => ("vec2f", format!("{words}[{row}].{lanes}")),
                Kind::Vec2u => ("vec2u", format!("bitcast<vec2u>({words}[{row}].{lanes})")),
                Kind::Vec3 => ("vec3f", format!("{words}[{row}].{lanes}")),
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

/// `text` with only the functions its entry points reach, each line one statement as
/// `strip_comments` leaves it: a function's lines from its `fn` to its brace's close, an
/// entry one whose attributes stand on the line before or its own.
fn read_fns(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let lines: Vec<&str> = text.lines().collect();
    // Each function's name, its first line and its last.
    let mut fns = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    let mut entries = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        // A function's line, its attributes before `fn` where they stand on it.
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
    let mut kept = Vec::with_capacity(lines.len());
    let mut k = 0;
    while k < lines.len() {
        let dropped = fns
            .iter()
            .find(|(name, (first, _))| *first == k && !reached.contains(*name));
        match dropped {
            Some((_, &(_, last))) => k = last + 1,
            None => {
                kept.push(lines[k]);
                k += 1;
            }
        }
    }
    kept.join("\n") + "\n"
}

/// `text` with only the structs something in it reads, each line one statement as
/// `strip_comments` leaves it: a struct's lines from its `struct` to its brace's close.
fn read_structs(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut lines: Vec<&str> = text.lines().collect();
    loop {
        // Each struct's name, its first line and its last.
        let mut structs = Vec::new();
        let mut k = 0;
        while k < lines.len() {
            let Some(rest) = lines[k].strip_prefix("struct ") else {
                k += 1;
                continue;
            };
            let name = &rest[..rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())];
            // Its brace closes on its own line, or on a line of its own after.
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

/// The faults the S23 and Safari showed, refused in every assembled module, compute and
/// render (doc/experiment/20261104-taking-rust-gpu-out, its faults seen;
/// doc/tip/shaders-phones-take.md): no struct holds a struct, directly or in an array; no
/// function's local is a struct, but a vertex or fragment entry point's local of the
/// type it returns; no function's local is an array, or holds one; no `i32::MIN`, written
/// or folded, which Safari writes as a C++ long that no `select` with an int takes.
fn held_to_faults(module: &naga::Module, name: &str) {
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
    let held_min = module
        .global_expressions
        .iter()
        .filter(|(_, e)| i32_min(e))
        .count()
        + functions
            .chain(module.entry_points.iter().map(|e| &e.function))
            .map(|f| f.expressions.iter().filter(|(_, e)| i32_min(e)).count())
            .sum::<usize>();
    if held_min > 0 {
        faults.push(format!("i32::MIN held {held_min} times"));
    }
    assert!(faults.is_empty(), "{name}: {}", faults.join("; "));
}

/// Every shader assembled from its parts (part.rs `PARTS`), validated as
/// WebGPU takes it, every capability off; written where record.rs `source`
/// includes it; and which stages read each of its bindings, read off its
/// entry points (`STAGES`, in OUT_DIR), which every pipeline's groups take their
/// visibility from (record.rs `pipeline`).
fn assemble(out: &Path) {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    let read = |name: &str| {
        std::fs::read_to_string(out.join(format!("{name}.wgsl")))
            .unwrap_or_else(|e| panic!("{name}.wgsl: {e}"))
    };
    let constant = read("constant");
    let (mut sources, mut stages) = (String::new(), String::new());
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
        // The raster's view as the host writes it (frame/shader/word.rs `RASTER_FLOATS`).
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
        // The lists' words as pxl-engine lays them (list.wgsl `MissArgs`, `Left`; miss_hit.wgsl
        // `MissLeft` begins as `Left`): each member's first word.
        {
            use pxl_engine::ray;
            let words: [(&str, &[u32]); 3] = [
                (
                    "MissArgs",
                    &[
                        ray::MISS_ARGS_LIST,
                        ray::MISS_ARGS_WHOLE,
                        ray::MISS_ARGS_CNT,
                        ray::MISS_ARGS_COUNT,
                    ],
                ),
                ("Left", &[0, ray::LEFT_CNT]),
                ("MissLeft", &[0, ray::LEFT_CNT]),
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
                if name == "MissArgs" {
                    assert_eq!(
                        span as usize,
                        4 * ray::MISS_ARGS_WORDS,
                        "`MissArgs` against `ray::MISS_ARGS_WORDS`"
                    );
                }
            }
        }
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        held_to_faults(&module, &format!("shader {kind} ({parts:?})"));
        let file = format!("{kind}.wgsl");
        std::fs::write(out.join("assembled").join(&file), &text).expect("an assembled shader");
        sources += &format!(
            "        {kind} => include_str!(concat!(env!(\"OUT_DIR\"), \"/shader/assembled/{file}\")),\n"
        );
        // Each binding, the stages whose entry points read it, and the layout kinds that take
        // it as it is declared.
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
            // Two parts may declare one binding, one of them unread: the binding is the one read.
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
    // The world's query asked word for word, as the test and Tint take it, in no shader.
    let asked = [
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "level",
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
    held_to_faults(&module, "world_asked");
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
        format!("/// Each shader's bindings, [group, binding, the stages that read it, a bit a layout kind\n/// that takes it as declared, a bit a vertex entry that reads it (`VERTEX_ENTRY`)], read off it\n/// by build.rs.\npub const STAGES: [&[[u32; 5]]; SHADERS as usize] = [\n{stages}];\n{BINDS}"),
    )
    .expect("the shaders' stages");
}

/// Whether a compute group's (binding, stages, kind) words lay out what the shaders of the
/// pipelines over it read, written beside `STAGES`: every binding the shader of one of
/// `pipelines` reads is in group 0, has an entry in `group` seen from the compute stage, and
/// its kind is one its declaration takes (the number, a buffer's read-only or not, a
/// texture's kind); and every entry of `group` is read by one of them. Each owner of a
/// compute group asserts it.
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

/// The layout kinds (api.json's record words) that take `var` as its declaration has it, as
/// the bits of an expression over record.rs's names; none for a kind no record word lays out.
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
