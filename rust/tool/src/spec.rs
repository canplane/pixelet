// pxl-tool/spec.rs

//! Writes the TypeScript and Rust generated from `spec/` JSON: its data
//! specs (data.rs), its command table (cmd.rs), and what else it generates
//! beside them. It links none of the crates it writes into, so it builds when
//! their generated code is broken.

pub mod asset;
pub(crate) mod bake;
pub mod catalog;
pub mod cmd;
pub mod data;
pub mod ingest;
pub mod route;
pub mod setting;

/// What xpute's generator writes and reads with, shared: the generated file,
/// the JSON's tree, and the text helpers.
pub use xpute_tool::spec::{
    align_star, casing_note, enum_name, file, js_num, json_str, line_comments, pascal, rs_doc,
    rs_ident, snake, ts_doc, Node, Output,
};

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::thread;

use serde::Deserialize;

use crate::fmt::format_text;
use crate::root;

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(asset::color_outputs()?);
    out.extend(asset::cube_outputs()?);
    out.extend(asset::decor_outputs()?);
    out.extend(bake::publication_outputs()?);
    out.extend(bake::object_outputs()?);
    out.extend(bake::place_theme_outputs()?);
    out.extend(data::outputs()?);
    out.extend(ingest::outputs()?);
    out.extend(catalog::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
}

/// Writes every generated file that differs from what its JSON writes and
/// deletes every one nothing writes any more, or, not `write`, fails where
/// one is either.
pub fn generate(write: bool) -> Result<(), String> {
    let outputs = outputs()?;
    let orphans = orphans(&outputs)?;
    let root = root();
    for o in &orphans {
        if write {
            std::fs::remove_file(root.join(o)).map_err(|e| format!("{o}: {e}"))?;
            println!("deleted {o}");
        } else {
            eprintln!("spec: {o} says it is generated, but nothing writes it — run `make spec`");
        }
    }
    let applied = apply(outputs, write);
    if write || orphans.is_empty() {
        return applied;
    }
    let orphaned = format!("{} orphaned", orphans.len());
    Err(applied
        .err()
        .map_or(orphaned.clone(), |e| format!("{e}, {orphaned}")))
}

/// What a file the generators write says on one of its first lines, behind
/// its comment's mark (`generated_line`).
const MARK: &str = "GENERATED from ";

/// The trees whose generated files another generator writes: xpute's, by
/// xpute-tool.
const OTHERS: [&str; 1] = ["xpute/"];

/// The tracked files that carry `MARK` and that no output writes.
fn orphans(outputs: &[Output]) -> Result<Vec<String>, String> {
    let root = root();
    let listed = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !listed.status.success() {
        return Err("git ls-files: not in a checkout".into());
    }
    let listed = String::from_utf8(listed.stdout).map_err(|e| format!("git ls-files: {e}"))?;
    let written: BTreeSet<&str> = outputs.iter().map(|o| o.path.as_str()).collect();
    Ok(listed
        .split('\0')
        .filter(|p| !p.is_empty() && !written.contains(p))
        .filter(|p| !OTHERS.iter().any(|o| p.starts_with(o)))
        .filter(|p| marked(&root.join(p)))
        .map(str::to_string)
        .collect())
}

/// Whether one of the file's first lines is `MARK`'s, behind a comment's mark.
fn marked(path: &Path) -> bool {
    let mut head = Vec::new();
    let read = File::open(path).and_then(|f| f.take(1024).read_to_end(&mut head));
    read.is_ok()
        && String::from_utf8_lossy(&head)
            .lines()
            .take(5)
            .any(|l| l.trim_start_matches(['/', '#', '*', ' ']).starts_with(MARK))
}

