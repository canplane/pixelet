// pxl-tool/fmt.rs

//! Generated files are written as the formatters leave them, since `deno fmt
//! --check` and `cargo fmt --check` are gates: each by the config of the tree
//! it lies in, found from the file up, as that tree's own check reads it. A
//! file under no tree's config is written as generated.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::root;

pub fn formatted(path: &str, text: String) -> Result<String, String> {
    let mut cmd = if path.ends_with(".rs") {
        let mut c = Command::new("rustfmt");
        c.args(["--edition", "2021", "--emit", "stdout", "--config-path"]).arg(root().join("rust/rustfmt.toml"));
        c
    } else if let Some(ext) = [".ts", ".json"].into_iter().find(|e| path.ends_with(e)) {
        let root = root();
        let file = root.join(path);
        let Some(tree) = file
            .ancestors()
            .skip(1)
            .take_while(|d| d.starts_with(&root))
            .find(|d| d.join("deno.jsonc").exists() || d.join("deno.json").exists())
        else {
            return Ok(text);
        };
        let mut c = Command::new("deno");
        c.args(["fmt", "--ext", &ext[1..], "-"]).current_dir(tree);
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
