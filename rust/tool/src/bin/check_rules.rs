// pxl-tool/bin/check_rules.rs

//! Holds the tree to what a reader cannot see and the code must keep: no raw
//! pointer casts in offset crates, each rounding half to even in every
//! language, no CommonJS file, each generated shader made from the sources the
//! tree holds now, the layers' order, where nothing names what is above it,
//! and the screen's pixels read only at the screen. A rule Rust alone keeps
//! is clippy's (rust/clippy.toml); how the tree is written is the convention
//! skill's.
//!
//!   cargo check-rules            (in rust/; make check-rules from the root)

use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{inputs_digest, stale};
use regex::Regex;

/// The crates that address the core memory by offset, where an offset becomes
/// a pointer only through `xpute_guest::mem::at` or `Memory::at`.
const OFFSET_CRATES: [&str; 6] = [
    "xpute/rust/guest/src/",
    "xpute/rust/host/src/",
    "rust/client/kernel/src/",
    "rust/client/render/src/",
    "rust/client-bracket/wasm/src/",
    "rust/client-bracket/native/src/",
];

/// One rounding: half to even (ARCHITECTURE.md, Terrain). A call that rounds
/// otherwise, or a floor of a half added, is refused in pixelet's code and
/// xpute's; xpute's scalar keeps its named variants, round_half_up and
/// round_half_away, as options a library offers.
const ROUNDING_ROOTS: [&str; 3] = ["rust/", "ts/", "xpute/"];
const ROUNDING_NAMED: [&str; 6] = [
    "xpute/rust/kit/src/math/scalar.rs",
    "xpute/rust/kit/src/math/scalar.test.rs",
    "xpute/ts/kit/src/math/scalar.ts",
    "xpute/ts/kit/src/math/scalar.test.ts",
    "xpute/cpp/xpute/kit/math/scalar.hpp",
    "xpute/cpp/xpute/kit/math/scalar.test.cpp",
];
const NOT_EVEN: [&str; 9] = [
    r"\.round\(\)",
    r"\bf(?:32|64)::round\b",
    r"\blibm::roundf?\(",
    r"\bMath\.round\(",
    r"\bstd::round\b",
    // A number's text rounds its last place half up: `fixed` (ts/client shell/fixed.ts) rounds it half to even.
    r"\.toFixed\(",
    r"\.toPrecision\(",
    // A half added inside a floor's own parentheses, or before `.floor()`.
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
    ("approach", 4),
    ("param", 4),
    ("view", 3),
    ("query", 2),
    ("layer", 1),
    ("sys", 0),
];
const KERNEL_HEADS: [&str; 2] = ["scene", "control"];
const KERNEL_BESIDE: [&str; 1] = ["profiler"];

