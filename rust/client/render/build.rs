// pxl-render/build.rs

// The shipped WGSL is the shader files with comments and indentation stripped; nothing is renamed.

use naga::front::wgsl;
use pxl_engine::answer;
use pxl_engine::grid;
use pxl_engine::ground;
use pxl_engine::lattice;
use pxl_engine::param::light;
use pxl_engine::screen;
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
mod word;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
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
    let faces = format!("const RAY_FACES: u32 = {FACES}u;\nconst FACES_LINE: u32 = {LINE_SHIFT}u;\nconst FACES_RISE: u32 = {RISE_SHIFT}u;\nconst FACES_CORNER: u32 = {CORNER_SHIFT}u;\n");
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
            corner, BOX_AXIS, BOX_HI, BOX_ID, BOX_LO, BOX_WORDS, EYED, LACK, NEAR,
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
            "const RASTER_CORNERS = array<u32, {}>({});\nfn raster_corner(k: u32) -> vec3f {{\n  let b = (RASTER_CORNERS[k / 10u] >> (3u * (k % 10u))) & 7u;\n  return vec3f(f32(b & 1u), f32((b >> 1u) & 1u), f32(b >> 2u));\n}}\nconst RASTER_BOX_WORDS: u32 = {BOX_WORDS}u;\nconst RASTER_BOX_LO: u32 = {BOX_LO}u;\nconst RASTER_BOX_HI: u32 = {BOX_HI}u;\nconst RASTER_BOX_ID: u32 = {BOX_ID}u;\nconst RASTER_BOX_AXIS: u32 = {BOX_AXIS}u;\nconst RASTER_AXIS_MASK: u32 = {}u;\nconst RASTER_EYED: u32 = {EYED}u;\nconst RASTER_LACK: u32 = {LACK}u;\nconst RASTER_EYED_DEPTH: f32 = {eyed_depth:?};\nconst RASTER_NEAR: f32 = {NEAR:?};\n",
            corners.len(),
            corners.join(", "),
            EYED - 1
        );
        std::fs::write(out.join("raster_box.wgsl"), raster).expect("the raster's boxes' shader");
    }
    // The eye's words the screen pass reads (pxl-engine eye.rs), each its
    // word, and the vec4s it binds to reach the last of them.
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
    // A light slot's word read as its depth, a share of the light box's, as
    // pxl-engine light.rs `word` lays it: the one place a word's depth is
    // read.
    use pxl_engine::light::DEPTH_ONE;
    let light = format!("fn light_depth(w: u32) -> f32 {{ return f32(w) / {DEPTH_ONE}.0; }}\n");
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
    std::fs::write(out.join("debug_look.wgsl"), debug_looks()).expect("the debug views' looks");
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("compute.rs"),
        compute(dir),
    )
    .expect("the compute layouts");
    assemble(&out);
    println!("cargo:rerun-if-changed=src/frame/shader");
    println!("cargo:rerun-if-changed=src/frame/shader/word.rs");
}

/// The debug views' looks (debug_look.json) as WGSL: the grid's and the
/// tints decoded to light, which their passes mix in, and the steps' as they
/// are, which the pass writes to the screen; the ramp read straight between
/// its stops.
fn debug_looks() -> String {
    use debug_look::{share, steps};
    // A world's cell spans (1, `SPANS`] of the view's cells where the rays
    // stop on it (pxl-engine grid.rs `cell_zoom_at`), so its share of a cell
    // squared is within (1, `SPANS`²]: the tints' bounds are those.
    let spans = grid::SPANS;
    assert!(
        share::finer::most == 1.0 && share::within::most == spans * spans,
        "debug_look.json's share bounds are the split's"
    );
    let srgb = |c: u32| [c >> 16, c >> 8, c].map(|v| (v & 255) as f32 / 255.0);
    let light = |c: u32| {
        srgb(c).map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                libm::powf((v + 0.055) / 1.055, 2.4)
            }
        })
    };
    let parts = |v: [f32; 3]| format!("{:?}, {:?}, {:?}", v[0], v[1], v[2]);
    let vec3 = |v: [f32; 3]| format!("vec3f({})", parts(v));
    let ramp: Vec<String> = steps::ramp.iter().map(|&c| vec3(srgb(c))).collect();
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
        vec3(srgb(steps::none)),
        ramp.len(),
        ramp.join(", "),
        last - 1
    )
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

