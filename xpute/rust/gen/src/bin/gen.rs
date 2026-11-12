// xpute-gen/bin/gen.rs

//! This generates a program's manifest into the files its `gen` names
//! (spec/manifest.rs). With `--check` it writes nothing, and fails where a
//! file on disk differs from what it would write.
//!
//!   xpute-gen <manifest.json> [--check]
//!
//! Paths are relative to the directory it runs in.

use std::process::ExitCode;

use xpute_gen::spec::apply_at;
use xpute_gen::spec::manifest;

const USAGE: &str = "xpute-gen <manifest.json> [--check]";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let jsons: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if jsons.is_empty() || args.len() != jsons.len() + usize::from(check) {
        return Err(USAGE.into());
    }
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut outputs = Vec::new();
    for json in jsons {
        outputs.extend(manifest::outputs(&root, json)?);
    }
    apply_at(&root, outputs, !check)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[xpute-gen] {e}");
            ExitCode::FAILURE
        }
    }
}
