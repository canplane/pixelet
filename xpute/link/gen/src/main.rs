// xpute-link-gen/main.rs

//! The command that writes a program's bindings, or checks them, over
//! xpute's own packages, which it carries itself. Its paths are from the
//! directory it runs in.
//!
//!   xpute-link-gen <guest|host> <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>] [--check]
//!
//! `<wit>` is the directory of the program's package, whose world is its
//! guest's. The guest takes Rust and C++, a host Rust and TypeScript. With
//! `--check` it writes nothing and fails where a file on disk is not what the
//! WIT writes.

use std::path::Path;
use std::process::ExitCode;

use xpute_gen::spec::apply_at;
use xpute_link_gen::wit::{wit_outputs, Side, WitTarget};

const USAGE: &str =
    "xpute-link-gen <guest|host> <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>] [--check]";

fn run(args: &[String]) -> Result<(), String> {
    let (side, wit, mut rest) = match args {
        [side, wit, rest @ ..] => (side, wit, rest),
        _ => return Err(USAGE.into()),
    };
    let side = match side.as_str() {
        "guest" => Side::Guest,
        "host" => Side::Host,
        _ => return Err(USAGE.into()),
    };
    let mut t = WitTarget {
        wit,
        rs: None,
        ts: None,
        cpp: None,
        side: Some(side),
    };
    let mut check = false;
    while let [flag, tail @ ..] = rest {
        rest = tail;
        let at = match flag.as_str() {
            "--check" => {
                check = true;
                continue;
            }
            "--rs" => &mut t.rs,
            "--ts" => &mut t.ts,
            "--cpp" => &mut t.cpp,
            _ => return Err(USAGE.into()),
        };
        let [path, tail @ ..] = rest else {
            return Err(USAGE.into());
        };
        *at = Some(path);
        rest = tail;
    }
    let root = Path::new(".")
        .canonicalize()
        .map_err(|e| format!(".: {e}"))?;
    apply_at(&root, wit_outputs(&root, &t)?, !check)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[xpute-link-gen] {e}");
            ExitCode::FAILURE
        }
    }
}
