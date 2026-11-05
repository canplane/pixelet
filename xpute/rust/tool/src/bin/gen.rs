// xpute-tool/bin/gen.rs

//! A program's machine and protocols generated where each one's `gen` says
//! (xpute-tool spec/machine.rs, spec/cmd.rs `protocol_outputs`), or, with
//! `--check`, a failure where a file is not what it writes. A file that
//! declares `mem` is a machine; any other, a protocol, and the protocols
//! together give no number twice.
//!
//!   xpute-gen <machine.json> <protocol.json>... [--check]
//!
//! Its paths are from the directory it runs in.

use std::process::ExitCode;

use xpute_tool::spec::apply_at;
use xpute_tool::spec::cmd::protocol_outputs;
use xpute_tool::spec::{machine, read};

const USAGE: &str = "xpute-gen <machine.json> <protocol.json>... [--check]";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let jsons: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if jsons.is_empty() || args.len() != jsons.len() + usize::from(check) {
        return Err(USAGE.into());
    }
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut outputs = Vec::new();
    let mut protocols = Vec::new();
    for json in jsons {
        let at = root.join(json).to_string_lossy().into_owned();
        if read(&at)?.get("mem").is_some() {
            outputs.extend(machine::outputs(&root, json)?);
        } else {
            protocols.push(json.as_str());
        }
    }
    outputs.extend(protocol_outputs(&root, &protocols)?);
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
