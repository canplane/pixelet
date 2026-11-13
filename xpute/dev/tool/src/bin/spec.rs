// xpute-tool/bin/spec.rs

//! This writes every file generated from xpute's JSON and WIT, or checks that
//! each one on disk is what its source writes.
//!
//!   cargo spec --write   # write every generated file
//!   cargo spec           # fail where one on disk is not what its source writes

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
