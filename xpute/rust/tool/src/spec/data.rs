// xpute-tool/spec/data.rs

//! Data specs: JSON of named values written where each spec says, as
//! TypeScript `as const`, as Rust constants, as C++ constants and as make
//! variables. A number is f64 unless a `$type` says otherwise; a u64 is a hex
//! string, an rgb a `#RRGGBB`. `{ "$val", "$doc" }` carries a reason. A table
//! may be laid out in words (`laid_out`), its widths read from the specs a
//! spec uses. A list of names may give each member as `{ "name", <flag>:
//! bool, … }`, every member the same flags: the TypeScript keeps its names,
//! and the Rust's enum gives each flag as a const of the members it holds
//! (`flagged`). A program's own `$type` may name an entry of a list it keeps
//! (`Slots`).

use std::cell::Cell;
use std::path::Path;

use super::{
    align_star, casing_note, enum_name, file, generated, js_num, json_str, line_comments, read,
    rgb_literal, rs_doc, rs_ident, snake, ts_doc, Node, Output,
};
use crate::root;

/// Which of a spec's lists of names, and tables of numbers, the Rust also
/// writes as an enum.
pub enum Enums {
    None,
    /// Every list of names, and every table of distinct numbers below 256 or
    /// of `$number`s.
    All,
    /// The lists of names it names.
    Named(&'static [&'static str]),
}

impl Enums {
    fn has(&self, name: &str) -> bool {
        match self {
            Enums::None => false,
            Enums::All => true,
            Enums::Named(list) => list.contains(&name),
        }
    }
}

pub struct DataSpec {
    pub json: &'static str,
    /// The specs whose names its expressions may read, each name owned by
    /// one file in scope.
    pub uses: &'static [&'static str],
    pub ts: Option<&'static str>,
    /// The names the TypeScript writes; all where none are given.
    pub ts_names: Option<&'static [&'static str]>,
    pub rs: Option<&'static str>,
    /// The module the Rust is included as, in the directory of its file.
    pub module: &'static str,
    /// Where the same Rust is written again, for a crate that cannot depend
    /// on the one `rs` is in (a build script), each under its crate's name.
    pub rs_also: &'static [(&'static str, &'static str)],
    pub enums: Enums,
    /// The C++ header, and its namespace.
    pub cpp: Option<(&'static str, &'static str)>,
    /// For a spec a Makefile reads: where its numbers are written as make
    /// variables, and the prefix each takes.
    pub mk: Option<(&'static str, &'static str)>,
}

impl DataSpec {
    /// A spec that writes nothing: a table's row names what differs from it.
    pub const DEFAULT: DataSpec = DataSpec {
        json: "",
        uses: &[],
        ts: None,
        ts_names: None,
        rs: None,
        module: "spec",
        rs_also: &[],
        enums: Enums::None,
        cpp: None,
        mk: None,
    };
}

/// A spec's run of slots: the spec, its first slot, and how many it holds.
pub type Run<'a> = (&'a str, u16, u16);

/// A program's own `$type`, whose leaves each name an entry of a list it
/// keeps: in TypeScript the entry's number, its place in `names` from 1; in
/// Rust a slot, `rs` around a number, a spec's leaves numbered in document
/// order from the first of its run.
pub struct Slots<'a> {
    pub ty: &'a str,
    pub rs: &'a str,
    pub names: &'a [String],
    /// A spec not listed holds none.
    pub runs: &'a [Run<'a>],
}

impl Slots<'_> {
    fn number(&self, name: &str) -> Result<usize, String> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| i + 1)
            .ok_or(format!("{name} is no `{}` the program lists", self.ty))
    }

    fn run(&self, json: &str) -> (u16, u16) {
        self.runs
            .iter()
            .find(|(j, ..)| *j == json)
            .map_or((0, 0), |(_, first, cnt)| (*first, *cnt))
    }
}

/// Where a program's data specs are written, and what its Rust carries.
pub struct Layout<'a> {
    /// A TypeScript file's directory, and the name it is imported by.
    pub ts: &'a [(&'a str, &'a str)],
    /// A Rust file's `src/`, and its crate's name.
    pub rs: &'a [(&'a str, &'a str)],
    /// Whether the Rust carries names: a list of them as itself, and a
    /// table's keys as its `KEYS`. Where it does not, a list is its count.
    pub rs_names: bool,
    pub slots: &'a [Slots<'a>],
}