/// pxl-spec-core's order: what every part speaks, the layers' storage and
/// the world's fields, the query over them (ARCHITECTURE.md, "The query, the
/// only seam"), and what asks it. A layer knows no query, and the query no
/// asker.
const SPEC_CORE: &str = "rust/spec/core/src/";
const SPEC_CORE_ORDER: [(&str, u8); 26] = [
    ("param", 0),
    ("cube", 0),
    ("lattice", 0),
    ("grid", 0),
    ("answer", 0),
    ("tally", 0),
    ("area", 1),
    ("cave", 1),
    ("cloud", 1),
    ("ground", 1),
    ("window", 1),
    ("way", 1),
    ("fill", 1),
    ("overwrite", 1),
    ("world", 2),
    ("cast", 2),
    ("support", 2),
    ("observed", 2),
    ("beam", 3),
    ("eye", 3),
    ("raster", 3),
    ("ray", 3),
    ("sun", 3),
    ("scatter", 3),
    ("decor", 3),
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

/// On the GPU the screen's size is the eye's rays' image's (pxl-shader
/// cast.rs `Rays`), bound only to the passes that draw a pixel's ray or read
/// the rays drawn: the cast and its walks again, and the sun's receivers. The
/// beam, the cells, the samples, decor and the sun's map read the eye's
/// angles alone.
const SHADER: &str = "rust/client/shader/src/";
const RAYS_ENTRIES: [&str; 4] = ["cast_cs", "miss_cs", "miss_list_cs", "sun_mark_cs"];

/// Every entry point in `text`, a shader's source, whose parameters bind
/// the rays' image, by its name or a path to it, and is not one of
/// `RAYS_ENTRIES`, each with its line: the parameters from its
/// `entry_point_name` to the body's opening brace.
fn rays_bound(text: &str) -> Vec<(usize, String)> {
    let entry = Regex::new(r#"entry_point_name\s*=\s*"(\w+)""#).expect("an entry's pattern");
    let rays = Regex::new(r"&\s*(?:\w+::)*Rays\b").expect("the rays' pattern");
    let mut out = Vec::new();
    for c in entry.captures_iter(text) {
        let (at, name) = (c.get(0).map_or(0, |m| m.start()), &c[1]);
        let end = text[at..].find(") {").map_or(text.len(), |k| at + k);
        if rays.is_match(&text[at..end]) && !RAYS_ENTRIES.contains(&name) {
            out.push((text[..at].matches('\n').count() + 1, name.to_string()));
        }
    }
    out
}

/// xpute's words are host, guest and their contract (AGENTS.md): pixelet's
/// appear nowhere under it.
const XPUTE: &str = "xpute/";
const PIXELET_WORDS: &str = r"(?i)\b(pixelet|kernel|shell)\b";

/// The modules `text` names through `crate::`, a `domain::` step passed
/// over (the kernel's heads and trunk live under it), a group's every
/// first name among them, each with the line it is named on.
fn named(text: &str) -> Vec<(usize, String)> {
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
fn ordered(
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
    for f in files.iter().filter(|f| f.starts_with(root) && f.ends_with(".rs")) {
        let me = standing(f, root);
        let Some(mine) = level(&me) else {
            if !["lib", "domain"].contains(&me.as_str()) && !beside.contains(&me.as_str()) {
                unplaced.insert(me);
            }
            continue;
        };
        for (line, them) in named(&read(f).unwrap_or_default()) {
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
/// laid out under a checkout's ignored target (the collaboration skill),
/// every file under it but the builds' and the packages', which git there
/// would list none of. Outside any checkout, an error.
fn listed(root: &std::path::Path) -> Result<String, String> {
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
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            if ["target", "node_modules", ".git"].iter().any(|n| name == *n) {
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
    let listed = listed(&root)?;
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
        .filter(|f| f.ends_with(".rs") && OFFSET_CRATES.iter().any(|c| f.starts_with(c)))
    {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if !comment.is_match(line) && cast.is_match(line) {
                problems.push(format!("{f}:{}: casts to a pointer where an offset goes through mem::at or Memory::at: {}", n + 1, line.trim()));
            }
        }
    }

    let not_even: Vec<Regex> = NOT_EVEN
        .iter()
        .map(|p| Regex::new(p).expect("a rounding's pattern"))
        .collect();
    let rounded = Regex::new(r"\.(rs|tsx?|wgsl|hpp|cpp)$").expect("the extensions' pattern");
    for f in files.iter().filter(|f| {
        ROUNDING_ROOTS.iter().any(|r| f.starts_with(r))
            && !ROUNDING_NAMED.contains(f)
            && rounded.is_match(f)
            && !f.ends_with(".gen.wgsl")
    }) {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            if not_even.iter().any(|r| r.is_match(line)) {
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

    problems.extend(ordered(&files, &read, KERNEL, &KERNEL_ORDER, &KERNEL_HEADS, &KERNEL_BESIDE));
    problems.extend(ordered(&files, &read, SPEC_CORE, &SPEC_CORE_ORDER, &[], &[]));
    for f in files.iter().filter(|f| {
        f.starts_with(KERNEL) && f.ends_with(".rs") && !SCREEN_USERS.iter().any(|u| f.starts_with(u))
    }) {
        for line in screen_reached(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: reaches the screen's pixels where the world is decided in angles; only SCREEN_USERS may (check_rules.rs)"
            ));
        }
    }
    for f in files.iter().filter(|f| f.starts_with(SHADER) && f.ends_with(".rs")) {
        for (line, name) in rays_bound(&read(f).unwrap_or_default()) {
            problems.push(format!(
                "{f}:{line}: `{name}` binds the rays' image, the screen's size, past the passes that draw a pixel; only RAYS_ENTRIES may (check_rules.rs)"
            ));
        }
    }
    let pixelet = Regex::new(PIXELET_WORDS).expect("pixelet's words' pattern");
    for f in files.iter().filter(|f| f.starts_with(XPUTE)) {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            // Make's `$(shell …)` is the build tool's, not pixelet's shell.
            if pixelet.is_match(&line.replace("$(shell", "")) {
                problems.push(format!("{f}:{}: names pixelet's word under xpute: {}", n + 1, line.trim()));
            }
        }
    }

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
            println!("check-rules: the memory's offsets, one rounding, ES modules, the generated shaders, the layers' order and the screen's pixels hold");
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
    fn the_screen_is_reached_however_it_is_named() {
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
    fn only_the_listed_entries_bind_the_rays() {
        let entry = |name: &str, param: &str| {
            format!("#[spirv(compute(threads(8, 8, 1), entry_point_name = \"{name}\"))]\npub fn {name}(\n    {param}\n) {{\n    let x = 1;\n}}\n")
        };
        let rays = "#[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,";
        let pathed = "#[spirv(descriptor_set = 0, binding = 3)] rays: &crate::cast::Rays,";
        assert!(rays_bound(&entry("cast_cs", rays)).is_empty());
        assert_eq!(rays_bound(&entry("cell_cs", rays)).len(), 1);
        assert_eq!(rays_bound(&entry("cell_cs", pathed)).len(), 1);
        // A helper after an entry, or its body, is not the entry's binding.
        let after = entry("cell_cs", "id: UVec3,") + "fn written(rays: &Rays) {}\n";
        assert!(rays_bound(&after).is_empty());
    }
}
