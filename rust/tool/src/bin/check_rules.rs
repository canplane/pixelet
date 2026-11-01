// pxl-tool/bin/check_rules.rs

//! Holds the tree to what a reader cannot see and the code must keep: no raw
//! pointer casts in offset crates, each rounding half to even where clippy
//! does not see it, no CommonJS file, each generated shader made from the
//! sources the tree holds now, the layers' order, where nothing names what is
//! above it, the screen's pixels read only at the screen, each type's name a
//! head noun, never a bare participle, and no Rust file outside a crate's
//! module tree. A rule Rust alone keeps is clippy's (rust/clippy.toml,
//! xpute/rust/clippy.toml).
//!
//!   cargo check-rules            (in rust/; make check-rules from the root)

use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{inputs_digest, stale};
use regex::Regex;

/// The crates that address the core memory by offset, where an offset becomes
/// a pointer only through `xpute_guest::mem::at` or `Memory::at`.
const OFF_CRATES: [&str; 6] = [
    "xpute/rust/guest/src/",
    "xpute/rust/host/src/",
    "rust/client/kernel/src/",
    "rust/client/render/src/",
    "rust/client-bracket/wasm/src/",
    "rust/client-bracket/native/src/",
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

/// The kernel's order (ARCHITECTURE.md, "The kernel is one order of
/// dependency"): a module names only what is at or below it, and neither
/// head the other. The top wires everything; the profiler stands beside,
/// reading anything, and is in no step of it.
const KERNEL: &str = "rust/client/kernel/src/";
const KERNEL_ORDER: [(&str, u8); 12] = [
    ("turn", 6),
    ("door", 6),
    ("shell", 6),
    ("pointer", 6),
    ("scene", 5),
    ("control", 5),
    ("smooth", 4),
    ("param", 4),
    ("view", 3),
    ("query", 2),
    ("layer", 1),
    ("sys", 0),
];
const KERNEL_HEADS: [&str; 2] = ["scene", "control"];
const KERNEL_BESIDE: [&str; 1] = ["perf"];

/// pxl-spec-core's order: what every part speaks, the layers' storage and
/// the world's fields, the query over them (ARCHITECTURE.md, "The query, the
/// only seam"), and what asks it. A layer knows no query, and the query no
/// asker.
const SPEC_CORE: &str = "rust/spec/core/src/";
const SPEC_CORE_ORDER: [(&str, u8); 28] = [
    ("param", 0),
    ("cube", 0),
    ("decor", 0),
    ("lattice", 0),
    ("grid", 0),
    ("answer", 0),
    ("tally", 0),
    ("area", 1),
    ("cave", 1),
    ("scatter", 1),
    ("cloud", 1),
    ("ground", 1),
    ("clipmap", 1),
    ("way", 1),
    ("fill", 1),
    ("overwrite", 1),
    ("world", 2),
    ("cast", 2),
    ("support", 2),
    ("pyramid", 2),
    ("beam", 3),
    ("eye", 3),
    ("raster", 3),
    ("ray", 3),
    ("light", 3),
    ("screen", 3),
    ("generation", 3),
    ("face", 3),
];

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
    "rust/client/kernel/src/domain/control/camera/input.rs",
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

/// A type's name has a head noun saying what it holds; a bare past participle
/// names only an enum variant for a state or an event type (the naming
/// skill, "A type's head noun"). A struct, an enum, a type alias or a trait
/// in Rust and a struct in WGSL, under these roots, is held to it, its
/// generated WGSL by its source; and a type alias, an interface, a class or
/// an enum in the engine's TypeScript, the shell's UI left out. An enum's
/// variants are not.
const PARTICIPLE_ROOTS: [&str; 2] = ["rust/", "xpute/"];
const TS_PARTICIPLE_ROOTS: [&str; 2] = ["xpute/ts/", "ts/client/src/"];
const TS_UNCHECKED_ROOT: &str = "ts/client/src/shell/";
/// The irregular past participles a type's name may not be alone. A word
/// that is also a noun (`Hit`, `Set`, `Read`, `Run`, `Cut`, `Split`) is not
/// among them.
const IRREGULAR_PARTICIPLES: [&str; 84] = [
    "Held",
    "Kept",
    "Seen",
    "Laid",
    "Built",
    "Shown",
    "Drawn",
    "Found",
    "Taken",
    "Stood",
    "Bound",
    "Made",
    "Left",
    "Lent",
    "Sent",
    "Told",
    "Met",
    "Struck",
    "Given",
    "Gone",
    "Done",
    "Known",
    "Grown",
    "Thrown",
    "Hidden",
    "Written",
    "Ridden",
    "Driven",
    "Risen",
    "Fallen",
    "Eaten",
    "Beaten",
    "Shaken",
    "Chosen",
    "Broken",
    "Frozen",
    "Spoken",
    "Stolen",
    "Woken",
    "Worn",
    "Torn",
    "Born",
    "Borne",
    "Flown",
    "Blown",
    "Sewn",
    "Sown",
    "Begun",
    "Sung",
    "Rung",
    "Sunk",
    "Swum",
    "Stuck",
    "Spun",
    "Caught",
    "Taught",
    "Bought",
    "Brought",
    "Fought",
    "Sought",
    "Lit",
    "Hid",
    "Slid",
    "Bent",
    "Spent",
    "Swept",
    "Slept",
    "Dealt",
    "Meant",
    "Sold",
    "Paid",
    "Said",
    "Heard",
    "Fed",
    "Led",
    "Fled",
    "Lost",
    "Got",
    "Gotten",
    "Forgotten",
    "Forgiven",
    "Withheld",
    "Undone",
    "Overseen",
];
/// The words ending in -ed that are no participle.
const NOT_PARTICIPLES: [&str; 14] = [
    "Bed", "Bleed", "Breed", "Deed", "Embed", "Feed", "Hundred", "Need", "Red", "Reed", "Seed",
    "Shed", "Speed", "Weed",
];
/// The event types, each something that happened, which a bare participle
/// names: the file and the type.
const EVENTS: [(&str, &str); 2] = [
    // The host's answer to an open.
    ("rust/client/kernel/src/sys/io/file.rs", "Opened"),
    // How a stream's run ended.
    ("rust/client/kernel/src/sys/io/failure.rs", "Ended"),
];

/// Whether `name`, one capitalized word, is a past participle: ending in
/// -ed, or an irregular one.
fn participle(name: &str) -> bool {
    (name.ends_with("ed") && !NOT_PARTICIPLES.contains(&name))
        || IRREGULAR_PARTICIPLES.contains(&name)
}

/// Every type `text` names by a bare past participle, each with its line:
/// a Rust or WGSL source's structs, enums, type aliases and traits, or
/// where `ts`, a TypeScript source's type aliases, interfaces, classes and
/// enums.
fn participle_types(text: &str, ts: bool) -> Vec<(usize, String)> {
    let decl = Regex::new(if ts {
        r"(?m)^[ \t]*(?:export\s+)?(?:declare\s+)?(?:abstract\s+)?(?:const\s+)?(?:type|interface|class|enum)\s+([A-Z][a-z]*)(?:[^A-Za-z0-9_]|$)"
    } else {
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:struct|enum|type|trait)\s+([A-Z][a-z]*)(?:[^A-Za-z0-9_]|$)"
    })
    .expect("a type's pattern");
    decl.captures_iter(text)
        .filter_map(|c| c.get(1))
        .filter(|m| participle(m.as_str()))
        .map(|m| {
            (
                text[..m.start()].matches('\n').count() + 1,
                m.as_str().to_string(),
            )
        })
        .collect()
}

