// xpute-tool/spec.rs

//! Writes the TypeScript, Rust and C++ generated from spec/ JSON: a data
//! spec's named values (data.rs), a command table's readers and writers
//! (cmd.rs), and xpute's errno (errno.rs). It links none of the crates it
//! writes into.

pub mod cmd;
pub mod data;
pub mod errno;

use std::thread;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::fmt::format_text;
use crate::root;

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

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = cmd::outputs()?;
    out.extend(errno::outputs()?);
    out.extend(data::outputs()?);
    Ok(out)
}

/// Writes every generated file that differs from what its JSON writes, or,
/// not `write`, fails where one does.
pub fn generate(write: bool) -> Result<(), String> {
    apply(outputs()?, write)
}

/// Writes every output that differs from the file on disk, or, not `write`,
/// fails where one does.
pub fn apply(outputs: Vec<Output>, write: bool) -> Result<(), String> {
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

/// A JSON value in file order, which numbers enums and orders tables;
/// `serde_json::Value` sorts its keys.
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

    pub fn value(&self) -> &Node {
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
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
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

pub const RS_CASING_NOTE: [&str; 1] =
    ["// Names as the spec spells them, so one record has one name on every side."];

pub fn casing_note() -> Vec<String> {
    RS_CASING_NOTE.iter().map(|l| l.to_string()).collect()
}

pub fn snake(name: &str) -> String {
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in name.chars() {
        if c.is_ascii_uppercase()
            && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        {
            out.push('_');
        }
        out.push(c);
        prev = Some(c);
    }
    out.to_lowercase()
}

pub fn pascal(name: &str) -> String {
    name.to_lowercase()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

pub fn enum_name(name: &str) -> String {
    let e = pascal(name);
    if let Some(stem) = e.strip_suffix("ies") {
        format!("{stem}y")
    } else if e.ends_with('s') && !e.ends_with("ss") {
        e[..e.len() - 1].to_string()
    } else {
        e
    }
}

const RS_KEYWORDS: [&str; 50] = [
    "type", "move", "mod", "fn", "struct", "enum", "match", "loop", "ref", "self", "super",
    "crate", "use", "where", "impl", "trait", "static", "const", "let", "mut", "in", "for", "if",
    "else", "while", "return", "true", "false", "as", "box", "dyn", "pub", "unsafe", "async",
    "await", "break", "continue", "extern", "priv", "virtual", "yield", "try", "macro", "abstract",
    "become", "do", "final", "override", "typeof", "unsized",
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

pub fn align_star(line: String, indent: &str) -> String {
    match line
        .strip_prefix(indent)
        .and_then(|rest| rest.strip_prefix('*'))
    {
        Some(rest) => format!("{indent} *{rest}"),
        None => line,
    }
}

/// A generated file's line naming the JSON it is generated from.
pub fn generated(json: &str) -> String {
    format!("// GENERATED from {json}")
}

pub fn file(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}
