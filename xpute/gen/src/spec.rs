// xpute-gen/spec.rs

//! This generates the named values of a data spec in TypeScript, Rust and C++
//! (data.rs), and holds what every generator writes with: an output, the
//! write or check of a tree's generated files, and their header. It links none
//! of the crates it writes into.

pub mod data;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};

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

    /// Whether the node is a token (DTCG): a value with its `$value`.
    pub fn is_wrapper(&self) -> bool {
        self.get("$value").is_some()
    }

    /// A token's `$value`, or the node itself where it is a value as it is.
    pub fn val(&self) -> &Node {
        self.get("$value").unwrap_or(self)
    }

    /// The node's `$description`, line by line.
    pub fn doc(&self) -> Option<Vec<String>> {
        self.str_at("$description")
            .map(|d| d.lines().map(String::from).collect())
    }

    /// xpute's own mark `name`, under the node's `$extensions` (`MARKS`).
    pub fn mark(&self, name: &str) -> Option<&Node> {
        self.get("$extensions")?.get(MARKS)?.get(name)
    }

    /// The node, each token that carries nothing past its `$value` read as
    /// that value, at every depth.
    pub fn folded(self) -> Node {
        match self {
            Node::Obj(entries) if entries.len() == 1 && entries[0].0 == "$value" => entries
                .into_iter()
                .next()
                .map_or(Node::Null, |(_, v)| v.folded()),
            Node::Obj(entries) => {
                Node::Obj(entries.into_iter().map(|(k, v)| (k, v.folded())).collect())
            }
            Node::Arr(a) => Node::Arr(a.into_iter().map(Node::folded).collect()),
            n => n,
        }
    }

    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::str)
    }
}

/// The key under a node's `$extensions` that holds xpute's own marks; a spec
/// is otherwise written in DTCG's format (`Node::mark`).
pub const MARKS: &str = "xpute";