const DATA_SPECS: [DataSpec; 8] = [
    DataSpec {
        json: "spec/conformance/core.json",
        ts: Some("ts/runtime/src/conformance.spec.ts"),
        rs: Some("rust/conformance/src/core.spec.rs"),
        cpp: Some(("cpp/conformance/core.spec.hpp", "xpute::conformance")),
        mk: Some(("cpp/conformance/core.spec.mk", "CONFORMANCE_")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/ipc/head.json",
        ts: Some("ts/runtime/src/ipc/head.spec.ts"),
        rs: Some("rust/core/src/ipc/head.spec.rs"),
        cpp: Some(("cpp/xpute/core/ipc/head.spec.hpp", "xpute::head")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/ipc/frame.json",
        ts: Some("ts/runtime/src/ipc/frame.spec.ts"),
        rs: Some("rust/core/src/ipc/frame.spec.rs"),
        cpp: Some(("cpp/xpute/core/ipc/frame.spec.hpp", "xpute::frame")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/ipc/ring.json",
        ts: Some("ts/runtime/src/ipc/ring.spec.ts"),
        rs: Some("rust/core/src/ipc/ring.spec.rs"),
        cpp: Some(("cpp/xpute/core/ipc/ring.spec.hpp", "xpute::ring")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/ipc/fs.json",
        ts: Some("ts/runtime/src/ipc/fs.spec.ts"),
        rs: Some("rust/core/src/ipc/fs.spec.rs"),
        cpp: Some(("cpp/xpute/core/ipc/fs.spec.hpp", "xpute::fs")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/ipc/stream.json",
        ts: Some("ts/runtime/src/ipc/stream.spec.ts"),
        rs: Some("rust/core/src/ipc/stream.spec.rs"),
        cpp: Some(("cpp/xpute/core/ipc/stream.spec.hpp", "xpute::stream")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/gpu/api.json",
        ts: Some("ts/gpu/src/api.spec.ts"),
        rs: Some("rust/gpu/src/api.spec.rs"),
        enums: Enums::Named(&["BINDING_KIND", "RESOURCE_KIND"]),
        cpp: Some(("cpp/xpute/gpu/api.spec.hpp", "xpute::gpu")),
        ..DataSpec::DEFAULT
    },
    DataSpec {
        json: "spec/gpu/enum.json",
        ts: Some("ts/gpu/src/enum.spec.ts"),
        rs: Some("rust/gpu/src/enum.spec.rs"),
        enums: Enums::Named(&[
            "TEXTURE_FORMAT",
            "VERTEX_FORMAT",
            "PRIMITIVE_TOPOLOGY",
            "INDEX_FORMAT",
            "FRONT_FACE",
            "CULL_MODE",
            "COMPARE_FUNCTION",
            "STENCIL_OPERATION",
            "BLEND_OPERATION",
            "BLEND_FACTOR",
            "VERTEX_STEP_MODE",
            "ADDRESS_MODE",
            "FILTER_MODE",
            "MIPMAP_FILTER_MODE",
            "TEXTURE_DIMENSION",
            "TEXTURE_VIEW_DIMENSION",
            "TEXTURE_ASPECT",
            "BUFFER_BINDING_TYPE",
            "SAMPLER_BINDING_TYPE",
            "TEXTURE_SAMPLE_TYPE",
            "STORAGE_TEXTURE_ACCESS",
            "LOAD_OP",
            "STORE_OP",
            "QUERY_TYPE",
            "CANVAS_ALPHA_MODE",
            "FEATURE_NAME",
            "LIMIT_NAME",
        ]),
        cpp: Some(("cpp/xpute/gpu/enum.spec.hpp", "xpute::gpu")),
        ..DataSpec::DEFAULT
    },
];

const LAYOUT: Layout = Layout {
    ts: &[
        ("ts/runtime/src/", "@xpute/runtime/"),
        ("ts/gpu/src/", "@xpute/gpu/"),
    ],
    rs: &[
        ("rust/core/src/", "xpute-core"),
        ("rust/gpu/src/", "xpute-gpu"),
        ("rust/conformance/src/", "xpute-conformance"),
    ],
    rs_names: true,
    slots: &[],
};

pub fn outputs() -> Result<Vec<Output>, String> {
    outputs_at(&root(), &DATA_SPECS, &LAYOUT)
}

fn type_of<'a>(n: &'a Node, or: &'a str) -> &'a str {
    n.str_at("$type").unwrap_or(or)
}

fn is_ts_ident(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_' || f == '$')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '$')
}

/// What a module cannot export a constant as.
const TS_RESERVED: [&str; 46] = [
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

/// A group each of whose members is a number, by a name a module can export:
/// its members, written as a module of constants beside the group's file.
fn numbers(n: &Node) -> Option<Vec<(&str, &Node)>> {
    let entries: Vec<(&str, &Node)> = n.is_obj().then(|| n.named().collect())?;
    (!entries.is_empty()
        && entries.iter().all(|(k, v)| {
            is_ts_ident(k)
                && !TS_RESERVED.contains(k)
                && matches!(if v.is_wrapper() { v.value() } else { v }, Node::Num(_))
        }))
    .then_some(entries)
}

/// A group's module beside `ts_path`, `<its stem>/<the group>.spec.ts`: the
/// path, and what the file beside it imports it from.
fn beside(ts_path: &str, group: &str) -> (String, String) {
    let stem = ts_path.strip_suffix(".spec.ts").unwrap_or(ts_path);
    let file_stem = stem.rsplit('/').next().unwrap_or(stem);
    let module = format!("{}.spec.ts", snake(group));
    (
        format!("{stem}/{module}"),
        format!("./{file_stem}/{module}"),
    )
}

/// A TypeScript file's path as it is imported, by the layout's name for its
/// directory.
fn logical(layout: &Layout, path: &str) -> String {
    layout
        .ts
        .iter()
        .find_map(|(dir, name)| path.strip_prefix(dir).map(|p| format!("{name}{p}")))
        .unwrap_or(path.to_string())
}

/// A group as a module of its members' constants, read as a namespace of the
/// group's name: a bundle inlines each member and carries none of their names.
fn constants(logical: &str, json: &str, members: &[(&str, String, Option<Vec<String>>)]) -> String {
    let mut out = vec![format!("// {logical}"), "//".into(), generated(json)];
    for (k, lit, doc) in members {
        out.push(String::new());
        if let Some(doc) = doc {
            out.extend(ts_doc(doc, "").into_iter().enumerate().map(|(i, l)| {
                if i == 0 {
                    l
                } else {
                    align_star(l, "")
                }
            }));
        }
        out.push(format!("export const {k} = {lit};"));
    }
    file(&out)
}

/// A flag of a list's members and the names of those it holds (`flagged`).
type Flag<'a> = (&'a str, Vec<&'a str>);

/// A list's names and its members' flags (`flagged`).
type Flagged<'a> = (Vec<&'a str>, Vec<Flag<'a>>);

/// A list of names whose members carry flags, each `{ "name", <flag>: bool,
/// … }`, every one the same flags in one order: its names, and each flag with
/// the names it holds. None where the list's members are not such.
fn flagged(a: &[Node]) -> Result<Option<Flagged<'_>>, String> {
    fn flags_of(e: &Node) -> Vec<&str> {
        e.named().map(|(k, _)| k).filter(|k| *k != "name").collect()
    }
    let Some(first) = a.first().filter(|e| e.str_at("name").is_some()) else {
        return Ok(None);
    };
    let mut held: Vec<Flag<'_>> = flags_of(first)
        .into_iter()
        .map(|f| (f, Vec::new()))
        .collect();
    let mut names = Vec::with_capacity(a.len());
    for e in a {
        let name = e.str_at("name").ok_or(format!(
            "a member with no name after `{}`",
            names.join("`, `")
        ))?;
        let flags = flags_of(e);
        if flags.len() != held.len() || flags.iter().zip(&held).any(|(f, (h, _))| f != h) {
            return Err(format!(
                "`{name}` carries {flags:?}, not the first member's flags"
            ));
        }
        for (f, members) in &mut held {
            match e.get(f) {
                Some(Node::Bool(true)) => members.push(name),
                Some(Node::Bool(false)) => {}
                _ => return Err(format!("`{name}`'s `{f}` is not true or false")),
            }
        }
        names.push(name);
    }
    Ok(Some((names, held)))
}

/// `n` as a TypeScript literal, a leaf of a program's own `$type` by the
/// number `slots` gives it.
pub fn ts_literal(n: &Node, ty: &str, indent: &str, slots: &[Slots]) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_literal(n.value(), type_of(n, ty), indent, slots);
    }
    Ok(match n {
        Node::Num(v) => format!("{} as number", js_num(*v)),
        Node::Bool(b) => b.to_string(),
        Node::Str(s) if ty == "u64" => format!("{s}n"),
        Node::Str(s) if ty == "rgb" => rgb_literal(s)?,
        Node::Str(s) => match slots.iter().find(|k| k.ty == ty) {
            Some(k) => format!("{} as number", k.number(s)?),
            None => json_str(s),
        },
        Node::Arr(a) => match flagged(a)? {
            Some((names, _)) => format!(
                "[{}]",
                names
                    .iter()
                    .map(|n| json_str(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None => format!(
                "[{}]",
                a.iter()
                    .map(|e| ts_literal(e, ty, indent, slots))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
        },
        Node::Null => "null".to_string(),
        Node::Obj(_) => {
            let inner = format!("{indent}  ");
            let t = type_of(n, ty);
            let mut lines = vec!["{".to_string()];
            for (k, v) in n.named() {
                if let Some(doc) = v.is_obj().then(|| v.lines_at("$doc")).flatten() {
                    lines.extend(ts_doc(&doc, &inner).into_iter().enumerate().map(|(i, l)| {
                        if i == 0 {
                            l
                        } else {
                            align_star(l, &inner)
                        }
                    }));
                }
                let key = if is_ts_ident(k) {
                    k.to_string()
                } else {
                    json_str(k)
                };
                lines.push(format!(
                    "{inner}{key}: {},",
                    ts_literal(v, t, &inner, slots)?
                ));
            }
            lines.push(format!("{indent}}}"));
            lines.join("\n")
        }
    })
}

struct Scope<'a> {
    module: String,
    top: Vec<String>,
    enums: &'a Enums,
    rs_names: bool,
    slots: &'a [Slots<'a>],
    /// Each of `slots`' next slot.
    next: Vec<Cell<u16>>,
}

fn is_primitive(ty: &str) -> bool {
    matches!(
        ty,
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "rgb" | "str"
    )
}

fn enum_path(ty: &str, scope: &Scope) -> Option<String> {
    if is_primitive(ty) {
        return None;
    }
    let first = ty.split("::").next().unwrap_or(ty);
    let declared = scope.top.iter().any(|t| t == first);
    if ty.contains("::") && !declared {
        return Some(ty.to_string());
    }
    declared.then(|| format!("{}::{ty}", scope.module))
}

fn rs_scalar(n: &Node, ty: &str, scope: &Scope) -> Result<(String, String), String> {
    Ok(match n {
        Node::Bool(b) => ("bool".to_string(), b.to_string()),
        Node::Str(s) => match ty {
            "u64" => ("u64".to_string(), s.clone()),
            "rgb" => ("u32".to_string(), rgb_literal(s)?),
            _ => match scope.slots.iter().position(|k| k.ty == ty) {
                Some(i) => {
                    let (rs, next) = (scope.slots[i].rs, &scope.next[i]);
                    let slot = next.replace(next.get() + 1);
                    (rs.to_string(), format!("{rs}({slot})"))
                }
                None => match enum_path(ty, scope) {
                    None => ("&str".to_string(), json_str(s)),
                    Some(path) => (path.clone(), format!("{path}::{}", rs_ident(s))),
                },
            },
        },
        Node::Num(v) if ty == "f64" || ty == "f32" => (
            ty.to_string(),
            if v.fract() == 0.0 {
                format!("{}.0", js_num(*v))
            } else {
                js_num(*v)
            },
        ),
        Node::Num(v) => (ty.to_string(), js_num(*v)),
        _ => return Err(format!("a {ty} leaf that is not a scalar: {n:?}")),
    })
}

fn rs_enum(name: &str, variants: &[(String, String)], indent: &str, out: &mut Vec<String>) {
    let e = enum_name(name);
    out.extend([
        format!("{indent}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"),
        format!("{indent}#[repr(u8)]"),
        format!("{indent}pub enum {e} {{"),
    ]);
    out.extend(
        variants
            .iter()
            .map(|(v, n)| format!("{indent}    {} = {n},", rs_ident(v))),
    );
    out.extend([format!("{indent}}}"), String::new()]);
    out.extend([
        format!("{indent}impl {e} {{"),
        format!("{indent}    /// Every member, in the JSON's own order."),
        format!("{indent}    pub const ALL: [{e}; {}] = [", variants.len()),
    ]);
    out.extend(
        variants
            .iter()
            .map(|(v, _)| format!("{indent}        {e}::{},", rs_ident(v))),
    );
    out.extend([
        format!("{indent}    ];"),
        String::new(),
        format!("{indent}    /// The member a number names, or none for a number the set does not"),
        format!("{indent}    /// give out."),
        format!("{indent}    pub fn of(val: u8) -> Option<{e}> {{"),
        format!("{indent}        {e}::ALL.into_iter().find(|member| *member as u8 == val)"),
        format!("{indent}    }}"),
        format!("{indent}}}"),
    ]);
}

fn rs_items(
    name: &str,
    n: &Node,
    ty: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    let mut lines: Vec<String> = Vec::new();
    if n.is_wrapper() {
        rs_leaf(name, n.value(), type_of(n, ty), indent, &mut lines, scope)?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, indent, &mut lines, scope)?;
    } else {
        let t = type_of(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        let all = matches!(scope.enums, Enums::All);
        let numbers: Vec<Option<f64>> = keys.iter().map(|(_, v)| v.num()).collect();
        let numbered = all
            && !keys.is_empty()
            && matches!(t, "u8" | "u16" | "u32")
            && numbers.iter().all(|v| v.is_some_and(|v| v < 256.0));
        let distinct = numbers
            .iter()
            .enumerate()
            .all(|(i, a)| numbers[..i].iter().all(|b| b != a));
        if numbered && distinct {
            rs_enum(
                name,
                &keys
                    .iter()
                    .map(|(k, v)| (k.to_string(), js_num(v.num().unwrap_or_default())))
                    .collect::<Vec<_>>(),
                indent,
                &mut lines,
            );
            lines.push(String::new());
        } else if all
            && !keys.is_empty()
            && keys
                .iter()
                .all(|(_, v)| v.is_obj() && v.get("$number").and_then(Node::num).is_some())
        {
            // An object whose entries carry no `$number` is a structure, not a set.
            let variants: Vec<(String, String)> = keys
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        js_num(v.get("$number").and_then(Node::num).unwrap_or_default()),
                    )
                })
                .collect();
            rs_enum(name, &variants, indent, &mut lines);
            lines.push(String::new());
        }
        let mut body: Vec<String> = Vec::new();
        let inner = format!("{indent}    ");
        for (k, v) in &keys {
            rs_items(k, v, t, &inner, &mut body, scope)?;
        }
        if scope.rs_names {
            body.push(format!(
                "{indent}    pub const KEYS: &[&str] = &[{}];",
                keys.iter()
                    .map(|(k, _)| json_str(k))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !body.is_empty() {
            lines.push(format!("{indent}pub mod {} {{", rs_ident(name)));
            lines.extend(body);
            lines.push(format!("{indent}}}"));
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    if let Some(doc) = doc {
        out.extend(rs_doc(&doc, indent, "///"));
    }
    out.extend(lines);
    Ok(())
}

/// A member's name, as an enum takes it once made an identifier: a letter, a
/// digit or `_`, then those or `-` (WebGPU's "2d-array").
fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphanumeric() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// A member's name as a C++ enumerator: `-` an `_`, a leading digit behind
/// an `_`, a keyword followed by one.
fn cpp_ident(k: &str) -> String {
    const KEYWORDS: [&str; 12] = [
        "float", "double", "int", "char", "bool", "default", "delete", "new", "and", "or", "not",
        "xor",
    ];
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
    if KEYWORDS.contains(&id.as_str()) {
        format!("{id}_")
    } else if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{id}")
    } else {
        id
    }
}

/// A list of names in Rust: the names, where the Rust carries them, or how
/// many there are, as a spec's widths read a list's name.
fn rs_names(name: &str, names: &[&str], indent: &str, scope: &Scope) -> String {
    if scope.rs_names {
        let quoted: Vec<String> = names.iter().map(|n| json_str(n)).collect();
        format!(
            "{indent}pub const {}: &[&str] = &[{}];",
            rs_ident(name),
            quoted.join(", ")
        )
    } else {
        format!(
            "{indent}pub const {}: usize = {};",
            rs_ident(name),
            names.len()
        )
    }
}

fn rs_leaf(
    name: &str,
    v: &Node,
    ty: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    if let Node::Arr(a) = v {
        if let Some((names, flags)) = flagged(a)? {
            if !scope.enums.has(name) || !names.iter().all(|n| is_name(n)) {
                return Err(format!("{name}: its members carry flags, so it is an enum"));
            }
            out.push(rs_names(name, &names, indent, scope));
            let variants: Vec<(String, String)> = names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.to_string(), i.to_string()))
                .collect();
            rs_enum(name, &variants, indent, out);
            let e = enum_name(name);
            out.extend([String::new(), format!("{indent}impl {e} {{")]);
            for (f, members) in flags {
                out.extend([
                    format!(
                        "{indent}    /// The members whose `{f}` is true, in the JSON's order."
                    ),
                    format!(
                        "{indent}    pub const {}: [{e}; {}] = [{}];",
                        rs_ident(&f.to_uppercase()),
                        members.len(),
                        members
                            .iter()
                            .map(|m| format!("{e}::{}", rs_ident(m)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ]);
            }
            out.push(format!("{indent}}}"));
            return Ok(());
        }
        let strings = matches!(a.first(), Some(Node::Str(_)));
        let slot = scope.slots.iter().any(|k| k.ty == ty);
        if strings && ty != "u64" && ty != "rgb" && !slot && enum_path(ty, scope).is_none() {
            let names: Vec<&str> = a.iter().map(|e| e.str().unwrap_or_default()).collect();
            out.push(rs_names(name, &names, indent, scope));
            if scope.enums.has(name) && a.iter().all(|e| e.str().is_some_and(is_name)) {
                let variants: Vec<(String, String)> = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.to_string(), i.to_string()))
                    .collect();
                rs_enum(name, &variants, indent, out);
            }
            return Ok(());
        }
        let items = a
            .iter()
            .map(|e| rs_scalar(e, ty, scope))
            .collect::<Result<Vec<_>, _>>()?;
        let item_ty = items.first().map(|(t, _)| t.clone()).unwrap_or_else(|| {
            if ty == "str" {
                "&str".to_string()
            } else {
                ty.to_string()
            }
        });
        out.push(format!(
            "{indent}pub const {}: [{item_ty}; {}] = [{}];",
            rs_ident(name),
            a.len(),
            items
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return Ok(());
    }
    let (t, lit) = rs_scalar(v, ty, scope)?;
    out.push(format!(
        "{indent}pub const {}: {t} = {lit};",
        rs_ident(name)
    ));
    Ok(())
}

/// A table with `"$packed": true` is laid end to end: each field gives its
/// width in words, and the generator writes where it begins and the table's
/// `WORDS` where the last ends; a section inside is a packed table given its
/// `AT`. A width is a number or a product of terms (`"2 * COUNT"`), a term a
/// number, a list or a packed table above it in the file (its length, its
/// `WORDS`) or a number in one (`TABLE.FIELD`), else one in a file it uses
/// (`used`). So a field grown moves everything after it.
fn laid_out(node: Node, used: &[(String, Node)]) -> Result<Node, String> {
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
            // A list in a table is as wide as it is long: `TABLE.LIST`.
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
    // A product of terms, less any others: `TABLE.FIELD - 1`.
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

fn read_at(root: &Path, json: &str) -> Result<Node, String> {
    read(&root.join(json).to_string_lossy())
}

/// The names `json` may read past its own, from the specs it uses: each
/// laid out over its own uses, a name owned by one file, no use circular.
fn read_used(
    root: &Path,
    json: &str,
    specs: &[DataSpec],
    path: &mut Vec<&'static str>,
) -> Result<Vec<(String, Node)>, String> {
    let d = specs
        .iter()
        .find(|d| d.json == json)
        .ok_or(format!("{json}: uses a spec that is not listed"))?;
    if path.contains(&d.json) {
        return Err(format!(
            "{}: uses itself through {}",
            d.json,
            path.join(" → ")
        ));
    }
    path.push(d.json);
    let mut names: Vec<(String, Node)> = Vec::new();
    for &u in d.uses {
        let below = read_used(root, u, specs, path)?;
        let Node::Obj(entries) = laid_out(read_at(root, u)?, &below)? else {
            return Err(format!("{u}: not a table"));
        };
        for (k, v) in entries.into_iter().filter(|(k, _)| !k.starts_with('$')) {
            if names.iter().any(|(n, _)| *n == k) {
                return Err(format!(
                    "{}: `{k}` is named in two of the specs it uses",
                    d.json
                ));
            }
            names.push((k, v));
        }
    }
    path.pop();
    Ok(names)
}

/// Every file `specs` write, each path from `root`.
pub fn outputs_at(root: &Path, specs: &[DataSpec], layout: &Layout) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in specs {
        let uses = read_used(root, d.json, specs, &mut Vec::new())?;
        let spec = laid_out(read_at(root, d.json)?, &uses)?;
        if let Some(k) = spec
            .named()
            .map(|(k, _)| k)
            .find(|k| uses.iter().any(|(u, _)| u == k))
        {
            return Err(format!(
                "{}: `{k}` is named here and in a spec it uses",
                d.json
            ));
        }
        if let Some(ts_path) = d.ts {
            ts_outputs(d, &spec, ts_path, layout, &mut outputs)?;
        }
        if let Some(rs_path) = d.rs {
            rs_outputs(d, &spec, rs_path, layout, &mut outputs)?;
        }
        if let Some((cpp_path, namespace)) = d.cpp {
            outputs.push(Output::new(
                cpp_path,
                cpp_file(d, &spec, cpp_path, namespace)?,
            ));
        }
        if let Some((mk_path, prefix)) = d.mk {
            // Each whole number at the top, a make variable a link reads.
            let mut mk = vec![
                format!("# {mk_path}"),
                "#".into(),
                format!("# GENERATED from {}", d.json),
            ];
            for (name, n) in spec.named() {
                if let Some(v) = n.value().num().filter(|v| v.fract() == 0.0 && *v >= 0.0) {
                    mk.push(format!("{prefix}{name} := {v}"));
                }
            }
            outputs.push(Output::new(mk_path, file(&mk)));
        }
    }
    Ok(outputs)
}

/// The spec's head: the file as it is named, the JSON it is generated from,
/// and the spec's `$doc`.
fn head(shown: &str, json: &str, spec: &Node) -> Vec<String> {
    let mut out = vec![format!("// {shown}"), "//".into(), generated(json)];
    if let Some(doc) = spec.lines_at("$doc") {
        out.push("//".into());
        out.extend(line_comments(&doc, ""));
    }
    out
}

fn ts_outputs(
    d: &DataSpec,
    spec: &Node,
    ts_path: &str,
    layout: &Layout,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let names: Vec<&str> = spec.named().map(|(k, _)| k).collect();
    if let Some(listed) = d.ts_names {
        let unknown: Vec<&str> = listed
            .iter()
            .copied()
            .filter(|n| !names.contains(n))
            .collect();
        if !unknown.is_empty() {
            return Err(format!(
                "{}: tsNames lists {}, which the table does not have",
                d.json,
                unknown.join(", ")
            ));
        }
    }
    let mut ts = head(&logical(layout, ts_path), d.json, spec);
    let mut body: Vec<String> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    for name in names
        .iter()
        .copied()
        .filter(|n| d.ts_names.is_none_or(|l| l.contains(n)))
    {
        let n = spec.get(name).ok_or("a name it listed")?;
        body.push(String::new());
        if let Some(doc) = n.is_obj().then(|| n.lines_at("$doc")).flatten() {
            body.extend(ts_doc(&doc, "").into_iter().enumerate().map(|(i, l)| {
                if i == 0 {
                    l
                } else {
                    align_star(l, "")
                }
            }));
        }
        if n.is_wrapper() && !matches!(n.value(), Node::Arr(_) | Node::Obj(_) | Node::Null) {
            let t = type_of(n, "f64");
            let annot = (matches!(n.value(), Node::Num(_)) || t == "u64").then_some(t);
            let annot = annot.filter(|a| *a != "u64");
            if let Some(a) = annot {
                if !words.iter().any(|w| w == a) {
                    words.push(a.to_string());
                }
            }
            body.push(format!(
                "export const {name}{} = {};",
                annot.map(|a| format!(": {a}")).unwrap_or_default(),
                ts_literal(n, t, "", layout.slots)?
            ));
        } else if let Some(entries) = numbers(n) {
            let t = type_of(n, "f64");
            let members = entries
                .iter()
                .map(|(k, v)| {
                    let doc = v.is_obj().then(|| v.lines_at("$doc")).flatten();
                    ts_literal(v, t, "", layout.slots).map(|lit| (*k, lit, doc))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (path, from) = beside(ts_path, name);
            outputs.push(Output::new(
                &path,
                constants(&logical(layout, &path), d.json, &members),
            ));
            modules.push(format!("import * as {name} from \"{from}\";"));
            body.push(format!("export {{ {name} }};"));
        } else {
            body.push(format!(
                "export const {name} = {} as const;",
                ts_literal(n, "f64", "", layout.slots)?
            ));
            let entries: Vec<(&str, &Node)> = n.named().collect();
            if matches!(d.enums, Enums::All)
                && !entries.is_empty()
                && entries
                    .iter()
                    .all(|(_, v)| v.is_obj() && v.get("$number").and_then(Node::num).is_some())
            {
                let e = enum_name(name);
                let number =
                    |v: &Node| js_num(v.get("$number").and_then(Node::num).unwrap_or_default());
                if entries
                    .iter()
                    .all(|(k, _)| is_ts_ident(k) && !TS_RESERVED.contains(k))
                {
                    let members: Vec<(&str, String, Option<Vec<String>>)> =
                        entries.iter().map(|(k, v)| (*k, number(v), None)).collect();
                    let (path, from) = beside(ts_path, &e);
                    outputs.push(Output::new(
                        &path,
                        constants(&logical(layout, &path), d.json, &members),
                    ));
                    modules.push(format!("import * as {e} from \"{from}\";"));
                    body.extend([String::new(), format!("export {{ {e} }};")]);
                } else {
                    body.extend([String::new(), format!("export const {e} = {{")]);
                    body.extend(
                        entries
                            .iter()
                            .map(|(k, v)| format!("  {k}: {},", number(v))),
                    );
                    body.push("} as const;".into());
                }
            }
        }
    }
    if !words.is_empty() {
        words.sort();
        ts.extend([
            String::new(),
            format!(
                "import type {{ {} }} from \"@xpute/kit/abi/word.ts\";",
                words.join(", ")
            ),
        ]);
    }
    if !modules.is_empty() {
        if words.is_empty() {
            ts.push(String::new());
        }
        ts.extend(modules);
    }
    ts.extend(body);
    outputs.push(Output::new(ts_path, file(&ts)));
    Ok(())
}

fn rs_outputs(
    d: &DataSpec,
    spec: &Node,
    rs_path: &str,
    layout: &Layout,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let (crate_name, under_src) = layout
        .rs
        .iter()
        .find_map(|(dir, name)| rs_path.strip_prefix(dir).map(|rest| (*name, rest)))
        .ok_or(format!("{rs_path}: under no crate the layout names"))?;
    let mut rs = head(&format!("{crate_name}/{under_src}"), d.json, spec);
    rs.push(String::new());
    rs.extend(casing_note());
    rs.extend([
        "// A table is generated whole, whether or not this crate calls an entry.".to_string(),
        format!(
            "#![allow(non_upper_case_globals, non_snake_case, clippy::absolute_paths, {}dead_code, reason = \"generated from the spec\")]",
            if matches!(d.enums, Enums::None) {
                ""
            } else {
                "non_camel_case_types, "
            }
        ),
    ]);
    let dir = under_src.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let mut module = String::from("crate");
    if !dir.is_empty() {
        module.push_str("::");
        module.push_str(&dir.replace('/', "::"));
    }
    module.push_str("::");
    module.push_str(d.module);
    let names: Vec<&str> = spec.named().map(|(k, _)| k).collect();
    let mut top: Vec<String> = names.iter().map(|k| enum_name(k)).collect();
    top.extend(names.iter().map(|k| k.to_string()));
    let runs: Vec<(u16, u16)> = layout.slots.iter().map(|k| k.run(d.json)).collect();
    let scope = Scope {
        module,
        top,
        enums: &d.enums,
        rs_names: layout.rs_names,
        slots: layout.slots,
        next: runs.iter().map(|(first, _)| Cell::new(*first)).collect(),
    };
    for (name, n) in spec.named() {
        let mut items = Vec::new();
        rs_items(name, n, "f64", "", &mut items, &scope)?;
        if !items.is_empty() {
            rs.push(String::new());
            rs.extend(items);
        }
    }
    for ((k, (first, cnt)), next) in layout.slots.iter().zip(runs).zip(&scope.next) {
        if next.get() != first + cnt {
            return Err(format!(
                "{}: its `{}` leaves are not the {cnt} of its run",
                d.json, k.ty
            ));
        }
    }
    for (path, logical) in d.rs_also {
        let mut also = rs.clone();
        also[0] = format!("// {logical}");
        outputs.push(Output::new(path, file(&also)));
    }
    outputs.push(Output::new(rs_path, file(&rs)));
    Ok(())
}

fn cpp_file(d: &DataSpec, spec: &Node, cpp_path: &str, namespace: &str) -> Result<String, String> {
    let mut cpp = head(cpp_path, d.json, spec);
    let lists = spec.named().any(|(_, n)| matches!(n, Node::Arr(_)));
    cpp.extend([String::new(), "#pragma once".into(), String::new()]);
    if lists {
        cpp.push("#include <array>".into());
    }
    cpp.push("#include <cstdint>".into());
    if lists {
        cpp.push("#include <string_view>".into());
    }
    cpp.extend([String::new(), format!("namespace {namespace} {{")]);
    for (name, n) in spec.named() {
        cpp_items(name, n, "f64", d.enums.has(name), &mut cpp)?;
    }
    cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&cpp))
}

/// A list of names is a `string_view` array, and with `numbered` also an
/// enum class of them numbered by place; any other non-number is refused.
fn cpp_items(
    name: &str,
    n: &Node,
    ty: &str,
    numbered: bool,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(line_comments(&doc, ""));
    }
    if n.is_obj() && !n.is_wrapper() {
        let t = type_of(n, ty);
        out.push(format!("namespace {name} {{"));
        for (k, v) in n.named() {
            cpp_items(k, v, t, false, out)?;
        }
        out.push(format!("}} // namespace {name}"));
        return Ok(());
    }
    if let Node::Arr(list) = if n.is_wrapper() { n.value() } else { n } {
        let names = list
            .iter()
            .map(|v| {
                v.str()
                    .ok_or(format!("{name}: a C++ list takes names only"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if numbered {
            out.push(format!("enum class {} : std::uint8_t {{", enum_name(name)));
            out.extend(
                names
                    .iter()
                    .enumerate()
                    .map(|(k, v)| format!("    {} = {k},", cpp_ident(v))),
            );
            out.extend(["};".to_string(), String::new()]);
        }
        let quoted: Vec<String> = names.iter().map(|v| json_str(v)).collect();
        out.push(format!(
            "inline constexpr std::array<std::string_view, {}> {name}{{{}}};",
            names.len(),
            quoted.join(", ")
        ));
        return Ok(());
    }
    let (val, t) = if n.is_wrapper() {
        (n.value(), type_of(n, ty))
    } else {
        (n, ty)
    };
    let number = val
        .num()
        .ok_or(format!("{name}: a C++ header takes numbers only"))?;
    let (ty, suffix) = match t {
        "u32" => ("std::uint32_t", "u"),
        "i32" => ("std::int32_t", ""),
        other => {
            return Err(format!(
                "{name}: a C++ header takes u32 and i32, not {other}"
            ))
        }
    };
    out.push(format!(
        "inline constexpr {ty} {name} = {}{suffix};",
        js_num(number)
    ));
    Ok(())
}
