// pxl-tool/bin/check_rules.rs

//! Holds the tree to what a reader cannot see and the code must keep: no raw
//! pointer casts in offset crates, each rounding half to even where clippy
//! does not see it, no CommonJS file, the layers' order, where nothing names
//! what is above it, across a crate's folders and inside the kernel's heads',
//! the camera's and the view's, the profiler named by the top alone, no renderer under the
//! kernel's sys/, the layers read by the query alone, the screen's pixels read
//! only at the screen, and no Rust file outside a crate's module tree. A rule Rust alone keeps is clippy's
//! (rust/clippy.toml, xpute/rust/clippy.toml).
//!
//!   cargo check-rules            (in rust/; make check-rules from the root)

use std::process::{Command, ExitCode};

use pxl_tool::root;
use regex::Regex;

/// The crates that address the core memory by offset, where an offset becomes
/// a pointer only through `xpute_core::mem::at` or `Memory::at`.
const OFF_CRATES: [&str; 4] = [
    "xpute/rust/core/src/",
    "xpute/rust/runtime/src/",
    "rust/client/kernel/src/",
    "rust/client/render/src/",
];

/// One rounding: half to even (xpute ARCHITECTURE.md, `math/scalar`). A call
/// that rounds otherwise, in TypeScript, WGSL or C++, or a floor of a half
/// added in any of them or in Rust, is refused in pixelet's code and xpute's;
/// Rust's calls are clippy's (`disallowed-methods`). xpute's scalar keeps its
/// named variants, round_half_up and round_half_away, as options a library
/// offers.
const ROUNDING_ROOTS: [&str; 3] = ["rust/", "ts/", "xpute/"];
const ROUNDING_VARIANT_FILES: [&str; 4] = [
    "xpute/ts/kit/src/math/scalar.ts",
    "xpute/ts/kit/src/math/scalar.test.ts",
    "xpute/cpp/xpute/kit/math/scalar.hpp",
    "xpute/cpp/xpute/kit/math/scalar.test.cpp",
];
const NOT_EVEN: [&str; 4] = [
    r"\bMath\.round\(",
    r"\bstd::round\b",
    // A number's text rounds its last place half up: `fixed` (ts/client shell/fixed.ts) rounds it half to even.
    r"\.toFixed\(",
    r"\.toPrecision\(",
];
/// A half added inside a floor's own parentheses, or before `.floor()`: no
/// call clippy can name, so Rust's are refused here too.
const HALF_FLOORED: [&str; 2] = [
    r"\bfloor\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\+\s*0\.5\s*\)",
    r"\([^()]*\+\s*0\.5\s*\)\.floor\(\)",
];

/// One order of dependency over a folder's modules: a module names only
/// what is at or below it, and neither head the other; a module beside the
/// order names anything, and only the order's top names it. `root` is where
/// the folder's files are, `path` what its modules are named by, a `domain::`
/// step past it passed over; a name past `path` no module holds is the
/// folder's root's own, which stands at `own`, or, where none, is not read.
struct Order {
    root: &'static str,
    path: &'static str,
    levels: &'static [(&'static str, u8)],
    heads: &'static [&'static str],
    beside: &'static [&'static str],
    own: Option<u8>,
}

/// The kernel's order (ARCHITECTURE.md, "The kernel is one order of
/// dependency"): a module names only what is at or below it, and neither
/// head the other. The top wires everything, the shell's protocol among it;
/// the protocol's enums the domain reads, and an edit not yet made, which
/// the top hands from control to the scene, stand where both heads may name
/// them. The machine it asks of its runtime and the device transport's
/// decoder, which the device's app calls, are the bottom, naming none of the
/// kernel. The profiler, reading anything, stands beside it, in no step of
/// it, and only the top, which records into it, names it.
const KERNEL: &str = "rust/client/kernel/src/";
const KERNEL_ORDER: [(&str, u8); 16] = [
    ("turn", 7),
    ("door", 7),
    ("shell", 7),
    ("pointer", 7),
    ("scene", 6),
    ("control", 6),
    ("smooth", 5),
    ("param", 5),
    ("cmd", 5),
    ("preview", 5),
    ("view", 4),
    ("query", 3),
    ("layer", 2),
    ("sys", 1),
    ("machine", 0),
    ("unpack", 0),
];
const KERNEL_HEADS: [&str; 2] = ["scene", "control"];
const KERNEL_BESIDE: [&str; 1] = ["perf"];

