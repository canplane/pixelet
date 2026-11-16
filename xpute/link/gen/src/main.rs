// xpute-link-gen/main.rs

//! The command that writes a program's bindings, or checks them, over
//! xpute's own packages, which it carries itself. Its paths are from the
//! directory it runs in.
//!
//!   xpute-link-gen <guest|host> <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>]
//!       [--use <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>]]... [--check]
//!
//! `<wit>` is the directory of the program's package, whose world is its
//! guest's. The guest takes Rust and C++, a host Rust and TypeScript. Each
//! `--use` names a plugin's package the program's takes, and the paths after
//! it where that package's bindings of the same side lie, which the program's
//! bindings name. With `--check` it writes nothing and fails where a file on
//! disk is not what the WIT writes.

use std::path::Path;
use std::process::ExitCode;

use xpute_gen::spec::apply_at;
use xpute_link_gen::wit::{Side, WitTarget, wit_outputs};

const USAGE: &str = "xpute-link-gen <guest|host> <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>] \
     [--use <wit> [--rs <file>] [--ts <dir>] [--cpp <dir>]]... [--check]";

fn run(args: &[String]) -> Result<(), String> {
    let [side, wit, rest @ ..] = args else {
        return Err(USAGE.into());
    };
    let mut rest = rest;
    let side = match side.as_str() {
        "guest" => Side::Guest,
        "host" => Side::Host,
        _ => return Err(USAGE.into()),
    };
    let target = |wit| WitTarget {
        wit,
        rs: None,
        ts: None,
        cpp: None,
        side: Some(side),
    };
    // The program's package first, then each plugin's, in the order named;
    // a path flag sets the package named last.
    let mut targets = vec![target(wit)];
    let mut check = false;
    while let [flag, tail @ ..] = rest {
        rest = tail;
        if flag == "--check" {
            check = true;
            continue;
        }
        let [path, tail @ ..] = rest else {
            return Err(USAGE.into());
        };
        rest = tail;
        if flag == "--use" {
            targets.push(target(path));
            continue;
        }
        let t = targets.last_mut().ok_or(USAGE)?;
        let at = match flag.as_str() {
            "--rs" => &mut t.rs,
            "--ts" => &mut t.ts,
            "--cpp" => &mut t.cpp,
            _ => return Err(USAGE.into()),
        };
        *at = Some(path);
    }
    let root = Path::new(".")
        .canonicalize()
        .map_err(|e| format!(".: {e}"))?;
    apply_at(
        &root,
        wit_outputs(&root, &targets[0], &targets[1..])?,
        !check,
    )
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
