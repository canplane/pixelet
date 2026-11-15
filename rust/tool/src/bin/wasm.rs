// pxl-tool/bin/wasm.rs

//! Rewrites a linked wasm module in place, for ts/client/script/wasm-build.py.
//!
//!   wasm exports <module.wasm> <name>...      # keep only these exports

use std::process::ExitCode;

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

/// Removes every export but those named in `keep`: embassy and critical-section export their
/// `#[unsafe(no_mangle)]` symbols, which the module must not expose.
fn exported_only(bytes: &[u8], keep: &[String]) -> Result<Vec<u8>, String> {
    let mut out = bytes.get(..8).ok_or("not a wasm module")?.to_vec();
    let mut at = 8;
    let mut exported = 0;
    while at < bytes.len() {
        let start = at;
        let id = bytes[at];
        at += 1;
        let size = leb(bytes, &mut at)? as usize;
        let end = at + size;
        if id != EXPORT_SECTION {
            out.extend_from_slice(
                bytes
                    .get(start..end)
                    .ok_or("a section past the module's end")?,
            );
            at = end;
            continue;
        }
        let mut entries = Vec::new();
        for _ in 0..leb(bytes, &mut at)? {
            let from = at;
            let len = leb(bytes, &mut at)? as usize;
            let name = bytes
                .get(at..at + len)
                .ok_or("an export name past the module's end")?;
            // The name, then its kind byte and its index.
            at += len + 1;
            leb(bytes, &mut at)?;
            if keep.iter().any(|k| k.as_bytes() == name) {
                entries.extend_from_slice(&bytes[from..at]);
                exported += 1;
            }
        }
        let mut body = Vec::new();
        put(exported, &mut body);
        body.extend(entries);
        out.push(EXPORT_SECTION);
        put(body.len() as u32, &mut out);
        out.extend(body);
        at = end;
    }
    if exported as usize != keep.len() {
        return Err(format!(
            "the module exports {exported} of {}",
            keep.join(", ")
        ));
    }
    Ok(out)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || "usage: wasm exports <module.wasm> <name>...".to_string();
    let (mode, module) = (
        args.first().ok_or_else(usage)?,
        args.get(1).ok_or_else(usage)?,
    );
    let bytes = std::fs::read(module).map_err(|e| format!("{module}: {e}"))?;
    match mode.as_str() {
        "exports" => {
            let out = exported_only(&bytes, &args[2..])?;
            std::fs::write(module, out).map_err(|e| format!("{module}: {e}"))?;
        }
        _ => return Err(usage()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[wasm] {e}");
            ExitCode::FAILURE
        }
    }
}
