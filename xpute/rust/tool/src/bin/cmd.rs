// xpute-tool/bin/cmd.rs

//! A program's own command table, generated for both ends:
//!
//! ```sh
//! cargo run -p xpute-tool --bin cmd -- cmd.json --ts host/cmd.spec.ts --rs guest/src/cmd.spec.rs [--write]
//! ```
//!
//! Without `--write` it says which files are stale and fails, as `spec` does.

use std::path::Path;
use std::process::ExitCode;

use xpute_tool::spec::{apply, cmd};

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let abs = |p: &str| cwd.join(Path::new(p)).to_string_lossy().into_owned();
    let after = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .ok_or(format!("cmd: {flag} <path> is missing"))
    };
    let json = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("cmd: the table's path comes first")?;
    let (ts, rs) = (after("--ts")?, after("--rs")?);
    let outputs = cmd::program_outputs(&abs(json), &abs(ts), &abs(rs), [json, ts, rs])?;
    apply(outputs, args.iter().any(|a| a == "--write"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[cmd] {e}");
            ExitCode::FAILURE
        }
    }
}