/// Each generated compute entry's group 0 as its WGSL declares it, a binding
/// its index, its stages and its kind (frame/shader/record.rs's codes), and
/// its workgroup: what a pipeline's layout is made of and a dispatch counts
/// by, read off the one module rust-gpu wrote.
fn compute(dir: &Path) -> String {
    let mut out = String::new();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("the shaders")
        .map(|e| e.expect("a shader").path())
        .collect();
    files.sort();
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let Some(stem) = name.strip_suffix(".gen.wgsl") else {
            continue;
        };
        let text = std::fs::read_to_string(&path).expect("a generated shader");
        let lines: Vec<&str> = text.lines().collect();
        let mut group = Vec::new();
        let mut workgroup = None;
        for (k, line) in lines.iter().enumerate() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("@group(0) @binding(") {
                let binding: u32 = rest
                    .split(')')
                    .next()
                    .and_then(|n| n.parse().ok())
                    .expect("a binding's index");
                let var = lines.get(k + 1).map(|l| l.trim()).unwrap_or_default();
                let kind = if var.starts_with("var<uniform>") {
                    "UNIFORM"
                } else if var.starts_with("var<storage, read_write>") {
                    "STORAGE_RW"
                } else if var.starts_with("var<storage>") {
                    "STORAGE"
                } else if var.contains("texture_storage_2d_array<r32uint,read_write>") {
                    "STORAGE_TEXTURE_RW"
                } else if var.contains("texture_2d_array<u32>") {
                    "TEXTURE_UINT_ARRAY"
                } else {
                    panic!("{name}: binding {binding} is of no kind a layout names: {var}");
                };
                group.push((binding, kind));
            }
            if let Some(rest) = line.strip_prefix("@compute @workgroup_size(") {
                workgroup = Some(
                    rest.split(')')
                        .next()
                        .expect("a workgroup")
                        .replace(' ', ""),
                );
            }
        }
        group.sort();
        let upper = stem.to_uppercase();
        let entries: Vec<String> = group
            .iter()
            .map(|(b, k)| format!("{b}, COMPUTE, {k}"))
            .collect();
        out += &format!(
            "pub const {upper}_GROUP: [u32; {}] = [{}];\n",
            3 * group.len(),
            entries.join(", ")
        );
        out += &format!(
            "pub const {upper}_WORKGROUP: [u32; 3] = [{}];\n",
            workgroup.expect("an entry's workgroup")
        );
        // Its layout within what every device binds (frame/shader/floor.rs).
        out += &format!("const _: () = assert!(floor::holds(&[&{upper}_GROUP]), \"{stem}'s group binds past WebGPU's floor\");\n");
    }
    out
}

/// The tiles' uniform as every pass reads it (pxl-engine
/// `screen::TILES_WORDS`, gpu/tiles.rs `Tiles::words`), and a tile's word
/// read by it as pxl-engine's `screen::age` and `screen::claimed` read it:
/// the frames since it changed, wrapping, and claimed while fewer than the
/// uniform's `claimed_for`. The one rule the screen's and the light's passes
/// claim by (screen_tiles.wgsl, screen_propagate.wgsl); pxl-shader's cast calls
/// the Rust itself.
/// What the walk over the observed pyramid reads by hand (sample_hit.wgsl), from
/// pxl-engine, whose layouts they are.
fn walk_word() -> String {
    use pxl_engine::{answer, beam, eye, face, param, pyramid, ray, value};
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
        ("LINE_VISITS", beam::LINE_VISITS),
        ("LINE_NODE", beam::LINE_NODE),
        ("LINE_WORDS", beam::LINE_WORDS),
        ("NODE_ZOOM", beam::NODE_ZOOM),
        ("NODE_CORNER", beam::NODE_CORNER),
        ("NODE_ROOTS", beam::NODE_ROOTS),
        ("SAMPLE_NODE", beam::SAMPLE_NODE),
        ("SAMPLE_ASSET", beam::SAMPLE_ASSET),
        ("SAMPLE_LACKS", beam::SAMPLE_LACKS),
        ("SAMPLE_AXIS", beam::SAMPLE_AXIS),
        ("SAMPLE_VISITS", beam::SAMPLE_VISITS),
        ("SAMPLE_WORDS", beam::SAMPLE_WORDS),
        ("SAMPLE_NONE", beam::SAMPLE_NONE),
        ("ENTRIES", pyramid::ENTRIES),
        ("ENTRIES_AT", pyramid::ENTRIES_AT),
        ("ENTRY_WORDS", pyramid::ENTRY_WORDS),
        ("BUCKET", pyramid::BUCKET),
        ("W_KIND", pyramid::W_KIND),
        ("W_ASSET", pyramid::W_ASSET),
        ("W_NODE", pyramid::W_NODE),
        ("NONE", pyramid::NONE),
        ("CLAIMED", pyramid::CLAIMED),
        ("TAG_BITS", pyramid::TAG_BITS),
        ("KEY_ZOOMS", !((1u32 << pyramid::TAG_BITS) - 1)),
        ("KIND_SPLIT", answer::Kind::Split as u32),
        ("KIND_CELL", answer::Kind::Cell as u32),
        ("VAL_MASK", answer::PLACED_ASSET - 1),
        ("VALUE_VOID", value::VOID),
        ("VALUE_AIR", value::AIR),
        ("FILL_SHIFT", answer::FILL_SHIFT),
        ("EIGHTHS", answer::EIGHTHS),
        ("BOX_BITS", answer::BOX_BITS),
        ("BOX_STEPS", answer::BOX_STEPS),
        ("STEPS_MOST", 4096),
        ("JITTER", eye::JITTER as u32),
        ("Y_START", pxl_engine::cast::Y_START),
        ("Z_START", pxl_engine::cast::Z_START),
        ("SHARD_ZOOM", pxl_engine::SHARD_ZOOM),
        ("SHARD_UNKNOWN", pxl_engine::area::Shard::UNKNOWN.word()),
        ("LOOKUP_NONE", pxl_engine::clipmap::Lookup::NONE.word()),
        (
            "SLOT_NOTHING",
            pxl_engine::overwrite::NodeSlot::NOTHING.word(),
        ),
        ("LINE_INHERITANCE", beam::LINE_INHERITANCE),
        ("LINE_STEP", beam::LINE_STEP),
        ("W_INHERITANCE", pyramid::W_INHERITANCE),
        ("MISS_ACROSS", ray::MISS_ACROSS),
        ("MISS_LANES", ray::MISS_LANES),
        ("MARKS", pyramid::MARKS),
        ("MARKS_AT", pyramid::MARKS_AT),
        ("MARKS_CNT", pyramid::MARKS_CNT),
        ("HITS", beam::HITS),
        ("HIT_LANES", beam::HIT_LANES),
        ("HIT_WORDS", beam::HIT_WORDS),
        ("HIT_NODE", beam::HIT_NODE),
        ("HIT_FACE", beam::HIT_FACE),
        ("HIT_PX", beam::HIT_PX),
        ("HIT_WAITS", beam::HIT_WAITS),
        ("FACE_BOX", face::BOX),
        ("RAY_FACE_SHIFT", ray::FACE_SHIFT),
        ("RAY_ZOOM_SHIFT", ray::ZOOM_SHIFT),
        ("RAY_FILL_SHIFT", ray::FILL_SHIFT),
    ];
    let mut out = format!(
        "const ZOOM_SPLIT: f32 = {:?};\nconst NARROW: f32 = {:?};\n",
        grid::CELLS_A_RAD / grid::SPANS,
        pxl_engine::cast::NARROW,
    );
    for (name, v) in words {
        out += &format!("const {name}: u32 = {v}u;\n");
    }
    out
}