/// Writes every output that differs from the file on disk, or, not `write`,
/// fails where one does.
pub(crate) fn apply(outputs: Vec<Output>, write: bool) -> Result<(), String> {
    let outputs = thread::scope(|scope| {
        let handles: Vec<_> = outputs
            .into_iter()
            .map(|o| {
                scope.spawn(move || {
                    format_text(&o.path, o.text).map(|text| Output { path: o.path, text })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| "a formatter thread panicked".to_string())?
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let root = root();
    let stale: Vec<&Output> = outputs
        .iter()
        .filter(|o| {
            std::fs::read_to_string(root.join(&o.path)).ok().as_deref() != Some(o.text.as_str())
        })
        .collect();
    if write {
        // Only what differs: a rewrite moves the mtime, and cargo and Vite rebuild.
        for o in &stale {
            let at = root.join(&o.path);
            std::fs::create_dir_all(at.parent().ok_or("a path with no parent")?)
                .map_err(|e| e.to_string())?;
            std::fs::write(&at, &o.text).map_err(|e| format!("{}: {e}", o.path))?;
            println!("wrote {}", o.path);
        }
        return Ok(());
    }
    let stale: Vec<&str> = stale.iter().map(|o| o.path.as_str()).collect();
    if !stale.is_empty() {
        for s in &stale {
            eprintln!("spec: {s} is not what its JSON writes — run `make spec`");
        }
        return Err(format!("{} stale", stale.len()));
    }
    println!(
        "spec: {} generated files are what their JSON writes",
        outputs.len()
    );
    Ok(())
}

/// A table with `"$packed": true` is laid end to end: each field gives its
/// width in words, and the generator writes where it begins and the table's
/// `WORDS` where the last ends; a section inside is a packed table given its
/// `AT`. A width is a number or a product of terms (`"2 * CENSUS_TAGS"`), a
/// term a number, a list or a packed table above it in the file (its length,
/// its `WORDS`) or a number in one (`LIMIT.ZOOMS`, `op.DESTROY`), else one
/// in a file it uses (`used`). So a lane grown moves everything after it.
pub(crate) fn laid_out(node: Node, used: &[(String, Node)]) -> Result<Node, String> {
    let Node::Obj(entries) = node else {
        return Ok(node);
    };
    let mut above: Vec<(String, Node)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let v = match v {
            Node::Obj(ref t)
                if t.iter()
                    .any(|(f, x)| f == "$packed" && *x == Node::Bool(true)) =>
            {
                Node::Obj(lay(t.clone(), &above, used).map_err(|e| format!("{k}.{e}"))?)
            }
            v => v,
        };
        above.push((k, v));
    }
    Ok(Node::Obj(above))
}

fn lay(
    entries: Vec<(String, Node)>,
    above: &[(String, Node)],
    used: &[(String, Node)],
) -> Result<Vec<(String, Node)>, String> {
    let mut at = 0.0;
    let mut out = Vec::with_capacity(entries.len() + 1);
    for (k, v) in entries {
        if k.starts_with('$') {
            out.push((k, v));
            continue;
        }
        if k == "WORDS" || k == "AT" {
            return Err(format!(
                "{k}: a packed table's {k} is laid out, not written"
            ));
        }
        match v {
            Node::Obj(section) => {
                let mut section = lay(section, above, used).map_err(|e| format!("{k}.{e}"))?;
                let words = words_of(&section).unwrap_or(0.0);
                let first = section
                    .iter()
                    .take_while(|(f, _)| f.starts_with('$'))
                    .count();
                section.insert(first, ("AT".into(), Node::Num(at)));
                at += words;
                out.push((k, Node::Obj(section)));
            }
            v => {
                let width = width(&v, above, used).map_err(|e| format!("{k}: {e}"))?;
                out.push((k, Node::Num(at)));
                at += width;
            }
        }
    }
    out.push(("WORDS".into(), Node::Num(at)));
    Ok(out)
}

fn words_of(table: &[(String, Node)]) -> Option<f64> {
    table
        .iter()
        .find(|(f, _)| f == "WORDS")
        .and_then(|(_, w)| w.num())
}

fn width(v: &Node, above: &[(String, Node)], used: &[(String, Node)]) -> Result<f64, String> {
    let named = |name: &str| {
        above
            .iter()
            .chain(used)
            .find(|(k, _)| k == name)
            .map(|(_, n)| n)
    };
    let term = |t: &str| -> Result<f64, String> {
        let t = t.trim();
        if let Ok(n) = t.parse::<f64>() {
            return Ok(n);
        }
        let found = match t.split_once('.') {
            // A list in a table is as wide as it is long: `FRAME_GROUP.rays`.
            Some((table, field)) => named(table)
                .and_then(|n| n.get(field))
                .and_then(|f| match f {
                    Node::Arr(a) => Some(a.len() as f64),
                    _ => f.value().num(),
                }),
            None => named(t).and_then(|n| match n {
                Node::Arr(a) => Some(a.len() as f64),
                Node::Obj(_) if n.is_wrapper() => n.value().num(),
                Node::Obj(o) => words_of(o),
                _ => n.num(),
            }),
        };
        found.ok_or_else(|| format!("`{t}` names no width above it"))
    };
    // A product of terms, less any others: `op.DESTROY - 1` is the ops below DESTROY past the first.
    let product = |p: &str| p.split('*').map(term).product::<Result<f64, String>>();
    match v {
        Node::Num(n) => Ok(*n),
        Node::Str(s) => {
            let mut parts = s.split('-');
            let first = product(parts.next().unwrap_or_default())?;
            parts.try_fold(first, |w, p| Ok(w - product(p)?))
        }
        _ => Err("a width is a number or a product of names, less others".into()),
    }
}

pub fn read(path: &str) -> Result<Node, String> {
    read_as(path)
}

pub(crate) fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

pub(crate) fn packed(hex: &str) -> Result<u32, String> {
    let digits = hex
        .strip_prefix('#')
        .filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()));
    digits
        .and_then(|d| u32::from_str_radix(d, 16).ok())
        .ok_or(format!("color JSON: \"{hex}\" is not #RRGGBB"))
}

pub(crate) fn rgb_literal(hex: &str) -> Result<String, String> {
    Ok(format!("0x{:06x}", packed(hex)?))
}

/// A generated file's line naming the JSON it is generated from.
pub fn generated_line(json: &str) -> String {
    format!("// GENERATED from {json}")
}
