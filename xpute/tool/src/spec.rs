// xpute-tool/spec.rs

//! This writes xpute's own generated files, with the generators a program
//! takes: its data specs, the errno table among them (data.rs, xpute-gen),
//! as its targets declare them (xpute-gen-targets),
//! and its bindings, each package's as a program's are written
//! (xpute-link-gen `wit_outputs`), with each side's root module, and the
//! license beside each crate and package.

pub mod data;

use xpute_gen::spec::{Output, generate_at};
use xpute_link_gen::wit::{Side, TARGETS, abi_root, wit_outputs};

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

/// The workspace's crates, as Cargo.toml's `members` lists them.
fn crate_dirs(toml: &str) -> Result<Vec<String>, String> {
    let members = toml
        .split_once("members = [")
        .and_then(|(_, t)| t.split_once(']'))
        .ok_or("Cargo.toml: no workspace members")?
        .0;
    Ok(members
        .split(',')
        .map(|m| m.trim().trim_matches('"').to_string())
        .filter(|m| !m.is_empty())
        .collect())
}

/// The root's `LICENSE`, the one place its text is, beside every crate and
/// every TypeScript package: a registry takes a unit's own directory and
/// nothing above it.
fn licenses() -> Result<Vec<Output>, String> {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).map_err(|e| format!("{p}: {e}"));
    let text = read("LICENSE")?;
    Ok(crate_dirs(&read("Cargo.toml")?)?
        .into_iter()
        .chain(workspace_dirs()?)
        .map(|d| Output::new(&format!("{d}/LICENSE"), text.clone()))
        .collect())
}

/// Fails unless every TypeScript package that is published carries in its
/// `package.json` the version of Cargo.toml's workspace, the one place the
/// packages' version is set, and the root package.json's `engines`, which is
/// the Node of .tool-versions, the one place the Node is, at its major; unless
/// every package is named and has a src/; and unless the workspace's
/// `rust-version` is the toolchain's (rust-toolchain.toml), the one place the
/// Rust is, at its minor, and every crate that is published inherits it; and
/// unless every crate and package that is published says the workspace's
/// `license`.
fn check_versions() -> Result<(), String> {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).map_err(|e| format!("{p}: {e}"));
    let toml = read("Cargo.toml")?;
    let package = |key: &str| {
        toml.split("[workspace.package]")
            .nth(1)
            .and_then(|t| t.lines().find_map(|l| l.trim().strip_prefix(key)))
            .map(|v| v.trim_matches('"').to_string())
            .ok_or(format!(
                "Cargo.toml: no workspace {}",
                key.trim_end_matches(" = ")
            ))
    };
    let version = package("version = ")?;
    let rust = package("rust-version = ")?;
    let license = package("license = ")?;
    let channel = read("rust-toolchain.toml")?
        .lines()
        .find_map(|l| l.strip_prefix("channel = "))
        .map(|v| v.trim_matches('"').to_string())
        .ok_or("rust-toolchain.toml: no channel")?;
    if channel.rsplit_once('.').map(|(minor, _)| minor) != Some(rust.as_str()) {
        return Err(format!(
            "Cargo.toml: rust-version {rust} is not rust-toolchain.toml's {channel}"
        ));
    }
    for m in crate_dirs(&toml)? {
        let p = format!("{m}/Cargo.toml");
        let crate_toml = read(&p)?;
        let has = |line: &str| crate_toml.lines().any(|l| l == line);
        if has("publish = false") {
            continue;
        }
        if !has("rust-version.workspace = true") {
            return Err(format!("{p}: no rust-version.workspace = true"));
        }
        if !has("license.workspace = true") {
            return Err(format!("{p}: no license.workspace = true"));
        }
    }
    let manifest = |p: &str| -> Result<serde_json::Value, String> {
        serde_json::from_str(&read(p)?).map_err(|e| format!("{p}: {e}"))
    };
    let node = read(".tool-versions")?
        .lines()
        .find_map(|l| l.strip_prefix("nodejs "))
        .and_then(|v| v.split('.').next())
        .map(String::from)
        .ok_or(".tool-versions: no nodejs")?;
    let engines = manifest("package.json")?["engines"].clone();
    if engines["node"].as_str() != Some(format!(">={node}").as_str()) {
        return Err(format!(
            "package.json: engines.node is not >={node}, .tool-versions' Node"
        ));
    }
    for d in workspace_dirs()? {
        let p = format!("{d}/package.json");
        let json = manifest(&p)?;
        if !json["name"].is_string() {
            return Err(format!("{p}: no name"));
        }
        if !root().join(&d).join("src").is_dir() {
            return Err(format!("{d}: no src/"));
        }
        if json["private"] == true {
            continue;
        }
        if json["version"].as_str() != Some(version.as_str()) {
            return Err(format!("{p}: version is not Cargo.toml's {version}"));
        }
        if json["engines"] != engines {
            return Err(format!("{p}: engines is not the root package.json's"));
        }
        if json["license"].as_str() != Some(license.as_str()) {
            return Err(format!("{p}: license is not Cargo.toml's {license}"));
        }
    }
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    check_versions()?;
    let mut out = data::outputs()?;
    out.extend(licenses()?);
    for t in TARGETS {
        out.extend(wit_outputs(&root(), t, &[])?);
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