/// Control's own order (domain/control.rs): the pointer's edits and its
/// gestures, then navigation, the camera, and under them the sensors and
/// where the pivot stands.
const CONTROL_ORDER: [(&str, u8); 7] = [
    ("edit", 4),
    ("input", 4),
    ("navigation", 3),
    ("camera", 2),
    ("sensor", 1),
    ("support", 1),
    ("param", 0),
];

/// The camera's own order (domain/control/camera.rs): the camera itself,
/// the folder's root, under everything, beside a gesture's anchor; what the
/// controls ask over them; the orbit and the first-person camera, and the
/// tests' fixture that places them; and over them which controller moves it.
const CAMERA_ORDER: [(&str, u8); 6] = [
    ("controller", 3),
    ("orbit", 2),
    ("first_person", 2),
    ("fixture", 2),
    ("state", 1),
    ("gesture", 0),
];

/// The view's own order (domain/view.rs): demand and the lens over the
/// observation, over the ground under a point and the frame, over the
/// settings.
const VIEW_ORDER: [(&str, u8); 7] = [
    ("demand", 3),
    ("lens", 3),
    ("observation", 2),
    ("above_ground", 1),
    ("frame", 1),
    ("setting", 0),
    ("param", 0),
];

/// The scene's own order (domain/scene.rs): the stage, the frame's top, over
/// the root, which sequences the parts that draw (`SCENE_OWN`), over the
/// markers, the characters and the indicators, over what they draw with,
/// each part keeping its own state.
const SCENE_ORDER: [(&str, u8); 15] = [
    ("stage", 7),
    ("marker", 5),
    ("character", 4),
    ("indicator", 4),
    ("environment", 3),
    ("gizmo", 3),
    ("decor", 2),
    ("model", 1),
    ("animation", 0),
    ("cast", 0),
    ("column", 0),
    ("frame", 0),
    ("param", 0),
    ("resolution", 0),
    ("transform", 0),
];
const SCENE_OWN: u8 = 6;

/// pxl-engine's order: what every part speaks, the layers' storage and
/// the world's fields, the query over them (ARCHITECTURE.md, "The query, the
/// only seam"), and what asks it. A layer knows no query, and the query no
/// asker.
const ENGINE: &str = "rust/spec/engine/src/";
const ENGINE_ORDER: [(&str, u8); 26] = [
    ("param", 0),
    ("seed", 0),
    ("content", 0),
    ("value", 0),
    ("lattice", 0),
    ("grid", 0),
    ("answer", 0),
    ("count", 0),
    ("area", 1),
    ("ground", 1),
    ("clipmap", 1),
    ("way", 1),
    ("fill", 1),
    ("overwrite", 1),
    ("world", 2),
    ("cast", 2),
    ("support", 2),
    ("node_cache", 2),
    ("beam", 3),
    ("eye", 3),
    ("raster", 3),
    ("record", 3),
    ("ray", 3),
    ("light", 3),
    ("screen", 3),
    ("face", 3),
];

/// pxl-content's order: its parameters, its palette and the one rule a
/// count is handed down by, its rules and fields, then the decor's generation
/// over its scatter.
const CONTENT: &str = "rust/spec/content/src/";
const CONTENT_ORDER: [(&str, u8); 9] = [
    ("param", 0),
    ("share", 0),
    ("cube", 0),
    ("decor", 0),
    ("cave", 1),
    ("strata", 1),
    ("cloud", 1),
    ("scatter", 1),
    ("generation", 2),
];

