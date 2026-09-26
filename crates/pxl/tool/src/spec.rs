// pxl-tool/spec.rs

//! The specs are JSON, and the TypeScript and the Rust that carry them are
//! written from it: neither language is where a number or a name is decided.
//! `deno task spec` writes every generated file, and `deno task check:spec`
//! (part of `deno task check`) fails where one on disk is not what its JSON
//! writes.
//!
//! A generated file is imported as any module is — the client's bundler does
//! not take a JSON import from outside its root — and is never edited by
//! hand.
//!
//! It links none of the crates it writes into (pxl-tool's Cargo.toml says
//! why), so everything it knows of the specs is what it reads here.

pub mod bake;
pub mod cell;
pub mod cmd;
pub mod data;
pub mod route;
pub mod setting;
pub mod xpute;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::root;

/// One generated file: its path from the repository's root, and its text
/// before the formatter has seen it.
pub struct Output {
    pub path: String,
    pub text: String,
}

impl Output {
    pub fn new(path: &str, text: String) -> Output {
        Output { path: path.to_string(), text }
    }
}

/// Every generated file, each generator's in the order it writes them.
pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(xpute::errno_outputs()?);
    out.extend(xpute::fetch_outputs()?);
    out.extend(cell::color_outputs()?);
    out.extend(cell::cube_type_outputs()?);
    out.extend(cell::decor_outputs()?);
    out.extend(bake::published_outputs()?);
    out.extend(bake::poi_badge_outputs()?);
    out.extend(data::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
}

// ---- the JSON ----

/// A JSON value with its objects in the order the file writes them, which is
/// the order a generated enum numbers its members in and a table lists its
/// rows in. `serde_json::Value` sorts its keys unless a feature every crate
/// in the workspace would then share says otherwise. A spec whose objects
/// are records rather than tables — cmd.json, errno.json, fetch.json — is
/// read into types of its own instead (`read_as`).
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

    /// An object's entries, the `$` keys that annotate it among them.
    pub fn entries(&self) -> &[(String, Node)] {
        match self {
            Node::Obj(entries) => entries,
            _ => &[],
        }
    }

    /// An object's entries less the `$` keys, which annotate rather than name.
    pub fn named(&self) -> impl Iterator<Item = (&str, &Node)> {
        self.entries().iter().filter(|(k, _)| !k.starts_with('$')).map(|(k, v)| (k.as_str(), v))
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

    /// A leaf that carries its reason: `{ "$value": …, "$doc": [lines] }`.
    pub fn is_wrapper(&self) -> bool {
        self.get("$value").is_some()
    }

    /// A value, through the wrapper that carries its reason where it has one.
    pub fn value(&self) -> &Node {
        self.get("$value").unwrap_or(self)
    }

    /// A string field of an object, where it has one.
    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::str)
    }

    /// A list of lines, as every `doc` and `$doc` is written.
    pub fn lines_at(&self, key: &str) -> Option<Vec<String>> {
        self.get(key)?.arr().map(|a| a.iter().filter_map(|l| l.str().map(str::to_string)).collect())
    }
}

/// A spec file, read from the repository's root.
pub fn read(path: &str) -> Result<Node, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// A spec file into a type of its own, for one whose objects are records
/// rather than ordered tables.
pub fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// A number as a JavaScript literal writes it, and as TypeScript and Rust
/// both read it back: an integral one with no point, a very large or very
/// small one with an exponent.
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

/// A string as a JSON literal, which is also a TypeScript and a Rust one for
/// everything a spec says.
pub fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// A JSON value laid out two spaces an indent, as `JSON.stringify(v, null, 2)`
/// lays it out.
pub fn pretty(n: &Node, indent: &str, out: &mut String) {
    match n {
        Node::Null => out.push_str("null"),
        Node::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Node::Num(v) => out.push_str(&js_num(*v)),
        Node::Str(s) => out.push_str(&json_str(s)),
        Node::Arr(a) if a.is_empty() => out.push_str("[]"),
        Node::Obj(o) if o.is_empty() => out.push_str("{}"),
        Node::Arr(a) => {
            let inner = format!("{indent}  ");
            out.push_str("[\n");
            for (i, v) in a.iter().enumerate() {
                out.push_str(&inner);
                pretty(v, &inner, out);
                out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
            }
            out.push_str(indent);
            out.push(']');
        }
        Node::Obj(o) => {
            let inner = format!("{indent}  ");
            out.push_str("{\n");
            for (i, (k, v)) in o.iter().enumerate() {
                out.push_str(&inner);
                out.push_str(&json_str(k));
                out.push_str(": ");
                pretty(v, &inner, out);
                out.push_str(if i + 1 < o.len() { ",\n" } else { "\n" });
            }
            out.push_str(indent);
            out.push('}');
        }
    }
}

