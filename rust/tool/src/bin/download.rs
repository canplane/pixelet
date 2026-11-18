// pxl-tool/bin/download.rs

//! Fetches a source of the tree's data by its name.
//!
//!   make download-imagery | download-moon | download-pack | download-star

use std::process::ExitCode;

use pxl_tool::download::{imagery, moon, pack, star};

/// What fetches a source.
type Run = fn() -> Result<(), String>;

const SOURCES: [(&str, Run); 4] = [
    ("imagery", imagery::run),
    ("moon", moon::run),
    ("pack", pack::run),
    ("star", star::run),
];

fn main() -> ExitCode {
    let name = std::env::args().nth(1).unwrap_or_default();
    let Some((_, run)) = SOURCES.iter().find(|(n, _)| *n == name) else {
        eprintln!("usage: download imagery|moon|pack|star");
        return ExitCode::FAILURE;
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[{name}] {e}");
            ExitCode::FAILURE
        }
    }
}
