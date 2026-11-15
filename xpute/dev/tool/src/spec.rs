// xpute-tool/spec.rs

//! This writes xpute's own generated files, with the generators a program
//! takes: its data specs, the errno table among them (data.rs, xpute-gen),
//! as its targets declare them (xpute-gen-targets, xpute-gpu-gen-targets),
//! and its bindings, each package's as a program's are written
//! (xpute-link-gen `wit_outputs`), with each side's root module.

pub mod data;

use xpute_gen::spec::{generate_at, Output};
use xpute_link_gen::wit::{abi_root, wit_outputs, Side, TARGETS};

use crate::root;

/// The TypeScript packages pnpm-workspace.yaml lists: one `  - <dir>` a line
/// under `packages:`, up to a blank line. Anything else there refuses it.
fn workspace_dirs() -> Result<Vec<String>, String> {
    let file = "pnpm-workspace.yaml";
    let text = std::fs::read_to_string(root().join(file)).map_err(|e| format!("{file}: {e}"))?;
    let after = text
        .split_once("\npackages:\n")
        .ok_or(format!("{file}: no `packages:`"))?
        .1;
    let dirs: Vec<String> = after
        .lines()
        .take_while(|l| !l.is_empty())
        .map(|l| {
            l.strip_prefix("  - ")
                .filter(|d| {
                    !d.is_empty()
                        && d.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "_./-".contains(c))
                })
                .map(String::from)
                .ok_or(format!("{file}: `{l}` is not `  - <dir>`"))
        })
        .collect::<Result<_, _>>()?;
    if dirs.is_empty() {
        return Err(format!("{file}: lists no package"));
    }
    Ok(dirs)
}

/// Fails unless every TypeScript package that is published carries in its
/// `package.json` the version of Cargo.toml's workspace, the one place the
/// packages' version is set, and every one is named and has a src/.
fn check_versions() -> Result<(), String> {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).map_err(|e| format!("{p}: {e}"));
    let toml = read("Cargo.toml")?;
    let version = toml
        .split("[workspace.package]")
        .nth(1)
        .and_then(|t| t.lines().find_map(|l| l.trim().strip_prefix("version = ")))
        .map(|v| v.trim_matches('"').to_string())
        .ok_or("Cargo.toml: no workspace version")?;
    for d in workspace_dirs()? {
        let p = format!("{d}/package.json");
        let json: serde_json::Value =
            serde_json::from_str(&read(&p)?).map_err(|e| format!("{p}: {e}"))?;
        if !json["name"].is_string() {
            return Err(format!("{p}: no name"));
        }
        if !root().join(&d).join("src").is_dir() {
            return Err(format!("{d}: no src/"));
        }
        if json["private"] != true && json["version"].as_str() != Some(version.as_str()) {
            return Err(format!("{p}: version is not Cargo.toml's {version}"));
        }
    }
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    check_versions()?;
    let mut out = data::outputs()?;
    for t in TARGETS {
        out.extend(wit_outputs(&root(), t)?);
    }
    for side in [Side::Guest, Side::Host] {
        out.push(abi_root(&root(), side)?);
    }
    Ok(out)
}

/// With `write`, writes every generated file that differs from what its source
/// writes, and deletes every generated file that nothing writes any more.
/// Without it, fails where a file is either.
pub fn generate(write: bool) -> Result<(), String> {
    generate_at(&root(), outputs()?, &[], write)
}
