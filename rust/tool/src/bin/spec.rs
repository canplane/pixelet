// pxl-tool/bin/spec.rs

//! Every file generated from `spec/`, written or checked.
//!
//!   make spec          # write every generated file
//!   make check-spec    # fail where one on disk is not what its JSON writes

use std::process::ExitCode;
use std::thread;

use pxl_tool::fmt::formatted;
use pxl_tool::root;
use pxl_tool::spec::{outputs, Output};

fn run() -> Result<(), String> {
    let write = std::env::args().any(|a| a == "--write");
    let outputs = thread::scope(|scope| {
        let handles: Vec<_> = outputs()?
            .into_iter()
            .map(|o| scope.spawn(move || formatted(&o.path, o.text).map(|text| Output { path: o.path, text })))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().map_err(|_| "a formatter thread panicked".to_string())?)
            .collect::<Result<Vec<_>, String>>()
    })?;
    let root = root();
    let stale: Vec<&Output> = outputs
        .iter()
        .filter(|o| std::fs::read_to_string(root.join(&o.path)).ok().as_deref() != Some(o.text.as_str()))
        .collect();
    if write {
        // Only what differs: a rewrite moves the mtime, and cargo and Vite rebuild.
        for o in &stale {
            let at = root.join(&o.path);
            std::fs::create_dir_all(at.parent().ok_or("a path with no parent")?).map_err(|e| e.to_string())?;
            std::fs::write(&at, &o.text).map_err(|e| format!("{}: {e}", o.path))?;
            println!("wrote {}", o.path);
        }
        return Ok(());
    }
    let stale: Vec<&str> = stale.iter().map(|o| o.path.as_str()).collect();
    if !stale.is_empty() {
        for s in &stale {
            eprintln!("spec: {s} is not what its JSON writes — run `make spec`");
        }
        return Err(format!("{} stale", stale.len()));
    }
    println!("spec: {} generated files are what their JSON writes", outputs.len());
    Ok(())
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
