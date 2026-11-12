// xpute-tool/spec.rs

//! This writes xpute's own generated files, with the generator a program takes
//! (xpute-gen): its data specs (data.rs), its errno table (errno.rs) and its
//! link's bindings, from wit/.

pub mod data;
pub mod errno;

use xpute_gen::spec::wit::{wit_outputs, WitTarget};
use xpute_gen::spec::{generate_at, Output};

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
    let mut out = errno::outputs()?;
    out.extend(data::outputs()?);
    out.extend(wit_outputs(
        &root(),
        &WitTarget {
            dirs: &["wit"],
            rs: Some(("rust/core/src/abi.rs", "xpute-core/abi.rs")),
            ts: Some(("ts/runtime/src/abi", "@xpute/runtime/abi")),
            core: "crate",
            runtime_ts: "../link.ts",
            externs: &[],
        },
    )?);
    out.extend(wit_outputs(
        &root(),
        &WitTarget {
            dirs: &["wit", "wit/gpu"],
            rs: Some(("rust/gpu/src/abi.rs", "xpute-gpu/abi.rs")),
            ts: Some(("ts/gpu/src/abi", "@xpute/gpu/abi")),
            core: "xpute_core",
            runtime_ts: "@xpute/runtime",
            externs: &[("xpute:core", "xpute_core::abi", "@xpute/runtime/abi")],
        },
    )?);
    Ok(out)
}

/// With `write`, writes every generated file that differs from what its JSON
/// writes, and deletes every generated file that nothing writes any more.
/// Without it, fails where a file is either.
pub fn generate(write: bool) -> Result<(), String> {
    generate_at(&root(), outputs()?, &[], write)
}