fn tile_word() -> String {
    // The Rust's rule, as the WGSL below spells it: a u32's difference that
    // wraps, a claim that ends at `claimed_for`.
    assert_eq!(screen::age(0, u32::MAX), 1);
    assert_eq!(screen::age(5, 5), 0);
    assert!(screen::claimed(0, 1) && !screen::claimed(1, 1));
    assert!(screen::claimed(u32::MAX - 1, u32::MAX) && !screen::claimed(u32::MAX, u32::MAX));
    concat!(
        "struct Tiles { count: vec4u, size: vec4f, };\n",
        "fn tiles_age(t: Tiles, changed_at: u32) -> u32 { return t.count.w - changed_at; }\n",
        "fn tiles_claimed(t: Tiles, changed_at: u32) -> bool { return tiles_age(t, changed_at) < t.count.z; }\n",
    )
    .to_string()
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
        format!("const AIR_FROXELS: u32 = {AIR_FROXELS}u;\n"),
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
        let generated = parts.len() == 1 && parts[0].ends_with(".gen");
        let mut text = if generated {
            String::new()
        } else {
            constant.clone() + "\n"
        };
        for part in parts.iter() {
            text += &read(part);
            text += "\n";
        }
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
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        let file = format!("{kind}.wgsl");
        std::fs::write(out.join("assembled").join(&file), &text).expect("an assembled shader");
        sources += &format!("        {kind} => include_str!(concat!(env!(\"OUT_DIR\"), \"/shader/assembled/{file}\")),\n");
        // Each binding and the stages whose entry points read it.
        let mut read_by: Vec<(u32, u32, Vec<&str>)> = Vec::new();
        for (handle, var) in module.global_variables.iter() {
            let Some(b) = var.binding else { continue };
            let mut by = Vec::new();
            for (i, entry) in module.entry_points.iter().enumerate() {
                let stage = match entry.stage {
                    naga::ShaderStage::Vertex => "VERTEX",
                    naga::ShaderStage::Fragment => "FRAGMENT",
                    _ => "COMPUTE",
                };
                if !info.get_entry_point(i)[handle].is_empty() && !by.contains(&stage) {
                    by.push(stage);
                }
            }
            read_by.push((b.group, b.binding, by));
        }
        read_by.sort();
        let entries: Vec<String> = read_by
            .iter()
            .map(|(g, b, by)| {
                format!(
                    "[{g}, {b}, {}]",
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
    let root = out.parent().expect("OUT_DIR");
    std::fs::write(
        root.join("sources.rs"),
        format!("/// Shader `kind` as build.rs assembled and validated it (part.rs).\npub fn src(kind: u32) -> &'static str {{\n    match kind {{\n{sources}        _ => \"\",\n    }}\n}}\n"),
    )
    .expect("the shaders' sources");
    std::fs::write(
        root.join("stages.rs"),
        format!("/// Each shader's bindings, [group, binding, the stages that read it], read off it by build.rs.\npub const STAGES: [&[[u32; 3]]; SHADERS as usize] = [\n{stages}];\n"),
    )
    .expect("the shaders' stages");
}