/// xpute's words are host, guest and their contract (AGENTS.md): pixelet's
/// appear nowhere under it.
const XPUTE: &str = "xpute/";
const PIXELET_WORDS: &str = r"(?i)\b(pixelet|kernel|shell)\b";

/// The modules `text` names through `crate::`, a `domain::` step passed
/// over (the kernel's heads and trunk live under it), a group's every
/// first name among them, each with the line it is named on.
fn crate_modules(text: &str) -> Vec<(usize, String)> {
    let code: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let ident = |s: &str| -> String {
        s.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    };
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = code[at..].find("crate::") {
        let start = at + k;
        let mut rest = &code[start + "crate::".len()..];
        if let Some(r) = rest.strip_prefix("domain::") {
            rest = r;
        }
        let line = code[..start].matches('\n').count() + 1;
        if let Some(group) = rest.strip_prefix('{') {
            // A group: the first name of each item at its own depth.
            let (mut depth, mut item_start) = (0, true);
            for (i, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    ',' if depth == 0 => item_start = true,
                    c if item_start && (c.is_alphanumeric() || c == '_') => {
                        out.push((line, ident(&group[i..])));
                        item_start = false;
                    }
                    _ => {}
                }
            }
        } else {
            out.push((line, ident(rest)));
        }
        at = start + "crate::".len();
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

/// Every name in `root`'s files of a module above the file's own in
/// `order`, or of one head by another; and every module `order` does not
/// place, but the crate's root and those that stand `beside` it, so a new
/// one is placed before it is checked.
fn out_of_order(
    files: &[&str],
    read: &dyn Fn(&str) -> Option<String>,
    root: &str,
    order: &[(&str, u8)],
    heads: &[&str],
    beside: &[&str],
) -> Vec<String> {
    let level = |m: &str| order.iter().find(|(n, _)| *n == m).map(|(_, l)| *l);
    let mut problems = Vec::new();
    let mut unplaced = std::collections::BTreeSet::new();
    for f in files
        .iter()
        .filter(|f| f.starts_with(root) && f.ends_with(".rs"))
    {
        let me = standing(f, root);
        let Some(mine) = level(&me) else {
            if !["lib", "domain"].contains(&me.as_str()) && !beside.contains(&me.as_str()) {
                unplaced.insert(me);
            }
            continue;
        };
        for (line, them) in crate_modules(&read(f).unwrap_or_default()) {
            let Some(theirs) = level(&them) else { continue };
            let head = heads.contains(&me.as_str()) && heads.contains(&them.as_str());
            if them != me && (theirs > mine || head) {
                problems.push(format!(
                    "{f}:{line}: `{me}` names `{them}`, above it or the other head, in the layers' order"
                ));
            }
        }
    }
    for me in unplaced {
        problems.push(format!(
            "{root}{me}: `{me}` has no place in the layers' order; give it one (check_rules.rs)"
        ));
    }
    problems
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
fn unlinked_files(files: &[&str], read: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
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
            && !f.ends_with(".gen.wgsl")
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

    problems.extend(out_of_order(
        &files,
        &read,
        KERNEL,
        &KERNEL_ORDER,
        &KERNEL_HEADS,
        &KERNEL_BESIDE,
    ));
    problems.extend(out_of_order(
        &files,
        &read,
        SPEC_CORE,
        &SPEC_CORE_ORDER,
        &[],
        &[],
    ));
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
    for f in files.iter() {
        let ts = TS_PARTICIPLE_ROOTS.iter().any(|r| f.starts_with(r))
            && !f.starts_with(TS_UNCHECKED_ROOT)
            && (f.ends_with(".ts") || f.ends_with(".tsx"));
        let rs = PARTICIPLE_ROOTS.iter().any(|r| f.starts_with(r))
            && (f.ends_with(".rs") || (f.ends_with(".wgsl") && !f.ends_with(".gen.wgsl")));
        if !ts && !rs {
            continue;
        }
        for (line, name) in participle_types(&read(f).unwrap_or_default(), ts) {
            if EVENTS.contains(&(*f, name.as_str())) {
                continue;
            }
            problems.push(format!(
                "{f}:{line}: `{name}` is a bare past participle; a type's name has a head noun saying what it holds, and a bare participle names only a state's variant or an event type in EVENTS (the naming skill, \"A type's head noun\"; check_rules.rs)"
            ));
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

    problems.extend(unlinked_files(&files, &read));

    // The generated shaders' stamp names the digest of the sources they were
    // made from (pxl-tool shader.rs `STAMP`); a set the tree has moved past is
    // stale.
    for f in stale(&root, &inputs_digest(&root)?) {
        problems.push(format!(
            "{f}: stale against pxl-spec-core's sources; run `cargo shader-wgsl` in rust/"
        ));
    }
    Ok(problems)
}

fn main() -> ExitCode {
    match run() {
        Ok(problems) if problems.is_empty() => {
            println!("check-rules: the memory's offsets, one rounding, ES modules, the generated shaders, the layers' order, the screen's pixels, the types' head nouns and the crates' module trees hold");
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
        let found = unlinked_files(&files, &read);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("rust/a/src/stray.rs:"));
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

    #[test]
    fn bare_participle_type_refused() {
        for text in [
            "struct Seen {",
            "pub(super) type Laid = u32;",
            "pub trait Observed: Base {",
            "    struct Kept(u32);",
            "struct Asked<'a> {",
            "pub enum Offered {",
        ] {
            assert_eq!(participle_types(text, false).len(), 1, "{text}");
        }
        for text in [
            "struct ResidentTile {",
            "struct Seed;",
            "struct Hit {",
            "enum State { Ready, Closed }",
            "// struct Seen",
            "let built = Built;",
        ] {
            assert!(participle_types(text, false).is_empty(), "{text}");
        }
        for text in [
            "export interface Bound {",
            "interface Opened extends Guest {",
            "export type Stored = number;",
            "export abstract class Named {",
        ] {
            assert_eq!(participle_types(text, true).len(), 1, "{text}");
        }
        assert!(participle_types("export interface Binding {", true).is_empty());
    }
}
