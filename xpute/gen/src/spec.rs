// xpute-gen/spec.rs

//! This generates the named values of a data spec in TypeScript, Rust and C++
//! (data.rs), and holds what every generator writes with: an output, the
//! write or check of a tree's generated files, and their header. It links none
//! of the crates it writes into.

pub mod data;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::thread;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::fmt::format_text;

pub struct Output {
    pub path: String,
    pub text: String,
}

impl Output {
    pub fn new(path: &str, text: String) -> Output {
        Output {
            path: path.to_string(),
            text,
        }
    }
}

/// A tool's `generate` over a program's outputs, their paths relative to `root`.
/// `others` lists the trees whose generated files another generator writes,
/// and this one never deletes a file under them.
pub fn generate_at(
    root: &Path,
    outputs: Vec<Output>,
    others: &[&str],
    write: bool,
) -> Result<(), String> {
    let orphans = orphans(root, &outputs, others)?;
    for o in &orphans {
        if write {
            std::fs::remove_file(root.join(o)).map_err(|e| format!("{o}: {e}"))?;
            println!("deleted {o}");
        } else {
            eprintln!("spec: {o} says it is generated, but nothing writes it — generate it again");
        }
    }
    let applied = apply_at(root, outputs, write);
    if write || orphans.is_empty() {
        return applied;
    }
    let orphaned = format!("{} orphaned", orphans.len());
    Err(applied
        .err()
        .map_or(orphaned.clone(), |e| format!("{e}, {orphaned}")))
}

/// Every generated file carries this text on one of its first lines, after the
/// comment marker (`mark`).
const MARK: &str = "GENERATED from ";

/// `orphans` answers the tracked files under `root`, outside `others`, that
/// carry `MARK` but that no output writes.
fn orphans(root: &Path, outputs: &[Output], others: &[&str]) -> Result<Vec<String>, String> {
    let listed = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
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
        .filter(|p| !others.iter().any(|o| p.starts_with(o)))
        .filter(|p| marked(&root.join(p)))
        .map(str::to_string)
        .collect())
}

fn marked(path: &Path) -> bool {
    let mut head = Vec::new();
    let read = File::open(path).and_then(|f| f.take(1024).read_to_end(&mut head));
    read.is_ok()
        && String::from_utf8_lossy(&head)
            .lines()
            .take(5)
            .any(|l| l.trim_start_matches(['/', '#', '*', ' ']).starts_with(MARK))
}

