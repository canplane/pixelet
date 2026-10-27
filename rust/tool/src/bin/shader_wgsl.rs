// pxl-tool/bin/shader_wgsl.rs

//! Makes pxl-spec-core's WGSL (pxl-tool shader.rs): rust/client/shader built
//! by rust-gpu inside a Linux container, its SPIR-V read into WGSL by naga
//! (pxl-shader-build), each entry checked in under its header and the digest
//! of the sources that made it.
//!
//!   cargo shader-wgsl            write them   (in rust/)
//!   cargo shader-wgsl --force    write them, made again whatever they name
//!   cargo shader-wgsl --check    fail if one is not current
//!
//! A write makes nothing when every file already names the digest of what
//! makes it as it is now (pxl-tool shader.rs `INPUT_ROOTS`), the record
//! check_rules holds each to, so the container would write the same bytes.
//! A file edited by hand under its digest is what `--force` is for; `--check`
//! always makes them and compares.
//!
//! Whether the browser's compiler takes what this writes is the web app's
//! test (ts/app/web src/shader.test.ts, Tint through Dawn), which make
//! test-ts runs: naga takes modules Tint refuses.
//!
//! The check judges only that each checked-in file is what the source makes
//! now; it never compares two implementations, since there is one. Each file
//! carries the digest of its sources, which check_rules recomputes without
//! the container, so a file the source has moved past fails there too.

use std::path::Path;
use std::process::{Command, ExitCode};

use pxl_tool::root;
use pxl_tool::shader::{header, inputs_digest, made_from, stale, DIR, ENTRIES};

const IMAGE: &str = "rust:1-bookworm";
const BUILT: &str =
    "rust/target/shader/spirv-builder/spirv-unknown-vulkan1.3/release/deps/pxl_shader.spvs";

/// Each entry's module as it is checked in, under its header: the toolchain
/// comes from rust/client/shader/rust-toolchain.toml, which rustup's cargo
/// installs on first use. The container is let go after each run, so the
/// toolchain and the crates it fetched are kept in the build's own target,
/// as `RUSTUP_HOME` and `CARGO_HOME` under their default names.
fn generate(
    root: &Path,
    digest: &str,
) -> Result<Vec<(&'static str, &'static str, String)>, String> {
    let status = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "-v"])
        .arg(format!("{}:/w", root.display()))
        .args([
            "-w",
            "/w/rust/client/shader",
            "-e",
            "CARGO_TARGET_DIR=/w/rust/target/shader",
            "-e",
            "CARGO_HOME=/w/rust/target/shader/.cargo",
            "-e",
            "RUSTUP_HOME=/w/rust/target/shader/.rustup",
            IMAGE,
            "bash",
            "-c",
            "cargo run -q --release -p pxl-shader-build",
        ])
        .status()
        .map_err(|e| format!("docker: {e}"))?;
    if !status.success() {
        return Err("the rust-gpu build failed".into());
    }
    ENTRIES
        .iter()
        .map(|&(entry, file)| {
            let path = root.join(BUILT).join(format!("{entry}.wgsl"));
            let wgsl =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok((entry, file, header(digest) + &wgsl))
        })
        .collect()
}

fn run() -> Result<bool, String> {
    let check = std::env::args().any(|a| a == "--check");
    let force = std::env::args().any(|a| a == "--force");
    let root = root();
    let digest = inputs_digest(&root)?;
    if !check && !force && stale(&root, &digest).is_empty() {
        println!("current: every entry names the sources' digest {digest}");
        return Ok(true);
    }
    let made = generate(&root, &digest)?;
    let mut current = true;
    for (_, file, text) in &made {
        let target = root.join(DIR).join(file);
        if check {
            if std::fs::read_to_string(&target).ok().as_deref() == Some(text.as_str()) {
                println!("current: {}", target.display());
            } else {
                let named = std::fs::read_to_string(&target)
                    .is_ok_and(|text| made_from(&text) == Some(digest.as_str()));
                let mend = if named { " --force" } else { "" };
                eprintln!(
                    "stale: {} is not what the source makes now; run `cargo shader-wgsl{mend}`",
                    target.display()
                );
                current = false;
            }
        } else {
            // Renamed into place: a run killed mid-write would leave a body cut
            // short under a head that names the current digest, which a write
            // then takes as made.
            let part = target.with_extension("wgsl.part");
            std::fs::write(&part, text)
                .and_then(|()| std::fs::rename(&part, &target))
                .map_err(|e| format!("{}: {e}", target.display()))?;
            println!("wrote {} ({} bytes)", target.display(), text.len());
        }
    }
    Ok(current)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("[shader-wgsl] {e}");
            ExitCode::FAILURE
        }
    }
}
