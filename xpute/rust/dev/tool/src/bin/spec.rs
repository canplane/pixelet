// xpute-tool/bin/spec.rs

//! This writes every file generated from spec/, or checks that each one on disk
//! is what its JSON writes.
//!
//!   cargo spec --write   # write every generated file
//!   cargo spec           # fail where one on disk is not what its JSON writes

use std::process::ExitCode;

use xpute_tool::spec::generate;

fn run() -> Result<(), String> {
    generate(std::env::args().any(|a| a == "--write"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[spec] {e}");
            ExitCode::FAILURE
        }
    }
}
