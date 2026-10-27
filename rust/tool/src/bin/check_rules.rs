// pxl-tool/bin/check_rules.rs

//! Holds the tree to what a reader cannot see and the code must keep: no raw
//! pointer casts in offset crates, each rounding half to even in every
//! language, no CommonJS file, each generated shader made from the sources the
//! tree holds now, and the layers' order, where nothing names what is above
//! it. A rule Rust alone keeps is clippy's (rust/clippy.toml); how the tree is
//! written is the convention skill's.
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

/// pxl-spec-core's order: what every part speaks, the layers' storage, the
/// query over them (ARCHITECTURE.md, "The query, the only seam"), and what
/// asks it. A layer knows no query, and the query no asker.
const SPEC_CORE: &str = "rust/spec/core/src/";
const SPEC_CORE_ORDER: [(&str, u8); 24] = [
    ("cube", 0),
    ("lattice", 0),
    ("grid", 0),
    ("answer", 0),
    ("area", 1),
    ("caves", 1),
    ("ground", 1),
    ("window", 1),
    ("ways", 1),
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
    ("faces", 3),
    ("tally", 3),
];

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
/// `order`, or of one head by another.
fn ordered(
    files: &[&str],
    read: &dyn Fn(&str) -> Option<String>,
    root: &str,
    order: &[(&str, u8)],
    heads: &[&str],
) -> Vec<String> {
    let level = |m: &str| order.iter().find(|(n, _)| *n == m).map(|(_, l)| *l);
    let mut problems = Vec::new();
    for f in files.iter().filter(|f| f.starts_with(root) && f.ends_with(".rs")) {
        let me = standing(f, root);
        let Some(mine) = level(&me) else { continue };
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
    problems
}

fn run() -> Result<Vec<String>, String> {
    let root = root();
    let out = Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    let listed = String::from_utf8(out.stdout).map_err(|e| format!("git ls-files: {e}"))?;
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

    problems.extend(ordered(&files, &read, KERNEL, &KERNEL_ORDER, &KERNEL_HEADS));
    problems.extend(ordered(&files, &read, SPEC_CORE, &SPEC_CORE_ORDER, &[]));
    let pixelet = Regex::new(PIXELET_WORDS).expect("pixelet's words' pattern");
    for f in files.iter().filter(|f| f.starts_with(XPUTE)) {
        for (n, line) in read(f).unwrap_or_default().lines().enumerate() {
            // Make's `$(shell …)` is the build tool's, not pixelet's shell.
            if pixelet.is_match(&line.replace("$(shell", "")) {
                problems.push(format!("{f}:{}: names pixelet's word under xpute: {}", n + 1, line.trim()));
            }
        }
    }

    // A generated shader names the digest of the sources it was made from
    // (pxl-tool shader.rs `inputs_digest`); one the tree has moved past is stale.
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
            println!("check-rules: the memory's offsets, one rounding, ES modules, the generated shaders and the layers' order hold");
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
