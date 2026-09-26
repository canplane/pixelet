// pxl-tool/fmt.rs

//! A generated file as the tree's formatters leave it. `deno fmt --check` and
//! `cargo fmt --check` are both gates, so a generator that wrote its output
//! unformatted would make the next unrelated run of either fail, and one
//! whose output a formatter then changed would be stale by its own check.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::root;

/// `text` as rustfmt leaves it for a `.rs` path and as deno fmt does for a
/// `.ts` one; any other path's text as it is.
pub fn formatted(path: &str, text: String) -> Result<String, String> {
    let mut cmd = if path.ends_with(".rs") {
        let mut c = Command::new("rustfmt");
        c.args(["--edition", "2021", "--emit", "stdout", "--config-path"]).arg(root().join("rustfmt.toml"));
        c
    } else if path.ends_with(".ts") {
        let mut c = Command::new("deno");
        c.args(["fmt", "--ext", "ts", "-"]).current_dir(root());
        c
    } else {
        return Ok(text);
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("fmt {path}: {e}"))?;
    child.stdin.take().ok_or("no stdin")?.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("fmt {path}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("fmt {path}: {e}"))
}
