// pxl-tool/bin/shader_wgsl.rs

//! Makes pxl-spec-core's WGSL (pxl-tool shader.rs): rust/client/shader built
//! by rust-gpu inside a Linux container, its SPIR-V read into WGSL by naga
//! (pxl-shader-build), each entry checked in under its header and the digest
//! of the sources that made it.
//!
//!   cargo shader-wgsl            write them   (in rust/)
//!   cargo shader-wgsl --check    fail if one is not current
//!
//! Either way each module is compiled first by Tint, the WGSL compiler a
//! browser's WebGPU runs (script/tint.ts, over Dawn's), and nothing is written
//! where one is refused: naga takes modules Tint refuses (a literal past f32's
//! range, at e84ddd56e).
//!
//! The check judges only that each checked-in file is what the source makes
//! now; it never compares two implementations, since there is one. Each file
//! carries the digest of its sources, which check_rules recomputes without
//! the container, so a file the source has moved past fails there too.

use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use pxl_tool::root;
use pxl_tool::shader::{header, inputs_digest, DIR, ENTRIES};

const IMAGE: &str = "rust:1-bookworm";
const BUILT: &str = "rust/client/shader/target-linux/spirv-builder/spirv-unknown-vulkan1.3/release/deps/pxl_shader.spvs";

/// Each entry's module as it is checked in, under its header: the toolchain
/// comes from rust/client/shader/rust-toolchain.toml, which rustup's cargo
/// installs in the container on first use.
fn generate(root: &Path, digest: &str) -> Result<Vec<(&'static str, &'static str, String)>, String> {
    let status = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "-v"])
        .arg(format!("{}:/w", root.display()))
        .args([
            "-w",
            "/w/rust/client/shader",
            "-e",
            "CARGO_TARGET_DIR=/w/rust/client/shader/target-linux",
            "-e",
            "CARGO_HOME=/w/rust/client/shader/cargo-home",
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
            let wgsl = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok((entry, file, header(digest) + &wgsl))
        })
        .collect()
}

/// What Tint refuses of the modules, each said by the file it is written as
/// (script/tint.ts, given the modules as JSON on its input).
fn refused(root: &Path, made: &[(&str, &str, String)]) -> Result<Vec<String>, String> {
    let modules: Vec<serde_json::Value> = made.iter().map(|(entry, file, code)| serde_json::json!({ "entry": entry, "file": file, "code": code })).collect();
    let mut child = Command::new("deno")
        .args(["run", "-A", "tint.ts"])
        .current_dir(root.join("script"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("script/tint.ts: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(serde_json::Value::Array(modules).to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let said = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect::<Vec<_>>();
    if !out.status.success() && said.is_empty() {
        return Err("script/tint.ts failed".into());
    }
    Ok(said)
}

fn run() -> Result<bool, String> {
    let check = std::env::args().any(|a| a == "--check");
    let root = root();
    let made = generate(&root, &inputs_digest(&root)?)?;
    let refusals = refused(&root, &made)?;
    for r in &refusals {
        eprintln!("refused by Tint: {r}");
    }
    if !refusals.is_empty() {
        return Ok(false);
    }
    let mut current = true;
    for (_, file, text) in &made {
        let target = root.join(DIR).join(file);
        if check {
            if std::fs::read_to_string(&target).ok().as_deref() == Some(text.as_str()) {
                println!("current: {}", target.display());
            } else {
                eprintln!("stale: {} is not what the source makes now; run `cargo shader-wgsl`", target.display());
                current = false;
            }
        } else {
            std::fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
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
