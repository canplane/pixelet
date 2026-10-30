// xpute-tool/bin/spec.rs

//! Every file generated from spec/, written or checked.
//!
//!   make spec          # write every generated file
//!   make check-spec    # fail where one on disk is not what its JSON writes

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