/// With `write`, writes every output that differs from its file on disk, the
/// paths relative to `root`. Without it, fails where one differs.
pub fn apply_at(root: &Path, outputs: Vec<Output>, write: bool) -> Result<(), String> {
    let outputs = thread::scope(|scope| {
        let handles: Vec<_> = outputs
            .into_iter()
            .map(|o| {
                let at = root.join(&o.path).to_string_lossy().into_owned();
                scope.spawn(move || {
                    format_text(&at, o.text).map(|text| Output { path: o.path, text })
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
    let stale: Vec<&Output> = outputs
        .iter()
        .filter(|o| {
            std::fs::read_to_string(root.join(&o.path)).ok().as_deref() != Some(o.text.as_str())
        })
        .collect();
    if write {
        // Only the files that differ are written: rewriting a file moves its
        // mtime, which makes cargo and Vite rebuild.
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
            eprintln!("spec: {s} is not what its source writes — generate it again");
        }
        return Err(format!("{} stale", stale.len()));
    }
    println!(
        "spec: {} generated files are what their sources write",
        outputs.len()
    );
    Ok(())
}

/// A JSON value that keeps an object's keys in file order, since that order
/// numbers enums and orders tables; `serde_json::Value` sorts its keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Node>),
    Obj(Vec<(String, Node)>),
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Node, D::Error> {
        d.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }
    fn visit_bool<E>(self, v: bool) -> Result<Node, E> {
        Ok(Node::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Node, E> {
        Ok(Node::Num(v as f64))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Node, E> {
        Ok(Node::Num(v as f64))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Node, E> {
        Ok(Node::Num(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Node, E> {
        Ok(Node::Str(v.to_string()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut out = Vec::new();
        while let Some(n) = seq.next_element()? {
            out.push(n);
        }
        Ok(Node::Arr(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut out: Vec<(String, Node)> = Vec::new();
        while let Some((k, v)) = map.next_entry::<String, Node>()? {
            match out.iter_mut().find(|(key, _)| *key == k) {
                Some(slot) => slot.1 = v,
                None => out.push((k, v)),
            }
        }
        Ok(Node::Obj(out))
    }
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn entries(&self) -> &[(String, Node)] {
        match self {
            Node::Obj(entries) => entries,
            _ => &[],
        }
    }

    pub fn named(&self) -> impl Iterator<Item = (&str, &Node)> {
        self.entries()
            .iter()
            .filter(|(k, _)| !k.starts_with('$'))
            .map(|(k, v)| (k.as_str(), v))
    }

    pub fn is_obj(&self) -> bool {
        matches!(self, Node::Obj(_))
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Node::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn arr(&self) -> Option<&[Node]> {
        match self {
            Node::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_wrapper(&self) -> bool {
        self.get("$val").is_some()
    }

    pub fn val(&self) -> &Node {
        self.get("$val").unwrap_or(self)
    }

    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::str)
    }

    pub fn lines_at(&self, key: &str) -> Option<Vec<String>> {
        self.get(key)?.arr().map(|a| {
            a.iter()
                .filter_map(|l| l.str().map(str::to_string))
                .collect()
        })
    }
}

pub fn read(path: &str) -> Result<Node, String> {
    read_as(path)
}

pub fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

pub fn js_num(n: f64) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let magnitude = n.abs();
    if (1e-6..1e21).contains(&magnitude) {
        return format!("{n}");
    }
    let text = format!("{n:e}");
    match text.split_once('e') {
        Some((mantissa, exp)) if !exp.starts_with('-') => format!("{mantissa}e+{exp}"),
        _ => text,
    }
}

pub fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

pub fn ts_doc(doc: &[String], indent: &str) -> Vec<String> {
    if doc.len() == 1 {
        return vec![format!("{indent}/** {} */", doc[0])];
    }
    doc.iter()
        .enumerate()
        .map(|(i, l)| {
            let body = if i == 0 {
                format!("{indent}/** {l}")
            } else if l.is_empty() {
                format!("{indent} *")
            } else {
                format!("{indent} * {l}")
            };
            if i == doc.len() - 1 {
                format!("{body} */")
            } else {
                body
            }
        })
        .collect()
}

pub fn rs_doc(doc: &[String], indent: &str, mark: &str) -> Vec<String> {
    doc.iter()
        .map(|l| {
            if l.is_empty() {
                format!("{indent}{mark}")
            } else {
                format!("{indent}{mark} {l}")
            }
        })
        .collect()
}

pub fn line_comments(doc: &[String], indent: &str) -> Vec<String> {
    doc.iter()
        .map(|l| {
            if l.is_empty() {
                format!("{indent}//")
            } else {
                format!("{indent}// {l}")
            }
        })
        .collect()
}

/// A name's words, which every casing joins. A word breaks where a lower case
/// letter, or a digit after one, meets an upper case one, and at any mark
/// between words; a word written in upper case (`E2BIG`) stays whole. A number
/// after a lower case letter and one upper case letter that ends a word are a
/// word of their own (`maxTextureDimension1D`, `max_texture_dimension_1d`).
pub fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut lower = false;
    for (i, &c) in chars.iter().enumerate() {
        let leads = c.is_ascii_digit()
            && i > 0
            && chars[i - 1].is_ascii_lowercase()
            && {
                let mut rest = chars[i..].iter().skip_while(|d| d.is_ascii_digit());
                rest.next().is_some_and(char::is_ascii_uppercase)
                    && !rest.next().is_some_and(char::is_ascii_lowercase)
            };
        if !c.is_ascii_alphanumeric() || (c.is_ascii_uppercase() && lower) || leads {
            out.extend((!word.is_empty()).then(|| std::mem::take(&mut word)));
        }
        if c.is_ascii_alphanumeric() {
            lower = !leads && (c.is_ascii_lowercase() || (c.is_ascii_digit() && lower));
            word.push(c);
        } else {
            lower = false;
        }
    }
    out.extend((!word.is_empty()).then_some(word));
    out
}

/// A word with its first letter in upper case and the rest in lower case.
fn title(w: &str) -> String {
    let mut c = w.chars();
    c.next().map_or(String::new(), |f| f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase())
}

/// A name in snake case (`tile_cells`).
pub fn snake(name: &str) -> String {
    words(name).join("_").to_ascii_lowercase()
}

/// A name in SCREAMING snake case (`TILE_CELLS`).
pub fn screaming(name: &str) -> String {
    words(name).join("_").to_ascii_uppercase()
}

/// A name in Pascal case (`TileCells`).
pub fn pascal(name: &str) -> String {
    words(name).iter().map(|w| title(w)).collect()
}

/// A name's plural, as English forms it and in the name's own case
/// (`STEP_NAMES`, `PRIMITIVE_TOPOLOGIES`, `STORAGE_TEXTURE_ACCESSES`).
pub fn plural(name: &str) -> String {
    let upper = name.ends_with(|c: char| c.is_ascii_uppercase());
    let lower = name.to_ascii_lowercase();
    let (stem, end) = match lower.strip_suffix('y') {
        Some(rest) if !rest.ends_with(['a', 'e', 'i', 'o', 'u']) => (&name[..rest.len()], "ies"),
        _ if ["s", "x", "z", "ch", "sh"].iter().any(|e| lower.ends_with(e)) => (name, "es"),
        _ => (name, "s"),
    };
    let end = if upper { end.to_ascii_uppercase() } else { end.to_string() };
    format!("{stem}{end}")
}

/// Rust's keywords, strict and reserved, which an identifier takes as `r#`.
const RS_KEYWORDS: [&str; 50] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override",
    "priv", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
    "try", "type", "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub fn rs_ident(k: &str) -> String {
    let id: String = k
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if RS_KEYWORDS.contains(&id.as_str()) {
        format!("r#{id}")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

/// A record's field or a module as Rust names it: its name in snake case.
pub fn rs_field(k: &str) -> String {
    rs_ident(&snake(k))
}

/// A constant as Rust names it: its name in SCREAMING snake case.
pub fn rs_const(k: &str) -> String {
    rs_ident(&screaming(k))
}

/// An enum's member as Rust names it: its name in Pascal case.
pub fn rs_member(k: &str) -> String {
    rs_ident(&pascal(k))
}

/// Whether `k` is a TypeScript identifier as it is written.
pub fn is_ts_ident(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_' || f == '$')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
}

/// A module cannot export a constant under these words.
pub const TS_RESERVED: [&str; 46] = [
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// `k` as a TypeScript identifier: what no identifier holds becomes `_`, a
/// leading digit gets an `_` before it, and a reserved word one after it.
pub fn ts_ident(k: &str) -> String {
    let id: String = k
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if TS_RESERVED.contains(&id.as_str()) {
        format!("{id}_")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

/// An enum's member: its identifier in the language written, its name as its source spells it,
/// its number, and its doc.
pub struct Member {
    pub ident: String,
    pub name: String,
    pub number: String,
    pub doc: Vec<String>,
}

/// An enum's Rust, as every enum has it: its members, held in `repr`, with
/// `ALL`, `of` (a number to its member) and `name` (a member to its name), at
/// `indent`.
pub fn rs_enum(e: &str, members: &[Member], repr: &str, indent: &str) -> Vec<String> {
    let mut out = vec![
        format!("{indent}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"),
        format!("{indent}#[repr({repr})]"),
        format!("{indent}pub enum {e} {{"),
    ];
    for m in members {
        out.extend(rs_doc(&m.doc, &format!("{indent}    "), "///"));
        out.push(format!("{indent}    {} = {},", m.ident, m.number));
    }
    out.extend([
        format!("{indent}}}"),
        String::new(),
        format!("{indent}impl {e} {{"),
        format!("{indent}    /// Every member, in its source's own order."),
        format!("{indent}    pub const ALL: [{e}; {}] = [", members.len()),
    ]);
    out.extend(members.iter().map(|m| format!("{indent}        {e}::{},", m.ident)));
    out.extend([
        format!("{indent}    ];"),
        String::new(),
        format!("{indent}    /// The member `val` is, or none for a number no member has."),
        format!("{indent}    pub const fn of(val: {repr}) -> Option<{e}> {{"),
        format!("{indent}        match val {{"),
    ]);
    out.extend(members.iter().map(|m| format!("{indent}            {} => Some({e}::{}),", m.number, m.ident)));
    out.extend([
        format!("{indent}            _ => None,"),
        format!("{indent}        }}"),
        format!("{indent}    }}"),
        String::new(),
        format!("{indent}    /// The member's name, as its source spells it."),
        format!("{indent}    pub const fn name(self) -> &'static str {{"),
        format!("{indent}        match self {{"),
    ]);
    out.extend(
        members
            .iter()
            .map(|m| format!("{indent}            {e}::{} => {},", m.ident, json_str(&m.name))),
    );
    out.extend([
        format!("{indent}        }}"),
        format!("{indent}    }}"),
        format!("{indent}}}"),
    ]);
    out
}

/// An enum's TypeScript, as every enum has it: its type, the union of its
/// members' numbers, then each member as a constant of its own, named by the
/// enum and the member in SCREAMING snake case, which a reader imports by
/// name, and the member a number is (`ts_of`), as Rust's `of`. No object holds
/// them, so a bundle's tree shaking keeps no name.
pub fn ts_enum(e: &str, members: &[Member]) -> Vec<String> {
    let union: Vec<String> = members
        .iter()
        .map(|m| format!("typeof {}", ts_const(e, m)))
        .collect();
    let mut out = vec![String::new(), format!("export type {e} = {};", union.join(" | "))];
    for m in members {
        out.extend(ts_doc(&m.doc, ""));
        out.push(format!("export const {} = {};", ts_const(e, m), m.number));
    }
    let tests: Vec<String> = members.iter().map(|m| format!("val === {}", ts_const(e, m))).collect();
    out.extend([
        String::new(),
        "/** The member `val` is, or null for a number no member has. */".into(),
        format!("export function {}(val: number): {e} | null {{", ts_of(e)),
        format!("  return {} ? val : null;", tests.join(" || ")),
        "}".into(),
    ]);
    out
}

/// The TypeScript twin of a Rust enum's `of`, of the enum `e`: the member a
/// number is (`of_Tool`).
pub fn ts_of(e: &str) -> String {
    format!("of_{}", pascal(e))
}

/// The TypeScript constant of the enum `e`'s member `m`: both their names in
/// SCREAMING snake case (`TOOL_ITEM`).
pub fn ts_const(e: &str, m: &Member) -> String {
    format!("{}_{}", screaming(e), screaming(&m.name))
}

pub fn align_star(line: String, indent: &str) -> String {
    match line
        .strip_prefix(indent)
        .and_then(|rest| rest.strip_prefix('*'))
    {
        Some(rest) => format!("{indent} *{rest}"),
        None => line,
    }
}

/// A generated file's mark: `MARK`, what it is written from, and by what,
/// which each writer puts behind its file's comment marker on one of its first
/// lines, and by which `orphans` finds the file.
pub fn mark(src: &str, tool: &str) -> String {
    format!("{MARK}{src} ({tool})")
}

pub fn file(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}

/// The package `path` from `root` lies in: its name, from the nearest
/// Cargo.toml or package.json above it, and its directory from `root`.
pub fn package<'p>(root: &Path, path: &'p str) -> Result<(String, &'p Path), String> {
    for dir in Path::new(path).ancestors().skip(1) {
        let cargo = root.join(dir).join("Cargo.toml");
        let npm = root.join(dir).join("package.json");
        let name = if cargo.is_file() {
            let text = std::fs::read_to_string(&cargo)
                .map_err(|e| format!("{}: {e}", cargo.display()))?;
            let package = text.split("[package]").nth(1).unwrap_or("");
            package
                .lines()
                .take_while(|l| !l.starts_with('['))
                .find_map(|l| l.trim().strip_prefix("name = "))
                .map(|n| n.trim_matches('"').to_string())
                .ok_or(format!("{}: no package name", cargo.display()))?
        } else if npm.is_file() {
            let json: serde_json::Value = read_as(&npm.to_string_lossy())?;
            let name = json["name"].as_str();
            name.ok_or(format!("{}: no name", npm.display()))?.to_string()
        } else {
            continue;
        };
        return Ok((name, dir));
    }
    Err(format!("{path}: under no package"))
}

/// `package_path` names a generated file, `path` from `root`, as its package
/// takes it: the package's name, then its path under the package's `src/`,
/// or under the package for a file outside `src/`.
pub fn package_path(root: &Path, path: &str) -> Result<String, String> {
    let (name, dir) = package(root, path)?;
    let file = Path::new(path);
    let under = file
        .strip_prefix(dir.join("src"))
        .or_else(|_| file.strip_prefix(dir))
        .unwrap_or(file);
    Ok(format!("{name}/{}", under.to_string_lossy()))
}

