// xpute-gen/fmt.rs

//! A generated file is formatted before it is written, because
//! `prettier --check` and `cargo fmt --check` gate the trees. A Rust file goes
//! through rustfmt. A TypeScript, JSON or JSON5 file takes the prettier of the pnpm
//! tree it lies in, found by walking up from the file, so that it reads as that
//! tree's own check expects. A file under no tree is written as generated.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn format_text(path: &str, text: String) -> Result<String, String> {
    let (mut cmd, formatter) = if path.ends_with(".rs") {
        let mut c = Command::new("rustfmt");
        c.args(["--edition", "2024", "--emit", "stdout"]);
        (c, "rustfmt".to_string())
    } else if [".ts", ".json", ".json5"].iter().any(|e| path.ends_with(e)) {
        let file = Path::new(path);
        let Some(tree) = file
            .ancestors()
            .skip(1)
            .find(|d| d.join("pnpm-workspace.yaml").exists())
        else {
            return Ok(text);
        };
        let prettier = tree.join("node_modules/.bin/prettier");
        let mut c = Command::new(&prettier);
        c.arg("--stdin-filepath").arg(file).current_dir(tree);
        (c, prettier.display().to_string())
    } else {
        return Ok(text);
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("fmt {path}: {formatter}: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "fmt {path}: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("fmt {path}: {e}"))
}