/// Every order held. A folder whose root holds only its modules gives its
/// root's own a level over all of them, so no part reaches back to it.
const ORDERS: [Order; 7] = [
    Order {
        root: KERNEL,
        path: "crate",
        levels: &KERNEL_ORDER,
        heads: &KERNEL_HEADS,
        beside: &KERNEL_BESIDE,
        own: None,
    },
    Order {
        root: ENGINE,
        path: "crate",
        levels: &ENGINE_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: CONTENT,
        path: "crate",
        levels: &CONTENT_ORDER,
        heads: &[],
        beside: &[],
        own: None,
    },
    Order {
        root: "rust/client/kernel/src/domain/control/",
        path: "crate::domain::control",
        levels: &CONTROL_ORDER,
        heads: &[],
        beside: &[],
        own: Some(5),
    },
    Order {
        root: "rust/client/kernel/src/domain/control/camera/",
        path: "crate::domain::control::camera",
        levels: &CAMERA_ORDER,
        heads: &[],
        beside: &[],
        own: Some(0),
    },
    Order {
        root: "rust/client/kernel/src/domain/view/",
        path: "crate::domain::view",
        levels: &VIEW_ORDER,
        heads: &[],
        beside: &[],
        own: Some(4),
    },
    Order {
        root: "rust/client/kernel/src/domain/scene/",
        path: "crate::domain::scene",
        levels: &SCENE_ORDER,
        heads: &[],
        beside: &[],
        own: Some(SCENE_OWN),
    },
];

/// The kernel's sys/ stands below everything that draws: it names no
/// renderer (`pxl_render`), and, beside its order, no profiler
/// (`KERNEL_BESIDE`).
const SYS: &str = "rust/client/kernel/src/sys/";
const SYS_REFUSED: &str = r"\bpxl_render\b";

/// What the world shows is decided in angles, the camera grid's cell; a
/// pixel enters only at the screen (ARCHITECTURE.md, "The camera grid").
/// The kernel reads the view's CSS pixels through one module, the screen
/// (domain/view/frame/screen.rs), and only these name it: the view's own
/// frame, the camera's gestures, the markers' layout and selection, the
/// stage's viewport words and the shell. A path is a file, or a folder by
/// its trailing slash.
const SCREEN_USERS: [&str; 7] = [
    "rust/client/kernel/src/domain/view/frame/",
    "rust/client/kernel/src/domain/control/camera/controller.rs",
    "rust/client/kernel/src/domain/control/input.rs",
    "rust/client/kernel/src/domain/scene/marker/arrangement.rs",
    "rust/client/kernel/src/domain/scene/marker/selection.rs",
    "rust/client/kernel/src/domain/scene/stage.rs",
    "rust/client/kernel/src/shell.rs",
];

/// The lines of `text`, a kernel source, comments left out, that reach the
/// screen: a path through it (`screen::`), a group naming it (`frame::{…
/// screen …}`, over as many lines as it runs), a glob or an alias of the
/// frame that would reach it unnamed, or a re-export of it, which would lend
/// it to a module the list does not name.
fn screen_reached(text: &str) -> Vec<usize> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let reach = Regex::new(
        r"\bscreen::|\bframe::\{[^}]*\bscreen\b|\bframe::\*|\bframe\s+as\b|\bpub\s+use\b[^;]*\bscreen\b",
    )
    .expect("the screen's pattern");
    let mut lines: Vec<usize> = reach
        .find_iter(&code)
        .map(|m| code[..m.start()].matches('\n').count() + 1)
        .collect();
    lines.dedup();
    lines
}

/// Only the world's query reads a layer (ARCHITECTURE.md, "The query, the
/// only seam"): the layers' reads, the form both answer (`Query`), the
/// terrain's and the store's past it (`TerrainQuery`, `ObjectQuery`), the clip
/// levels as the CPU reads them (`CpuClipmap`) and the ways' pool, are named
/// in the kernel by the layers and the query alone, and the query lends none
/// of them on. A path is a file, or a folder by its trailing slash.
const LAYERS: [&str; 2] = [
    "rust/client/kernel/src/domain/layer.rs",
    "rust/client/kernel/src/domain/layer/",
];
const LAYER_READERS: [&str; 3] = [
    "rust/client/kernel/src/domain/query.rs",
    "rust/client/kernel/src/domain/query/",
    "rust/client/kernel/src/domain/query.test.rs",
];
const LAYER_READS: &str =
    r"\b(?:Query|TerrainQuery|ObjectQuery|CpuClipmap)\b|\bclipmap::(?:\{[^}]*\bpool\b|pool\b)";