/// `lines` as a `$description` holds them, one string (`Node::doc`).
pub fn description<S: AsRef<str>>(lines: &[S]) -> Node {
    Node::Str(
        lines
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// xpute's own marks, as a node's `$extensions` entry holds them.
pub fn marks(marks: Vec<(String, Node)>) -> (String, Node) {
    (
        "$extensions".into(),
        Node::Obj(vec![(MARKS.into(), Node::Obj(marks))]),
    )
}

/// The spec at `path` (`typed`).
pub fn read(path: &str) -> Result<Node, String> {
    typed(parse(path)?).map_err(|e| format!("{path}: {e}"))
}

/// The spec `text`, which `path` names (`typed`).
pub fn read_str(path: &str, text: &str) -> Result<Node, String> {
    typed(parse_str(path, text)?).map_err(|e| format!("{path}: {e}"))
}

/// The spec at `path`, JSON5, as it is written.
fn parse(path: &str) -> Result<Node, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    parse_str(path, &text)
}

/// The spec `text`, JSON5, which `path` names, as it is written.
fn parse_str(path: &str, text: &str) -> Result<Node, String> {
    json5::from_str::<Node>(text).map_err(|e| format!("{path}: {e}"))
}

/// `spec`, read from a file or built by a tool, its tokens folded
/// (`Node::folded`), each number typed and an integer whole and within its
/// width (`check_widths`).
fn typed(spec: Node) -> Result<Node, String> {
    let folded = spec.folded();
    let spec = resolved(&folded, folded.clone())?;
    check_widths(&spec, "", None)?;
    Ok(spec)
}

/// Whether `s` is a reference to a token, DTCG's `"{group.token}"`: a path
/// of names, at least a group's and a token's, in braces.
fn is_ref(s: &str) -> bool {
    s.strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .is_some_and(|p| {
            p.contains('.')
                && p.split('.').all(|k| {
                    !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                })
        })
}

/// `n`, each value that is a reference to another token of `root`
/// (`is_ref`), read as that token's value; a reference to no token, or to
/// one that is no value, is refused.
fn resolved(root: &Node, n: Node) -> Result<Node, String> {
    Ok(match n {
        Node::Str(s) if is_ref(&s) => {
            let path = &s[1..s.len() - 1];
            let to = path
                .split('.')
                .try_fold(root, |n, k| n.get(k))
                .map(Node::val)
                .ok_or(format!("{s} names no token"))?;
            if to.is_obj() {
                return Err(format!("{s} names no value"));
            }
            to.clone()
        }
        Node::Obj(entries) => Node::Obj(
            entries
                .into_iter()
                .map(|(k, v)| Ok((k, resolved(root, v)?)))
                .collect::<Result<_, String>>()?,
        ),
        Node::Arr(a) => Node::Arr(
            a.into_iter()
                .map(|v| resolved(root, v))
                .collect::<Result<_, String>>()?,
        ),
        n => n,
    })
}

/// The type of a packed table's numbers (data.rs `laid_out`): the words each
/// field spans, and the offsets and the count laid out from them.
pub const PACKED_TYPE: &str = "usize";

/// Whether `n` is a table a spec lays out in words, marked `"packed": true`,
/// which only the spec's own entries are (data.rs `laid_out`).
pub fn is_packed(n: &Node) -> bool {
    n.mark("packed") == Some(&Node::Bool(true))
}

/// An integer type a spec or the link names: its name, which is Rust's
/// spelling, the numbers it holds from `lo` up to and not reaching `end`, its
/// C++ type, the header that declares it and its literals' suffix, its
/// TypeScript type, and whether an enum may be held in it. TypeScript writes
/// one a `number` where a `number` holds it exactly, to 2^53, and a `bigint`
/// past that, as WIT's 64-bit integers are. A `usize` holds what a 32-bit
/// target's does, the narrowest a spec is written for.
pub struct Int {
    pub name: &'static str,
    pub lo: f64,
    pub end: f64,
    pub cpp: &'static str,
    pub header: &'static str,
    pub suffix: &'static str,
    pub ts: &'static str,
    pub repr: bool,
}

impl Int {
    /// Whether `x` is a number of the type: whole, from `lo` up to `end`.
    pub fn holds(&self, x: f64) -> bool {
        x.fract() == 0.0 && x >= self.lo && x < self.end
    }
}

const fn int(
    name: &'static str,
    bits: i32,
    signed: bool,
    cpp: &'static str,
    suffix: &'static str,
    repr: bool,
) -> Int {
    let width = bits - signed as i32;
    let end = (1u128 << width) as f64;
    let ts = if width > 53 { "bigint" } else { "number" };
    Int {
        name,
        lo: if signed { -end } else { 0.0 },
        end,
        cpp,
        header: "cstdint",
        suffix,
        ts,
        repr,
    }
}

/// Every integer type a spec may name, the one table of them.
pub const INTS: [Int; 9] = [
    int("u8", 8, false, "std::uint8_t", "u", true),
    int("u16", 16, false, "std::uint16_t", "u", true),
    int("u32", 32, false, "std::uint32_t", "u", true),
    int("u64", 64, false, "std::uint64_t", "ull", false),
    Int {
        header: "cstddef",
        ..int("usize", 32, false, "std::size_t", "u", false)
    },
    int("i8", 8, true, "std::int8_t", "", true),
    int("i16", 16, true, "std::int16_t", "", true),
    int("i32", 32, true, "std::int32_t", "", true),
    int("i64", 64, true, "std::int64_t", "ll", false),
];

/// The integer type `ty` names, none for any other type.
pub fn int_of(ty: &str) -> Option<&'static Int> {
    INTS.iter().find(|i| i.name == ty)
}

/// Whether `ty` is a float a spec may name, the other type a number has
/// beside `INTS`'.
pub fn is_float(ty: &str) -> bool {
    matches!(ty, "f32" | "f64")
}

/// The TypeScript type of a number of type `ty`: its integer's (`INTS`), or a
/// float's, a `number`.
pub fn ts_number(ty: &str) -> &'static str {
    int_of(ty).map_or("number", |i| i.ts)
}

