// pxl-tool/bin/finish_wasm.rs

//! Rewrites a linked wasm module in place, for ts/app/web/script/build-wasm.py.
//!
//!   finish_wasm exports <module.wasm> <name>...      # keep only these exports
//!   finish_wasm number <module.wasm> <paths.json>    # number the panic paths

use std::process::ExitCode;

use regex::bytes::Regex;

const EXPORT_SECTION: u8 = 7;

fn leb(bytes: &[u8], at: &mut usize) -> Result<u32, String> {
    let (mut n, mut shift) = (0u32, 0u32);
    loop {
        let b = *bytes.get(*at).ok_or("a LEB128 past the module's end")?;
        *at += 1;
        n |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            return Ok(n);
        }
        shift += 7;
    }
}

fn put(mut n: u32, out: &mut Vec<u8>) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        out.push(if n == 0 { b } else { b | 0x80 });
        if n == 0 {
            return;
        }
    }
}

/// embassy and critical-section export their `#[no_mangle]` symbols; only `keep` stays.
fn exported_only(bytes: &[u8], keep: &[String]) -> Result<Vec<u8>, String> {
    let mut out = bytes.get(..8).ok_or("not a wasm module")?.to_vec();
    let mut at = 8;
    let mut kept = 0;
    while at < bytes.len() {
        let start = at;
        let id = bytes[at];
        at += 1;
        let size = leb(bytes, &mut at)? as usize;
        let end = at + size;
        if id != EXPORT_SECTION {
            out.extend_from_slice(bytes.get(start..end).ok_or("a section past the module's end")?);
            at = end;
            continue;
        }
        let mut entries = Vec::new();
        for _ in 0..leb(bytes, &mut at)? {
            let from = at;
            let len = leb(bytes, &mut at)? as usize;
            let name = bytes.get(at..at + len).ok_or("an export name past the module's end")?;
            // The name, then its kind byte and its index.
            at += len + 1;
            leb(bytes, &mut at)?;
            if keep.iter().any(|k| k.as_bytes() == name) {
                entries.extend_from_slice(&bytes[from..at]);
                kept += 1;
            }
        }
        let mut body = Vec::new();
        put(kept, &mut body);
        body.extend(entries);
        out.push(EXPORT_SECTION);
        put(body.len() as u32, &mut out);
        out.extend(body);
        at = end;
    }
    if kept as usize != keep.len() {
        return Err(format!("the module exports {kept} of {}", keep.join(", ")));
    }
    Ok(out)
}

/// The workspace's members, as rust/Cargo.toml lists them in the tree the
/// tool runs in (`pxl_tool::root`): Cargo hands rustc a member's paths
/// relative to the workspace, so no prefix remap reaches them.
fn members() -> Result<Vec<String>, String> {
    let at = pxl_tool::root().join("rust/Cargo.toml");
    let manifest = std::fs::read_to_string(&at).map_err(|e| format!("{}: {e}", at.display()))?;
    let list = regex::Regex::new(r"(?ms)^\s*members\s*=\s*\[([^\]]*)\]").map_err(|e| e.to_string())?;
    let comment = regex::Regex::new(r"(?m)#.*$").map_err(|e| e.to_string())?;
    let quoted = regex::Regex::new(r#""([^"]+)""#).map_err(|e| e.to_string())?;
    let body = list.captures(&manifest).ok_or("rust/Cargo.toml: no workspace members")?;
    let body = comment.replace_all(&body[1], "");
    Ok(quoted.captures_iter(&body).map(|c| format!("{}/src/", &c[1])).collect())
}

/// Rust embeds caller paths for `#[track_caller]` panics with no stable way
/// out. They are overwritten in place, same length, as `...#n`; a `&'static
/// str` keeps its length apart, so the module runs the same. Returns the
/// paths in the order they were numbered.
fn numbered(bytes: &mut [u8]) -> Result<Vec<String>, String> {
    let path = Regex::new(r"(?-u)[A-Za-z0-9_/.-]*\.rs\b").map_err(|e| e.to_string())?;
    let source_root = Regex::new(r"(?-u)(?:^|[^A-Za-z0-9_])xpute/|/library/|/registry/|^/rustc/|^/rust/deps/|\.rustup/").map_err(|e| e.to_string())?;
    let members = members()?;
    let found: Vec<(usize, usize)> = path.find_iter(bytes).map(|m| (m.start(), m.end())).collect();
    let mut numbers: Vec<String> = Vec::new();
    for (start, end) in found {
        let text = String::from_utf8_lossy(&bytes[start..end]).into_owned();
        if !text.contains('/') {
            continue;
        }
        if !source_root.is_match(text.as_bytes()) && !members.iter().any(|m| text.starts_with(m.as_str())) {
            // Never overwrite on a guess: it could be a format string.
            return Err(format!("{text:?} ends in .rs but is under no source root — widen the source roots or leave it alone"));
        }
        let k = match numbers.iter().position(|p| *p == text) {
            Some(k) => k,
            None => {
                numbers.push(text);
                numbers.len() - 1
            }
        };
        // The number goes last: the linker merges a string that is another's tail.
        let tag = format!("#{k}").into_bytes();
        let at = start.max(end.saturating_sub(tag.len()));
        bytes[start..at].fill(b'.');
        bytes[at..end].copy_from_slice(&tag[tag.len() - (end - at)..]);
    }
    Ok(numbers)
}

fn paths_json(numbers: &[String]) -> String {
    if numbers.is_empty() {
        return "{}\n".into();
    }
    let rows: Vec<String> = numbers.iter().enumerate().map(|(k, p)| format!("  \"#{k}\": {}", serde_json::Value::from(p.as_str()))).collect();
    format!("{{\n{}\n}}\n", rows.join(",\n"))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || "usage: finish_wasm exports <module.wasm> <name>... | finish_wasm number <module.wasm> <paths.json>".to_string();
    let (mode, module) = (args.first().ok_or_else(usage)?, args.get(1).ok_or_else(usage)?);
    let mut bytes = std::fs::read(module).map_err(|e| format!("{module}: {e}"))?;
    match mode.as_str() {
        "exports" => {
            let out = exported_only(&bytes, &args[2..])?;
            std::fs::write(module, out).map_err(|e| format!("{module}: {e}"))?;
        }
        "number" => {
            let paths = args.get(2).ok_or_else(usage)?;
            let numbers = numbered(&mut bytes)?;
            if !numbers.is_empty() {
                std::fs::write(module, &bytes).map_err(|e| format!("{module}: {e}"))?;
            }
            std::fs::write(paths, paths_json(&numbers)).map_err(|e| format!("{paths}: {e}"))?;
            println!("{}", numbers.len());
        }
        _ => return Err(usage()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[finish_wasm] {e}");
            ExitCode::FAILURE
        }
    }
}