/// The lines of `text`, a kernel source, comments left out, that name a
/// layer's read (`LAYER_READS`), or glob a module that would bring one in
/// unnamed: where `lent`, only a re-export of one past the query.
fn layer_read_named(text: &str, lent: bool) -> Vec<usize> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let pattern = match lent {
        false => format!(r"{LAYER_READS}|\b(?:layer|terrain|clipmap|store)::\*"),
        true => format!(
            r"\bpub(?:\s*\(\s*(?:crate|in\s+crate::domain)\s*\))?\s+use\b[^;]*(?:{LAYER_READS}|::\*)"
        ),
    };
    let reach = Regex::new(&pattern).expect("the layers' reads' pattern");
    let mut lines: Vec<usize> = reach
        .find_iter(&code)
        .map(|m| code[..m.start()].matches('\n').count() + 1)
        .collect();
    lines.dedup();
    lines
}

/// xpute's words are runtime, core and their contract (AGENTS.md): pixelet's
/// appear nowhere under it.
const XPUTE: &str = "xpute/";
const PIXELET_WORDS: &str = r"(?i)\b(pixelet|kernel|shell)\b";

/// The modules `text` names past `path`, a `domain::` step past it passed
/// over (the kernel's heads and trunk live under it), a group's every first
/// name among them, each with the line it is named on; `path` itself, named
/// whole, is its root, an empty name.
fn named(text: &str, path: &str) -> Vec<(usize, String)> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let ident = |s: &str| -> String { s.chars().take_while(|&c| word(c)).collect() };
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = code[at..].find(path) {
        let start = at + k;
        at = start + path.len();
        let rest = &code[at..];
        // A longer name that holds `path`, as `pub(crate)` holds none.
        if rest.starts_with(word) || code[..start].ends_with(word) {
            continue;
        }
        let line = code[..start].matches('\n').count() + 1;
        let Some(rest) = rest.strip_prefix("::") else {
            out.push((line, String::new()));
            continue;
        };
        let rest = rest.strip_prefix("domain::").unwrap_or(rest);
        if let Some(group) = rest.strip_prefix('{') {
            // A group: the first name of each item at its own depth.
            let (mut depth, mut item_start) = (0, true);
            for (i, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    ',' if depth == 0 => item_start = true,
                    c if item_start && word(c) => {
                        out.push((line, ident(&group[i..])));
                        item_start = false;
                    }
                    _ => {}
                }
            }
        } else {
            out.push((line, ident(rest)));
        }
    }
    out
}

