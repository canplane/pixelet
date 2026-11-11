// xpute-tool/spec.rs

//! This writes xpute's own generated files, with the generator a program takes
//! (xpute-gen): its protocols (cmd.rs), its data specs (data.rs), its errno
//! table (errno.rs) and its example's manifest.

pub mod cmd;
pub mod data;
pub mod errno;

use xpute_gen::spec::cmd::protocol_outputs;
use xpute_gen::spec::{generate_at, manifest, Output};

use crate::root;

/// Fails unless every TypeScript package's `package.json` carries the version
/// of rust/Cargo.toml's workspace, the one place the packages' version is set.
fn check_versions() -> Result<(), String> {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).map_err(|e| format!("{p}: {e}"));
    let toml = read("rust/Cargo.toml")?;
    let version = toml
        .split("[workspace.package]")
        .nth(1)
        .and_then(|t| t.lines().find_map(|l| l.trim().strip_prefix("version = ")))
        .map(|v| v.trim_matches('"').to_string())
        .ok_or("rust/Cargo.toml: no workspace version")?;
    for p in ["kit", "runtime", "gpu", "native"].map(|n| format!("ts/{n}/package.json")) {
        let json: serde_json::Value =
            serde_json::from_str(&read(&p)?).map_err(|e| format!("{p}: {e}"))?;
        if json["version"].as_str() != Some(version.as_str()) {
            return Err(format!("{p}: version is not rust/Cargo.toml's {version}"));
        }
    }
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    check_versions()?;
    let mut out = cmd::outputs()?;
    out.extend(manifest::outputs(
        &root(),
        "rust/dev/example/manifest.json",
    )?);
    out.extend(protocol_outputs(&root(), &["rust/dev/example/asker.json"])?);
    out.extend(errno::outputs()?);
    out.extend(data::outputs()?);
    Ok(out)
}

/// With `write`, writes every generated file that differs from what its JSON
/// writes, and deletes every generated file that nothing writes any more.
/// Without it, fails where a file is either.
pub fn generate(write: bool) -> Result<(), String> {
    generate_at(&root(), outputs()?, &[], write)
}
