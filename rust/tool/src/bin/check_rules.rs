// pxl-tool/bin/check_rules.rs

//! Holds the tree to what a reader cannot see and the code must keep: no raw
//! pointer casts in offset crates, each rounding half to even in every
//! language, no CommonJS file, each generated shader made from the sources the
//! tree holds now. A rule Rust alone keeps is clippy's (rust/clippy.toml);
//! how the tree is written is the convention skill's.
//!
//!   cargo check-rules            (in rust/; make check-rules from the root)

use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{generated, inputs_digest, INPUTS};
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

    // A generated shader names the digest of the sources it was made from
    // (pxl-tool shader.rs `inputs_digest`); one the tree has moved past is stale.
    let digest = inputs_digest(&root)?;
    for f in generated() {
        let made = read(&f).and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix(INPUTS).map(str::to_string))
        });
        if made.as_deref() != Some(digest.as_str()) {
            problems.push(format!(
                "{f}: stale against pxl-spec-core's sources; run `cargo shader-wgsl` in rust/"
            ));
        }
    }
    Ok(problems)
}

fn main() -> ExitCode {
    match run() {
        Ok(problems) if problems.is_empty() => {
            println!("check-rules: the memory's offsets, one rounding, ES modules and the generated shaders hold");
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