/// Where `f` stands in `order` under `root`: its first step past `root`, or
/// past `domain/` for the kernel's heads and trunk.
fn standing(f: &str, root: &str) -> String {
    let rest = &f[root.len()..];
    let rest = rest.strip_prefix("domain/").unwrap_or(rest);
    rest.split('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Every name in `o`'s files of a module above the file's own, or of one
/// head by another, or of one beside the order off its top; and every module
/// the order does not place, but the crate's root and those that stand
/// beside it, so a new one is placed before it is checked.
fn out_of_order(files: &[&str], read: &dyn Fn(&str) -> Option<String>, o: &Order) -> Vec<String> {
    let level = |m: &str| o.levels.iter().find(|(n, _)| *n == m).map(|(_, l)| *l);
    let top = o.levels.iter().map(|&(_, l)| l).max().unwrap_or(0);
    let mut problems = Vec::new();
    let mut unplaced = std::collections::BTreeSet::new();
    for f in files
        .iter()
        .filter(|f| f.starts_with(o.root) && f.ends_with(".rs"))
    {
        let me = standing(f, o.root);
        let Some(mine) = level(&me) else {
            if !["lib", "domain"].contains(&me.as_str()) && !o.beside.contains(&me.as_str()) {
                unplaced.insert(me);
            }
            continue;
        };
        for (line, them) in named(&read(f).unwrap_or_default(), o.path) {
            if them == me {
                continue;
            }
            if o.beside.contains(&them.as_str()) {
                if mine < top {
                    problems.push(format!(
                        "{f}:{line}: `{me}` names `{them}`, which stands beside the order and only its top names"
                    ));
                }
                continue;
            }
            let Some(theirs) = level(&them).or(o.own) else {
                continue;
            };
            let head = o.heads.contains(&me.as_str()) && o.heads.contains(&them.as_str());
            if theirs > mine || head {
                let them = if level(&them).is_some() {
                    format!("`{them}`")
                } else {
                    format!("its folder's root (`{}`)", o.path)
                };
                problems.push(format!(
                    "{f}:{line}: `{me}` names {them}, above it or the other head, in the layers' order"
                ));
            }
        }
    }
    for me in unplaced {
        problems.push(format!(
            "{}{me}: `{me}` has no place in the layers' order; give it one (check_rules.rs)",
            o.root
        ));
    }
    problems
}

/// The lines of `text`, a sys/ source, comments left out, that name what
/// sys/ stands below (`SYS_REFUSED`).
fn sys_refused(text: &str) -> Vec<usize> {
    let refused = Regex::new(SYS_REFUSED).expect("sys's refused pattern");
    text.lines()
        .enumerate()
        .filter(|(_, l)| refused.is_match(l.split("//").next().unwrap_or("")))
        .map(|(n, _)| n + 1)
        .collect()
}

/// The tree's files, a path a line from `root`: git's, tracked and not
/// ignored, where `root` is a checkout's own top; else, as in an export
/// laid out under a checkout's ignored target,
/// every file under it but the builds' and the packages', which git there
/// would list none of. Outside any checkout, an error.
/// Each tracked `.rs` that no crate reaches: none of its own crate's roots
/// (`lib.rs`, `main.rs`, `build.rs`, a file under `src/bin/`, `tests/`,
/// `benches/` or `examples/`), and no `mod`, `#[path]` or `include!` in a
/// file, nor a manifest's `path`, names it. Such a file compiles in nothing,
/// so nothing checks it.
fn unlinked(files: &[&str], read: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    use std::collections::BTreeSet;
    use std::path::{Component, Path, PathBuf};
    fn clean(p: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                c => out.push(c),
            }
        }
        out
    }
    let rust =
        |f: &&&str| f.ends_with(".rs") && (f.starts_with("rust/") || f.starts_with("xpute/rust/"));
    let module = Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;")
        .expect("a module's pattern");
    let path_attr = Regex::new(r#"#\[path\s*=\s*"([^"]+)"\]"#).expect("a path's pattern");
    let include = Regex::new(r#"include!\(\s*"([^"]+\.rs)"\s*\)"#).expect("an include's pattern");
    let manifest_path =
        Regex::new(r#"(?m)^\s*path\s*=\s*"([^"]+\.rs)""#).expect("a manifest path's pattern");
    let mut reached: BTreeSet<PathBuf> = BTreeSet::new();
    for f in files.iter().filter(|f| f.ends_with("Cargo.toml")) {
        let dir = Path::new(f).parent().unwrap_or(Path::new(""));
        for c in manifest_path.captures_iter(&read(f).unwrap_or_default()) {
            reached.insert(clean(&dir.join(&c[1])));
        }
    }
    for f in files.iter().filter(rust) {
        let file = Path::new(f);
        let dir = file.parent().unwrap_or(Path::new(""));
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // A `mod` in a crate root or a `mod.rs` names files beside it; in any
        // other file, files in the folder named as that file is.
        let children = if ["lib", "main", "mod", "build"].contains(&stem) {
            dir.to_path_buf()
        } else {
            dir.join(stem)
        };
        let text = read(f).unwrap_or_default();
        for c in path_attr.captures_iter(&text) {
            reached.insert(clean(&dir.join(&c[1])));
        }
        for c in include.captures_iter(&text) {
            reached.insert(clean(&dir.join(&c[1])));
        }
        for c in module.captures_iter(&text) {
            reached.insert(children.join(format!("{}.rs", &c[1])));
            reached.insert(children.join(&c[1]).join("mod.rs"));
        }
    }
    files
        .iter()
        .filter(rust)
        .filter(|f| {
            let p = Path::new(f);
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let rooted = ["lib.rs", "main.rs", "build.rs"].contains(&name)
                || ["/src/bin/", "/tests/", "/benches/", "/examples/"]
                    .iter()
                    .any(|d| f.contains(d));
            !rooted && !reached.contains(p)
        })
        .map(|f| format!("{f}: no crate reaches it: no `mod`, `#[path]`, `include!` or manifest `path` names it, so it compiles in nothing"))
        .collect()
}

fn list_files(root: &std::path::Path) -> Result<String, String> {
    let top = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !top.status.success() {
        return Err("git rev-parse: not in a checkout".into());
    }
    let top = String::from_utf8(top.stdout).map_err(|e| format!("git rev-parse: {e}"))?;
    let own = std::path::Path::new(top.trim())
        .canonicalize()
        .is_ok_and(|t| root.canonicalize().is_ok_and(|r| r == t));
    if own {
        let out = Command::new("git")
            .args(["ls-files", "-co", "--exclude-standard"])
            .current_dir(root)
            .output()
            .map_err(|e| format!("git ls-files: {e}"))?;
        return String::from_utf8(out.stdout).map_err(|e| format!("git ls-files: {e}"));
    }
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            if ["target", "node_modules", ".git"]
                .iter()
                .any(|n| name == *n)
            {
                continue;
            }
            // A link is not followed, as git lists a link and not what it names.
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                walk(&p, root, out);
            } else if let Ok(rel) = p.strip_prefix(root) {
                out.push_str(&rel.to_string_lossy());
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(root, root, &mut out);
    Ok(out)
}

fn run() -> Result<Vec<String>, String> {
    let root = root();
    let listed = list_files(&root)?;
    // The tree as it stands: a file deleted and not yet committed is gone, so
    // a removal is checked before it is committed.
    let files: Vec<&str> = listed
        .lines()
        .filter(|f| !f.is_empty() && !f.contains("node_modules/") && root.join(f).is_file())
        .collect();
    let read = |f: &str| std::fs::read_to_string(root.join(f)).ok();
    let mut problems = Vec::new();

    let cast = Regex::new(r"\bas\s+\*(const|mut)\b").expect("the cast's pattern");
    let comment = Regex::new(r"^\s*//").expect("the comment's pattern");
    for f in files
        .iter()
        .filter(|f| f.ends_with(".rs") && OFF_CRATES.iter().any(|c| f.starts_with(c)))
    {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if !comment.is_match(line) && cast.is_match(line) {
                problems.push(format!("{f}:{}: casts to a pointer where an offset goes through mem::at or Memory::at: {}", n + 1, line.trim()));
            }
        }
    }

    let patterns = |ps: &[&str]| -> Vec<Regex> {
        ps.iter()
            .map(|p| Regex::new(p).expect("a rounding's pattern"))
            .collect()
    };
    let (not_even, half_floored) = (patterns(&NOT_EVEN), patterns(&HALF_FLOORED));
    let rounded = Regex::new(r"\.(rs|tsx?|wgsl|hpp|cpp)$").expect("the extensions' pattern");
    for f in files.iter().filter(|f| {
        ROUNDING_ROOTS.iter().any(|r| f.starts_with(r))
            && !ROUNDING_VARIANT_FILES.contains(f)
            && rounded.is_match(f)
    }) {
        let calls = !f.ends_with(".rs");
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if (calls && not_even.iter().any(|r| r.is_match(line)))
                || half_floored.iter().any(|r| r.is_match(line))
            {
                problems.push(format!(
                    "{f}:{}: rounds other than half to even: {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    // The tree's scripts are ES modules, TypeScript where a loader takes it,
    // the bundlers' configs among them: Node runs only the native toolchain's
    // own scripts (CocoaPods, Xcode's and Gradle's phases).
    for f in files
        .iter()
        .filter(|f| f.ends_with(".cjs") || f.ends_with(".cts"))
    {
        problems.push(format!("{f}: CommonJS; the tree's scripts are ES modules"));
    }

    for o in &ORDERS {
        problems.extend(out_of_order(&files, &read, o));
    }
    for f in files
        .iter()
        .filter(|f| f.starts_with(SYS) && f.ends_with(".rs"))
    {
        for line in sys_refused(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: names the renderer under sys/, which stands below everything that draws (check_rules.rs `SYS_REFUSED`)"
            ));
        }
    }
    for f in files.iter().filter(|f| {
        f.starts_with(KERNEL)
            && f.ends_with(".rs")
            && !SCREEN_USERS.iter().any(|u| f.starts_with(u))
    }) {
        for line in screen_reached(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: reaches the screen's pixels where the world is decided in angles; only SCREEN_USERS may (check_rules.rs)"
            ));
        }
    }
    for f in files
        .iter()
        .filter(|f| f.starts_with(KERNEL) && f.ends_with(".rs"))
    {
        if LAYERS.iter().any(|l| f.starts_with(l)) {
            continue;
        }
        let query = LAYER_READERS.iter().any(|r| f.starts_with(r));
        for line in layer_read_named(&read(f).unwrap_or_default(), query) {
            problems.push(match query {
                false => format!(
                    "{f}:{line}: reads a layer where every reader asks the world's query; only the query may (check_rules.rs `LAYER_READERS`)"
                ),
                true => format!(
                    "{f}:{line}: lends a layer's read on from the query, which answers only as the world (check_rules.rs)"
                ),
            });
        }
    }
    let pixelet = Regex::new(PIXELET_WORDS).expect("pixelet's words' pattern");
    for f in files.iter().filter(|f| f.starts_with(XPUTE)) {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            // Make's `$(shell …)` is the build tool's, not pixelet's shell.
            if pixelet.is_match(&line.replace("$(shell", "")) {
                problems.push(format!(
                    "{f}:{}: names pixelet's word under xpute: {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    problems.extend(unlinked(&files, &read));
    Ok(problems)
}

fn main() -> ExitCode {
    match run() {
        Ok(problems) if problems.is_empty() => {
            println!("check-rules: the memory's offsets, one rounding, ES modules, the layers' order, the profiler beside it, sys/ under the renderer, the query as the one seam, the screen's pixels and the crates' module trees hold");
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            eprintln!("{}\n\ncheck-rules: {}", problems.join("\n"), problems.len());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("[check-rules] {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlinked_file_named() {
        let files = [
            "rust/a/src/lib.rs",
            "rust/a/src/one.rs",
            "rust/a/src/one/two.rs",
            "rust/a/src/gen.spec.rs",
            "rust/a/src/stray.rs",
            "rust/a/Cargo.toml",
            "rust/a/src/test_bin.rs",
        ];
        let read = |f: &str| {
            Some(
                match f {
                    "rust/a/src/lib.rs" => {
                        "mod one;
#[path = \"gen.spec.rs\"]
mod spec;"
                    }
                    "rust/a/src/one.rs" => "pub(crate) mod two;",
                    "rust/a/Cargo.toml" => "[[test]]\npath = \"src/test_bin.rs\"",
                    _ => "",
                }
                .to_string(),
            )
        };
        let found = unlinked(&files, &read);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("rust/a/src/stray.rs:"));
    }

    #[test]
    fn order_refuses_up_beside_and_root() {
        const LEVELS: [(&str, u8); 4] = [("top", 2), ("mid", 1), ("f", 1), ("low", 0)];
        let crate_order = Order {
            root: "rust/k/src/",
            path: "crate",
            levels: &LEVELS,
            heads: &[],
            beside: &["perf"],
            own: None,
        };
        const PARTS: [(&str, u8); 2] = [("a", 1), ("b", 0)];
        let folder = Order {
            root: "rust/k/src/domain/f/",
            path: "crate::domain::f",
            levels: &PARTS,
            heads: &[],
            beside: &[],
            own: Some(2),
        };
        let files = [
            "rust/k/src/top.rs",
            "rust/k/src/mid.rs",
            "rust/k/src/low.rs",
            "rust/k/src/perf.rs",
            "rust/k/src/domain/f/a.rs",
            "rust/k/src/domain/f/b.rs",
        ];
        let read = |f: &str| {
            Some(
                match f {
                    "rust/k/src/top.rs" => "use crate::perf::x;\nuse crate::low;",
                    "rust/k/src/mid.rs" => "pub(crate) fn f() {}\nuse crate::perf::y;",
                    "rust/k/src/low.rs" => "use crate::{mid::z, low};",
                    "rust/k/src/perf.rs" => "use crate::top;",
                    "rust/k/src/domain/f/a.rs" => "use crate::domain::f::{b::x, sequence};",
                    "rust/k/src/domain/f/b.rs" => {
                        "use crate::domain::f::sequence;\nuse crate::domain::f;\nuse crate::domain::fx;"
                    }
                    _ => "",
                }
                .to_string(),
            )
        };
        let found = out_of_order(&files, &read, &crate_order);
        let at = |found: &[String]| -> Vec<String> {
            found
                .iter()
                .map(|p| p.split(": ").next().unwrap_or("").to_string())
                .collect()
        };
        assert_eq!(
            at(&found),
            ["rust/k/src/mid.rs:2", "rust/k/src/low.rs:1"],
            "{found:?}"
        );
        let found = out_of_order(&files, &read, &folder);
        assert_eq!(
            at(&found),
            [
                "rust/k/src/domain/f/a.rs:1",
                "rust/k/src/domain/f/b.rs:1",
                "rust/k/src/domain/f/b.rs:2"
            ],
            "{found:?}"
        );
    }

    #[test]
    fn sys_refuses_renderer() {
        assert_eq!(
            sys_refused("use pxl_spec::x;\nuse pxl_render::gpu::text::Raster;\n// pxl_render"),
            vec![2]
        );
    }

    #[test]
    fn layer_read_any_name() {
        for (text, at) in [
            ("use crate::domain::layer::Query;", vec![1]),
            (
                "use crate::domain::layer::terrain::TerrainQuery as _;",
                vec![1],
            ),
            (
                "use crate::domain::layer::object::store::{\n    object,\n    ObjectQuery,\n};",
                vec![3],
            ),
            (
                "use crate::domain::layer::terrain::clipmap::{pool, CpuClipmap};",
                vec![1],
            ),
            ("let w = clipmap::pool();", vec![1]),
            ("use crate::domain::layer::object::store::*;", vec![1]),
            ("use crate::domain::layer::*;", vec![1]),
        ] {
            assert_eq!(layer_read_named(text, false), at, "{text}");
        }
        for text in [
            "use crate::domain::query::World;",
            "use crate::domain::layer::object::store::{object, Entry};",
            "// the layer's Query, in a comment",
            "use pxl_content::generation::DecorQuery;",
        ] {
            assert!(layer_read_named(text, false).is_empty(), "{text}");
        }
        assert_eq!(
            layer_read_named("pub use crate::domain::layer::Query;", true),
            vec![1]
        );
        assert_eq!(
            layer_read_named(
                "pub(in crate::domain) use crate::domain::layer::Query;",
                true
            ),
            vec![1]
        );
        assert!(layer_read_named("use crate::domain::layer::Query;", true).is_empty());
        assert!(layer_read_named("pub(super) use crate::domain::layer::Query;", true).is_empty());
    }

    #[test]
    fn screen_reach_any_name() {
        for (text, at) in [
            ("use crate::domain::view::frame::screen::size;", vec![1]),
            ("use crate::domain::view::frame::{\n    pose::view,\n    screen::units_per_px_at,\n};", vec![1]),
            ("use crate::domain::view::frame::{pose, screen};", vec![1]),
            ("use crate::domain::view::frame::*;\nfn f() { size(); }", vec![1]),
            ("use crate::domain::view::frame as f;", vec![1]),
            ("pub use crate::domain::view::frame::screen::size as px;", vec![1]),
            ("let w = f::screen::size()[0];", vec![1]),
        ] {
            assert_eq!(screen_reached(text), at, "{text}");
        }
        for text in [
            "use crate::domain::view::frame::pose::view;",
            "// a comment naming frame::screen::size",
            "/// The screen as the eye sees it.",
            "let screen_drawn = 1;",
        ] {
            assert!(screen_reached(text).is_empty(), "{text}");
        }
    }
}