// ---- how the two languages are written ----

/// A doc comment in TypeScript: one line on one line, several as a block.
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

/// A doc comment in Rust, `///` or a module's `//!`.
pub fn rs_doc(doc: &[String], indent: &str, mark: &str) -> Vec<String> {
    doc.iter().map(|l| if l.is_empty() { format!("{indent}{mark}") } else { format!("{indent}{mark} {l}") }).collect()
}

/// Comment lines, `//` each, a blank one bare.
pub fn line_comments(doc: &[String], indent: &str) -> Vec<String> {
    doc.iter().map(|l| if l.is_empty() { format!("{indent}//") } else { format!("{indent}// {l}") }).collect()
}

/// What a generated Rust file says above the `allow` that suspends the
/// casing lints. A reader who meets one of these files otherwise has to guess
/// whether the attribute is a reason or an oversight.
pub const RS_CASING_NOTE: [&str; 3] = [
    "// Casing here is the spec's and not Rust's: a name is spelled as the JSON",
    "// spells it, so that one record is indexed by one name on both sides. To",
    "// case them the way Rust would is to rename the protocol.",
];

pub fn casing_note() -> Vec<String> {
    RS_CASING_NOTE.iter().map(|l| l.to_string()).collect()
}

/// `PlacePart` as a function's name spells it.
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in name.chars() {
        if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) {
            out.push('_');
        }
        out.push(c);
        prev = Some(c);
    }
    out.to_lowercase()
}

/// `PLACE_PART` or `place_part` as a type's name spells it.
pub fn pascal(name: &str) -> String {
    name.to_lowercase()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// An enum's name: the set's name, singular, as cube_type.json's `Land` and
/// decor.json's `Decor` are.
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
    "type", "move", "mod", "fn", "struct", "enum", "match", "loop", "ref", "self", "super", "crate", "use", "where", "impl", "trait", "static", "const", "let", "mut", "in", "for", "if", "else",
    "while", "return", "true", "false", "as", "box", "dyn", "pub", "unsafe", "async", "await", "break", "continue", "extern", "priv", "virtual", "yield", "try", "macro", "abstract", "become", "do",
    "final", "override", "typeof", "unsized",
];

/// A JSON name as a Rust identifier: a keyword raw, a leading digit behind an
/// underscore, anything else an identifier cannot hold as an underscore.
pub fn rs_ident(k: &str) -> String {
    let id: String = k.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if RS_KEYWORDS.contains(&id.as_str()) {
        format!("r#{id}")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

/// `#RRGGBB` as the one number it is. The JSON is written in hex because a
/// person picks a color that way; nothing downstream reads it back as text.
pub fn packed(hex: &str) -> Result<u32, String> {
    let digits = hex.strip_prefix('#').filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()));
    digits.and_then(|d| u32::from_str_radix(d, 16).ok()).ok_or(format!("color JSON: \"{hex}\" is not #RRGGBB"))
}

/// A `$type: "rgb"` leaf: written `#RRGGBB` because a person picks a color
/// that way, emitted as the one number it is because nothing that lights or
/// blends with it wants text.
pub fn rgb_literal(hex: &str) -> Result<String, String> {
    Ok(format!("0x{:06x}", packed(hex)?))
}

/// A line of a doc block that opens on `indent` then `*`, pushed in a space so
/// its star lines up under the `/**` above it.
pub fn align_star(line: String, indent: &str) -> String {
    match line.strip_prefix(indent).and_then(|rest| rest.strip_prefix('*')) {
        Some(rest) => format!("{indent} *{rest}"),
        None => line,
    }
}

/// Lines joined as a file is: one to a line, and a newline at the end.
pub fn file(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}