/// Refuses a number under `n` that has no type or a type no number has, or
/// whose type, its token's
/// `$type`, its group's (`ty`) or, for one of the spec's own packed tables,
/// `PACKED_TYPE`, is an integer it is not: not whole, or past its width. A
/// token's value is checked whole: a number, a list, a table or a record, each
/// number of it typed by the token. A group's `enum` is its own mark: its
/// members are numbered by their `repr` (data.rs `numbers`), not typed by it,
/// and its tables take the type around it.
fn check_widths(n: &Node, at: &str, ty: Option<&str>) -> Result<(), String> {
    fn value(v: &Node, at: &str, ty: Option<&str>) -> Result<(), String> {
        let ty = v.str_at("$type").or(ty);
        match v.val() {
            Node::Num(x) => match ty.map(|t| (t, int_of(t))) {
                None => Err(format!("{at}: {x} has no `$type`, its own or its group's")),
                Some((t, Some(i))) if !i.holds(*x) => Err(format!("{at}: {x} is no {t}")),
                Some((t, None)) if !is_float(t) => {
                    Err(format!("{at}: {x} is a number, and `{t}` no number's type"))
                }
                _ => Ok(()),
            },
            Node::Arr(a) => a
                .iter()
                .enumerate()
                .try_for_each(|(i, e)| value(e, &format!("{at}[{i}]"), ty)),
            o @ Node::Obj(_) => o
                .named()
                .try_for_each(|(k, e)| value(e, &format!("{at}.{k}"), ty)),
            _ => Ok(()),
        }
    }
    if n.is_wrapper() {
        return value(n, at, ty);
    }
    let own = n.str_at("$type");
    let top = at.is_empty();
    n.named().try_for_each(|(k, v)| {
        let at = if top {
            k.to_string()
        } else {
            format!("{at}.{k}")
        };
        match (v, own) {
            (Node::Obj(_), _) if top && is_packed(v) => check_widths(v, &at, Some(PACKED_TYPE)),
            (Node::Obj(_), _) if !v.is_wrapper() => {
                check_widths(v, &at, own.filter(|t| *t != "enum").or(ty))
            }
            (_, Some("enum")) => Ok(()),
            _ => value(v, &at, own.or(ty)),
        }
    })
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
        let leads = c.is_ascii_digit() && i > 0 && chars[i - 1].is_ascii_lowercase() && {
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
    c.next().map_or(String::new(), |f| {
        f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase()
    })
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
        _ if ["s", "x", "z", "ch", "sh"]
            .iter()
            .any(|e| lower.ends_with(e)) =>
        {
            (name, "es")
        }
        _ => (name, "s"),
    };
    let end = if upper {
        end.to_ascii_uppercase()
    } else {
        end.to_string()
    };
    format!("{stem}{end}")
}

/// Rust's keywords, strict and reserved, which an identifier takes as `r#`.
const RS_KEYWORDS: [&str; 50] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in",
    "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
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

/// C++'s keywords and alternative tokens, which an identifier takes `_` after.
const CPP_KEYWORDS: &str = "alignas alignof and and_eq asm auto bitand bitor bool break case catch \
    char char8_t char16_t char32_t class compl concept const consteval constexpr constinit \
    const_cast continue co_await co_return co_yield decltype default delete do double \
    dynamic_cast else enum explicit export extern false float for friend goto if inline int \
    long mutable namespace new noexcept not not_eq nullptr operator or or_eq private \
    protected public register reinterpret_cast requires return short signed sizeof static \
    static_assert static_cast struct switch template this thread_local throw true try typedef \
    typeid typename union unsigned using virtual void volatile wchar_t while xor xor_eq";

