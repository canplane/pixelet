// pxl-tool/spec.rs

//! Writes the TypeScript and Rust generated from `spec/` JSON. It links none
//! of the crates it writes into, so it builds when their generated code is broken.

pub mod bake;
pub mod block;
pub mod cmd;
pub mod data;
pub mod route;
pub mod setting;
pub mod xpute;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::root;

pub struct Output {
    pub path: String,
    pub text: String,
}

impl Output {
    pub fn new(path: &str, text: String) -> Output {
        Output { path: path.to_string(), text }
    }
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(xpute::errno_outputs()?);
    out.extend(block::color_outputs()?);
    out.extend(block::cube_outputs()?);
    out.extend(block::decor_outputs()?);
    out.extend(bake::published_outputs()?);
    out.extend(bake::object_outputs()?);
    out.extend(bake::place_theme_outputs()?);
    out.extend(data::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
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

    pub fn is_wrapper(&self) -> bool {
        self.get("$value").is_some()
    }

    pub fn value(&self) -> &Node {
        self.get("$value").unwrap_or(self)
    }

    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::str)
    }

    pub fn lines_at(&self, key: &str) -> Option<Vec<String>> {
        self.get(key)?.arr().map(|a| a.iter().filter_map(|l| l.str().map(str::to_string)).collect())
    }
}

/// A table with `"$packed": true` is laid end to end: each field gives its
/// width in words, and the generator writes where it begins and the table's
/// `WORDS` where the last ends; a section inside is a packed table given its
/// `AT`. A width is a number or a product of terms (`"2 * CENSUS_TAGS"`), a
/// term a number, a list or a packed table above it in the file (its length,
/// its `WORDS`) or a number in one (`LIMITS.ZOOMS`, `op.DESTROY`), else one
/// in a file it uses (`used`). So a lane grown moves everything after it.
pub fn laid_out(node: Node, used: &[(String, Node)]) -> Result<Node, String> {
    let Node::Obj(entries) = node else { return Ok(node) };
    let mut above: Vec<(String, Node)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let v = match v {
            Node::Obj(ref t) if t.iter().any(|(f, x)| f == "$packed" && *x == Node::Bool(true)) => Node::Obj(lay(t.clone(), &above, used).map_err(|e| format!("{k}.{e}"))?),
            v => v,
        };
        above.push((k, v));
    }
    Ok(Node::Obj(above))
}

fn lay(entries: Vec<(String, Node)>, above: &[(String, Node)], used: &[(String, Node)]) -> Result<Vec<(String, Node)>, String> {
    let mut at = 0.0;
    let mut out = Vec::with_capacity(entries.len() + 1);
    for (k, v) in entries {
        if k.starts_with('$') {
            out.push((k, v));
            continue;
        }
        if k == "WORDS" || k == "AT" {
            return Err(format!("{k}: a packed table's {k} is laid out, not written"));
        }
        match v {
            Node::Obj(section) => {
                let mut section = lay(section, above, used).map_err(|e| format!("{k}.{e}"))?;
                let words = words_of(&section).unwrap_or(0.0);
                let first = section.iter().take_while(|(f, _)| f.starts_with('$')).count();
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
    table.iter().find(|(f, _)| f == "WORDS").and_then(|(_, w)| w.num())
}

fn width(v: &Node, above: &[(String, Node)], used: &[(String, Node)]) -> Result<f64, String> {
    let named = |name: &str| above.iter().chain(used).find(|(k, _)| k == name).map(|(_, n)| n);
    let term = |t: &str| -> Result<f64, String> {
        let t = t.trim();
        if let Ok(n) = t.parse::<f64>() {
            return Ok(n);
        }
        let found = match t.split_once('.') {
            Some((table, field)) => named(table).and_then(|n| n.get(field)).and_then(|f| f.value().num()),
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
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
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
    doc.iter().map(|l| if l.is_empty() { format!("{indent}{mark}") } else { format!("{indent}{mark} {l}") }).collect()
}

pub fn line_comments(doc: &[String], indent: &str) -> Vec<String> {
    doc.iter().map(|l| if l.is_empty() { format!("{indent}//") } else { format!("{indent}// {l}") }).collect()
}

pub const RS_CASING_NOTE: [&str; 3] = [
    "// Casing here is the spec's and not Rust's: a name is spelled as the JSON",
    "// spells it, so that one record is indexed by one name on both sides. To",
    "// case them the way Rust would is to rename the protocol.",
];

pub fn casing_note() -> Vec<String> {
    RS_CASING_NOTE.iter().map(|l| l.to_string()).collect()
}

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

pub fn pascal(name: &str) -> String {
    name.to_lowercase()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
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
    "type", "move", "mod", "fn", "struct", "enum", "match", "loop", "ref", "self", "super", "crate", "use", "where", "impl", "trait", "static", "const", "let", "mut", "in", "for", "if", "else",
    "while", "return", "true", "false", "as", "box", "dyn", "pub", "unsafe", "async", "await", "break", "continue", "extern", "priv", "virtual", "yield", "try", "macro", "abstract", "become", "do",
    "final", "override", "typeof", "unsized",
];

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

pub fn packed(hex: &str) -> Result<u32, String> {
    let digits = hex.strip_prefix('#').filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()));
    digits.and_then(|d| u32::from_str_radix(d, 16).ok()).ok_or(format!("color JSON: \"{hex}\" is not #RRGGBB"))
}

pub fn rgb_literal(hex: &str) -> Result<String, String> {
    Ok(format!("0x{:06x}", packed(hex)?))
}

pub fn align_star(line: String, indent: &str) -> String {
    match line.strip_prefix(indent).and_then(|rest| rest.strip_prefix('*')) {
        Some(rest) => format!("{indent} *{rest}"),
        None => line,
    }
}

/// A path as xpute's own repository spells it: `xpute/` is its root.
pub fn in_xpute(path: &str) -> &str {
    path.strip_prefix("xpute/").unwrap_or(path)
}

/// The "do not edit" line. What is generated from `xpute/spec/` ships in
/// xpute's own repository, where this tool is not, so it names only its JSON.
pub fn generated(json: &str, shown: &str) -> String {
    if json.starts_with("xpute/spec/") {
        format!("// GENERATED from {} — do not edit.", in_xpute(json))
    } else {
        format!("// GENERATED by pxl-tool/bin/spec.rs from {shown} — do not edit.")
    }
}

pub fn file(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}