/// A name as a C++ identifier: each `-` becomes `_`, a leading digit gets an
/// `_` before it, and a keyword gets one after it.
pub fn cpp_ident(k: &str) -> String {
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
    if CPP_KEYWORDS.split_whitespace().any(|w| w == id) {
        format!("{id}_")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

/// An enum's member as C++ names it: in snake case, as the C++ here names
/// its constants' members. An outside name too, since C++'s own headers
/// define POSIX's errno names as macros.
pub fn cpp_member(k: &str) -> String {
    cpp_ident(&snake(k))
}

/// The headers a C++ file includes, each added where its body names what the
/// header declares, as Rust's `Scope` `use`s.
pub type Includes = BTreeSet<&'static str>;

/// An enum's C++, as every enum has it: an enum class held in `repr`, and the
/// member a number is (`of_ident`), as Rust's `of`.
pub fn cpp_enum(
    name: &str,
    members: &[Member],
    repr: &str,
    out: &mut Vec<String>,
    inc: &mut Includes,
) {
    let e = pascal(name);
    inc.extend(["cstdint", "optional"]);
    out.push(format!("enum class {e} : {repr} {{"));
    for m in members {
        out.extend(line_comments(&m.doc, "    "));
        out.push(format!("    {} = {},", m.ident, m.number));
    }
    out.extend([
        "};".to_string(),
        String::new(),
        "// The member `val` is, or none for a number no member has.".into(),
        format!(
            "constexpr std::optional<{e}> {}({repr} val) noexcept {{",
            of_ident(&e)
        ),
        "    switch (val) {".into(),
    ]);
    out.extend(
        members
            .iter()
            .map(|m| format!("    case {}: return {e}::{};", m.number, m.ident)),
    );
    out.extend([
        "    }".to_string(),
        "    return std::nullopt;".into(),
        "}".into(),
    ]);
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
#[derive(Clone)]
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
    out.extend(
        members
            .iter()
            .map(|m| format!("{indent}        {e}::{},", m.ident)),
    );
    out.extend([
        format!("{indent}    ];"),
        String::new(),
        format!("{indent}    /// The member `val` is, or none for a number no member has."),
        format!("{indent}    pub const fn of(val: {repr}) -> Option<{e}> {{"),
        format!("{indent}        match val {{"),
    ]);
    out.extend(members.iter().map(|m| {
        format!(
            "{indent}            {} => Some({e}::{}),",
            m.number, m.ident
        )
    }));
    out.extend([
        format!("{indent}            _ => None,"),
        format!("{indent}        }}"),
        format!("{indent}    }}"),
        String::new(),
        format!("{indent}    /// The member's name, as its source spells it."),
        format!("{indent}    pub const fn name(self) -> &'static str {{"),
        format!("{indent}        match self {{"),
    ]);
    out.extend(members.iter().map(|m| {
        format!(
            "{indent}            {e}::{} => {},",
            m.ident,
            json_str(&m.name)
        )
    }));
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
/// name, the member a number is (`of_ident`), as Rust's `of`, and a member's
/// name (`name_Errno`), as Rust's `name`. No object holds them, so a bundle's
/// tree shaking keeps no name it does not read.
pub fn ts_enum(e: &str, members: &[Member]) -> Vec<String> {
    let union: Vec<String> = members
        .iter()
        .map(|m| format!("typeof {}", ts_const(e, m)))
        .collect();
    let mut out = vec![
        String::new(),
        format!("export type {e} = {};", union.join(" | ")),
    ];
    for m in members {
        out.extend(ts_doc(&m.doc, ""));
        out.push(format!("export const {} = {};", ts_const(e, m), m.number));
    }
    let tests: Vec<String> = members
        .iter()
        .map(|m| format!("val === {}", ts_const(e, m)))
        .collect();
    out.extend([
        String::new(),
        "/** The member `val` is, or null for a number no member has. */".into(),
        format!(
            "export function {}(val: number): {e} | null {{",
            of_ident(e)
        ),
        format!("  return {} ? val : null;", tests.join(" || ")),
        "}".into(),
        String::new(),
        "/** The member's name, as its source spells it. */".into(),
        format!("export function name_{e}(val: {e}): string {{"),
        "  switch (val) {".into(),
    ]);
    for m in members {
        out.push(format!("    case {}:", ts_const(e, m)));
        out.push(format!("      return {};", json_str(&m.name)));
    }
    out.extend(["  }".to_string(), "}".into()]);
    out
}

/// The name TypeScript and C++ give a Rust enum's `of`, of the enum `e`: the
/// member a number is (`of_Tool`).
pub fn of_ident(e: &str) -> String {
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

/// How a path maps to its package: the package's name and its directory,
/// each from the root the path is from (`package`, or a table of the
/// packages a program depends on).
pub type Packages<'a> = &'a dyn Fn(&str) -> Result<(String, PathBuf), String>;

/// The package `path` from `root` lies in: its name, from the nearest
/// Cargo.toml or package.json above it, and its directory from `root`.
pub fn package(root: &Path, path: &str) -> Result<(String, PathBuf), String> {
    for dir in Path::new(path).ancestors().skip(1) {
        let cargo = root.join(dir).join("Cargo.toml");
        let npm = root.join(dir).join("package.json");
        let name = if cargo.is_file() {
            let text =
                std::fs::read_to_string(&cargo).map_err(|e| format!("{}: {e}", cargo.display()))?;
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
            name.ok_or(format!("{}: no name", npm.display()))?
                .to_string()
        } else {
            continue;
        };
        return Ok((name, dir.to_path_buf()));
    }
    Err(format!("{path}: under no package"))
}

/// The path from the directory `from` to `to`.
pub fn relative(from: &Path, to: &Path) -> String {
    let (f, t): (Vec<_>, Vec<_>) = (from.components().collect(), to.components().collect());
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let up = std::iter::repeat_n("..".to_string(), f.len() - common);
    let down = t[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    let path: Vec<String> = up.chain(down).collect();
    if path.is_empty() {
        ".".into()
    } else {
        path.join("/")
    }
}

/// How a Rust module in the crate at `here` names the crate of `path`: by
/// its name, or `crate` for its own, `root` being the root `path` is from
/// where `here` lies in that tree, and `pkg` mapping it to its package.
pub fn rs_crate(
    here: &Path,
    root: Option<&Path>,
    pkg: Packages,
    path: &str,
) -> Result<String, String> {
    let (name, dir) = pkg(path)?;
    Ok(if root.is_some_and(|r| r.join(dir) == here) {
        "crate".into()
    } else {
        snake(&name)
    })
}

/// How a Rust module in the crate at `here` names the module `path`
/// (`rs_crate`): its crate, then its path under `src/`, a generated file's
/// `.spec` dropped as its mount drops it.
pub fn rs_module(
    here: &Path,
    root: Option<&Path>,
    pkg: Packages,
    path: &str,
) -> Result<String, String> {
    let (_, dir) = pkg(path)?;
    let under = Path::new(path)
        .strip_prefix(dir.join("src"))
        .map_err(|_| format!("{path}: not under src/"))?;
    let under = under.to_string_lossy();
    let mods = under
        .trim_end_matches(".rs")
        .trim_end_matches(".spec")
        .split('/')
        .map(String::from);
    Ok(std::iter::once(rs_crate(here, root, pkg, path)?)
        .chain(mods)
        .collect::<Vec<_>>()
        .join("::"))
}

/// How a TypeScript module in the directory `here` names the path `path`
/// (`rs_crate`): relative to it in its own package, from `.` or `..` as an
/// import writes it, or by the package's name (`package_path`).
pub fn ts_module(
    here: &Path,
    root: Option<&Path>,
    pkg: Packages,
    path: &str,
) -> Result<String, String> {
    let (_, dir) = pkg(path)?;
    if let Some(r) = root.filter(|r| here.starts_with(r.join(&dir))) {
        let path = relative(here, &r.join(path));
        return Ok(match path.split('/').next() {
            Some("." | "..") => path,
            _ => format!("./{path}"),
        });
    }
    package_path(pkg, path)
}

/// `package_path` names a generated file `path`, which `pkg` maps to its
/// package, as its package takes it: the package's name, then its path under
/// the package's `src/`, or under the package for a file outside `src/`.
pub fn package_path(pkg: Packages, path: &str) -> Result<String, String> {
    let (name, dir) = pkg(path)?;
    let file = Path::new(path);
    let under = file
        .strip_prefix(dir.join("src"))
        .or_else(|_| file.strip_prefix(&dir))
        .unwrap_or(file);
    Ok(format!("{name}/{}", under.to_string_lossy()))
}
