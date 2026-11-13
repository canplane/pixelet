// xpute-gen/spec/data.rs

//! A data spec is a JSON file of named values, written into the files each spec
//! names as TypeScript `as const` values, Rust constants and C++ constants. A
//! number is an f64 unless a `$type` says otherwise. A value wrapped as `{
//! "$val", "$doc" }` carries its reason in `$doc`. A table may be laid out in
//! words (`laid_out`), with its widths read from the specs it uses. A list of
//! names whose `$type` is `enum` is also a Rust enum and a C++ enum class,
//! numbered by place; a table whose `$type` is `enum` is a Rust enum, numbered
//! by its members, each a number or a table that carries its `$number`. Every
//! enum is held in its `$repr` (`u8` where it gives none), and has `of`, a
//! number to its member, and its members' names, in Rust and in TypeScript
//! alike. A list of tables of the same fields is a list of records: an array of
//! objects in TypeScript, and in Rust an array of a struct (`rs_recs`), whose
//! `name`s are the enum where the list's `$type` is `enum`. A table a tool
//! builds in memory is written as a read one is (`table_outputs`).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use super::{
    align_star, casing_note, enum_name, file, mark, is_ts_ident, js_num, json_str, rs_enum, ts_enum,
    line_comments, package_path, read, rs_doc, rs_ident, ts_doc, Node, Output,
};

pub struct DataSpec {
    pub json: &'static str,
    pub ts: Option<&'static str>,
    pub rs: Option<&'static str>,
    /// The C++ header, and its namespace.
    pub cpp: Option<(&'static str, &'static str)>,
}

impl DataSpec {
    /// `DEFAULT` is a spec that writes nothing. A row of a table of specs
    /// spells out only what differs from it.
    pub const DEFAULT: DataSpec = DataSpec {
        json: "",
        ts: None,
        rs: None,
        cpp: None,
    };
}

fn type_of<'a>(n: &'a Node, or: &'a str) -> &'a str {
    n.str_at("$type").unwrap_or(or)
}

/// The type a table gives its members: its own `$type`, or `or` where it has
/// none or where it is `enum`, a mark of the table alone.
fn members_type<'a>(n: &'a Node, or: &'a str) -> &'a str {
    match type_of(n, or) {
        "enum" => or,
        t => t,
    }
}

/// A record field's shape across a list of records: what its values are
/// (`Node` 0 a string, 1 a bool, 2 a number), whether it is a list of them,
/// and whether a record may leave it null.
#[derive(Clone, Copy, PartialEq)]
struct Field {
    kind: u8,
    list: bool,
    option: bool,
}

fn scalar_kind(v: &Node) -> Option<u8> {
    match v {
        Node::Str(_) => Some(0),
        Node::Bool(_) => Some(1),
        Node::Num(_) => Some(2),
        _ => None,
    }
}

/// A list of records reads its fields from its first member: every member is
/// a table of the same fields in the same order, each a string, a bool or a
/// number, a list of one of them, or null where a record has none, alike in
/// every member. None where the list's members are not tables.
fn fields(a: &[Node]) -> Result<Option<Vec<(&str, Field)>>, String> {
    let Some(first) = a.first().filter(|e| e.is_obj() && !e.is_wrapper()) else {
        return Ok(None);
    };
    let keys: Vec<&str> = first.named().map(|(k, _)| k).collect();
    let mut shapes: Vec<Option<Field>> = vec![None; keys.len()];
    let mut option = vec![false; keys.len()];
    for (i, e) in a.iter().enumerate() {
        let has: Vec<&str> = e.named().map(|(k, _)| k).collect();
        if has != keys {
            return Err(format!(
                "[{i}] has the fields {has:?}, not the first member's {keys:?}"
            ));
        }
        for (f, (k, v)) in e.named().enumerate() {
            let shape = match v {
                Node::Null => {
                    option[f] = true;
                    continue;
                }
                Node::Arr(items) => {
                    let kinds: Vec<Option<u8>> = items.iter().map(scalar_kind).collect();
                    match kinds.first() {
                        None => continue,
                        Some(&Some(kind)) if kinds.iter().all(|k| *k == Some(kind)) => Field {
                            kind,
                            list: true,
                            option: false,
                        },
                        _ => return Err(format!("[{i}].{k}: a list of one kind of value")),
                    }
                }
                v => match scalar_kind(v) {
                    Some(kind) => Field {
                        kind,
                        list: false,
                        option: false,
                    },
                    None => return Err(format!("[{i}].{k} is no string, bool, number or list")),
                },
            };
            if shapes[f].is_some_and(|s| s != shape) {
                return Err(format!("[{i}].{k} is not as the other members' `{k}` are"));
            }
            shapes[f] = Some(shape);
        }
    }
    keys.into_iter()
        .zip(shapes)
        .zip(option)
        .map(|((k, s), option)| {
            let s = s.ok_or(format!("`{k}` holds no value in any member"))?;
            Ok((k, Field { option, ..s }))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(Some)
}

/// Writes `n` as a TypeScript literal.
pub fn ts_literal(n: &Node, ty: &str, indent: &str) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_literal(n.val(), type_of(n, ty), indent);
    }
    Ok(match n {
        Node::Num(v) => format!("{} as number", js_num(*v)),
        Node::Bool(b) => b.to_string(),
        Node::Str(s) => json_str(s),
        Node::Arr(a) => format!(
            "[{}]",
            a.iter()
                .map(|e| ts_literal(e, ty, indent))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        ),
        Node::Null => "null".to_string(),
        Node::Obj(_) => {
            let inner = format!("{indent}  ");
            let t = members_type(n, ty);
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
                    ts_literal(v, t, &inner)?
                ));
            }
            lines.push(format!("{indent}}}"));
            lines.join("\n")
        }
    })
}

struct Scope {
    /// The items the module takes from elsewhere, each `use`d at its head so
    /// the body names it by its name.
    uses: RefCell<BTreeSet<String>>,
}

impl Scope {
    /// The name the body writes for `path`, an item of another module, which
    /// the head `use`s.
    fn name_of(&self, path: &str) -> String {
        self.uses.borrow_mut().insert(path.to_string());
        path.rsplit("::").next().unwrap_or(path).to_string()
    }
}

fn is_primitive(ty: &str) -> bool {
    matches!(
        ty,
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"
            | "str"
    )
}

/// Refuses a `$type` that names no primitive and no enum
/// by its path (`module::Enum`), and `enum` on anything but a list or a table.
fn check_types(spec: &Node) -> Result<(), String> {
    fn walk(n: &Node, at: &str) -> Result<(), String> {
        if n.get("$repr").is_some() {
            if n.str_at("$type") != Some("enum") {
                return Err(format!("{at}: `$repr` holds an enum, beside `$type` enum"));
            }
            repr_of(n).map_err(|e| format!("{at}: {e}"))?;
        }
        if let (Some("enum"), Node::Arr(a)) = (n.str_at("$type"), n.val()) {
            let (r, range, _) = repr_of(n).map_err(|e| format!("{at}: {e}"))?;
            if !range.contains(&(a.len() as f64 - 1.0)) {
                let cnt = a.len();
                return Err(format!("{at}: {cnt} members are more than a {r} numbers"));
            }
        }
        if let Some(ty) = n.str_at("$type") {
            let marks = match n.val() {
                Node::Arr(a) => {
                    !a.is_empty()
                        && a.iter()
                            .all(|e| e.str().is_some() || e.str_at("name").is_some())
                }
                v => v.is_obj(),
            };
            if ty == "enum" && !marks {
                return Err(format!(
                    "{at}: `$type` enum marks a list of names, a list of records that carry a `name`, or a table"
                ));
            }
            let known = is_primitive(ty)
                || ty == "enum"
                || ty.contains("::");
            if !known {
                return Err(format!("{at}: `$type` {ty} is no type the generator knows"));
            }
        }
        match n {
            Node::Obj(entries) => {
                for (k, v) in entries.iter().filter(|(k, _)| !k.starts_with('$') || k == "$val") {
                    walk(v, &format!("{at}.{k}"))?;
                }
            }
            Node::Arr(a) => {
                for (i, v) in a.iter().enumerate() {
                    walk(v, &format!("{at}[{i}]"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    spec.named().try_for_each(|(k, v)| walk(v, k))
}

fn enum_path(ty: &str, scope: &Scope) -> Option<String> {
    (!is_primitive(ty) && ty.contains("::")).then(|| scope.name_of(ty))
}

fn rs_scalar(n: &Node, ty: &str, scope: &Scope) -> Result<(String, String), String> {
    Ok(match n {
        Node::Bool(b) => ("bool".to_string(), b.to_string()),
        Node::Str(s) => match enum_path(ty, scope) {
            None => ("&str".to_string(), json_str(s)),
            Some(path) => (path.clone(), format!("{path}::{}", rs_ident(s))),
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

/// The integers an enum may be held in: each `$repr`, the numbers it holds,
/// and its C++ type.
const REPRS: [(&str, f64, f64, &str); 6] = [
    ("u8", 0.0, u8::MAX as f64, "std::uint8_t"),
    ("u16", 0.0, u16::MAX as f64, "std::uint16_t"),
    ("u32", 0.0, u32::MAX as f64, "std::uint32_t"),
    ("i8", i8::MIN as f64, i8::MAX as f64, "std::int8_t"),
    ("i16", i16::MIN as f64, i16::MAX as f64, "std::int16_t"),
    ("i32", i32::MIN as f64, i32::MAX as f64, "std::int32_t"),
];

/// An enum's representation, its `$repr` as written, or `u8`: its name, the
/// numbers it holds and its C++ type.
type Repr = (&'static str, std::ops::RangeInclusive<f64>, &'static str);

fn repr_of(n: &Node) -> Result<Repr, String> {
    let r = n.str_at("$repr").unwrap_or("u8");
    REPRS
        .iter()
        .find(|(name, ..)| *name == r)
        .map(|&(name, lo, hi, cpp)| (name, lo..=hi, cpp))
        .ok_or(format!("`$repr` {r} is no integer an enum is held in"))
}

/// An enum's members as its writer takes them: each one's identifier, its
/// name as the JSON spells it, and its number.
fn members(variants: &[(String, String)]) -> Vec<(String, String, String)> {
    variants
        .iter()
        .map(|(v, n)| (rs_ident(v), v.clone(), n.clone()))
        .collect()
}

/// An enum table's numbers, by member: each member is a number, or a
/// table that carries its `$number`; the numbers are distinct whole numbers
/// that `range`, the enum's `$repr`, holds.
fn numbers(
    name: &str,
    keys: &[(&str, &Node)],
    range: &std::ops::RangeInclusive<f64>,
) -> Result<Vec<(String, String)>, String> {
    let numbers: Vec<Option<f64>> = keys
        .iter()
        .map(|(_, v)| match v.get("$number") {
            Some(n) if v.is_obj() => n.num(),
            _ => v.val().num(),
        })
        .collect();
    let distinct = numbers
        .iter()
        .enumerate()
        .all(|(i, a)| numbers[..i].iter().all(|b| b != a));
    if keys.is_empty()
        || !distinct
        || !numbers
            .iter()
            .all(|v| v.is_some_and(|v| range.contains(&v) && v.fract() == 0.0))
    {
        return Err(format!(
            "{name}: an enum's members are distinct numbers its `$repr` holds, or tables that carry one as `$number`"
        ));
    }
    Ok(keys
        .iter()
        .zip(numbers)
        .map(|((k, _), v)| (k.to_string(), js_num(v.unwrap_or_default())))
        .collect())
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
        let (val, t, repr) = (n.val(), type_of(n, ty), repr_of(n)?.0);
        rs_leaf(name, val, t, repr, indent, &mut lines, scope)?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, repr_of(n)?.0, indent, &mut lines, scope)?;
    } else {
        let enumed = type_of(n, ty) == "enum";
        let t = members_type(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        if enumed {
            let (repr, range, _) = repr_of(n)?;
            let variants = numbers(name, &keys, &range)?;
            lines.extend(rs_enum(&enum_name(name), &members(&variants), repr, indent));
            lines.push(String::new());
        }
        let mut body: Vec<String> = Vec::new();
        let inner = format!("{indent}    ");
        // A module `use`s what its own items name; the enclosing module's
        // `use`s don't reach into it.
        let outer = scope.uses.take();
        for (k, v) in keys
            .iter()
            .filter(|(_, v)| !enumed || (v.is_obj() && !v.is_wrapper()))
        {
            rs_items(k, v, t, &inner, &mut body, scope)?;
        }
        let uses = scope.uses.replace(outer);
        if !body.is_empty() {
            lines.push(format!("{indent}pub mod {} {{", rs_ident(name)));
            if !uses.is_empty() {
                lines.extend(uses.iter().map(|u| format!("{inner}use {u};")));
                lines.push(String::new());
            }
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

/// `is_name` says whether `s` can name an enum member once it is made an
/// identifier: a letter, a digit or `_`, followed by those or `-`. `-` is
/// allowed past the first character because WebGPU's names hold it
/// ("2d-array"), and an identifier made from such a name spells it `_`.
fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphanumeric() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// This makes a member's name a C++ enumerator: each `-` becomes `_`, a leading
/// digit gets an `_` before it, and a keyword gets one after it.
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

/// This writes a list of names in Rust, as a slice of the names.
fn rs_names(name: &str, names: &[&str], indent: &str) -> String {
    let quoted: Vec<String> = names.iter().map(|n| json_str(n)).collect();
    format!(
        "{indent}pub const {}: &[&str] = &[{}];",
        rs_ident(name),
        quoted.join(", ")
    )
}

/// A list of records is a struct, `<Enum>Record`, and an array of them; an
/// enum's list is also the enum of its members' `name`s, numbered by place.
fn rs_recs(
    name: &str,
    a: &[Node],
    ty: &str,
    repr: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    let fields = fields(a)?.unwrap_or_default();
    let r = format!("{}Record", enum_name(name));
    let num = if is_primitive(ty) { ty } else { "f64" };
    let base = |f: &Field| match f.kind {
        0 => "&'static str".to_string(),
        1 => "bool".to_string(),
        _ => num.to_string(),
    };
    let tys: Vec<String> = fields
        .iter()
        .map(|(_, f)| {
            let t = if f.list {
                format!("&'static [{}]", base(f))
            } else {
                base(f)
            };
            if f.option {
                format!("Option<{t}>")
            } else {
                t
            }
        })
        .collect();
    let lit = |v: &Node, f: &Field| -> Result<String, String> {
        let value = match v {
            Node::Null => return Ok("None".into()),
            Node::Arr(items) => format!(
                "&[{}]",
                items
                    .iter()
                    .map(|x| rs_scalar(x, num, scope).map(|(_, l)| l))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            v => rs_scalar(v, num, scope)?.1,
        };
        Ok(if f.option {
            format!("Some({value})")
        } else {
            value
        })
    };
    let mut rows = Vec::with_capacity(a.len());
    for e in a {
        let row = e
            .named()
            .zip(&fields)
            .map(|((k, v), (_, f))| Ok(format!("{}: {}", rs_ident(k), lit(v, f)?)))
            .collect::<Result<Vec<_>, String>>()?;
        rows.push(format!("{r} {{ {} }}", row.join(", ")));
    }
    let keys: Vec<&str> = fields.iter().map(|(k, _)| *k).collect();
    out.extend([
        format!("{indent}#[derive(Clone, Copy, Debug)]"),
        format!("{indent}pub struct {r} {{"),
    ]);
    out.extend(
        keys.iter()
            .zip(&tys)
            .map(|(k, t)| format!("{indent}    pub {}: {t},", rs_ident(k))),
    );
    out.extend([format!("{indent}}}"), String::new()]);
    out.push(format!(
        "{indent}pub const {}: [{r}; {}] = [{}];",
        rs_ident(name),
        a.len(),
        rows.join(", ")
    ));
    if ty == "enum" {
        let names: Vec<&str> = a.iter().filter_map(|e| e.str_at("name")).collect();
        if !names.iter().all(|n| is_name(n)) {
            return Err(format!("{name}: an enum's members are names"));
        }
        out.push(String::new());
        let variants: Vec<(String, String)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i.to_string()))
            .collect();
        out.extend(rs_enum(&enum_name(name), &members(&variants), repr, indent));
    }
    Ok(())
}

fn rs_leaf(
    name: &str,
    v: &Node,
    ty: &str,
    repr: &str,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    if let Node::Arr(a) = v {
        if fields(a)?.is_some() {
            return rs_recs(name, a, ty, repr, indent, out, scope);
        }
        let strings = matches!(a.first(), Some(Node::Str(_)));
        if strings && enum_path(ty, scope).is_none() {
            let names: Vec<&str> = a.iter().map(|e| e.str().unwrap_or_default()).collect();
            out.push(rs_names(name, &names, indent));
            if ty == "enum" {
                if !a.iter().all(|e| e.str().is_some_and(is_name)) {
                    return Err(format!("{name}: an enum's members are names"));
                }
                let variants: Vec<(String, String)> = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.to_string(), i.to_string()))
                    .collect();
                out.extend(rs_enum(&enum_name(name), &members(&variants), repr, indent));
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

/// A table with `"$packed": true` is laid out end to end. Each field gives its
/// width in words, and the generator writes where the field begins, and the
/// table's `WORDS` where the last field ends. What it writes are indices, so a
/// packed table declares no `$type`: its numbers are a `usize` in Rust, a
/// `std::size_t` in C++ and a number in TypeScript. A section inside the table is
/// itself a packed table, given its `AT`. A width is an element, or `[count,
/// element]`, as a C array is (Cap'n Proto's List(T)). The count is a number,
/// a number named above the field or in a spec it uses (`TABLE.FIELD` reaches
/// into a table), or a list, which stands for its length. The element is one
/// word where it is not given, a number of words or the name of one, a packed
/// table, its `WORDS`, or "bit", the count's bits rounded up to 32-bit words.
/// Growing a field therefore moves everything after it.
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
    let mut out = Vec::with_capacity(entries.len() + 2);
    out.push(("$type".into(), Node::Str("usize".into())));
    for (k, v) in entries {
        if k == "$type" {
            return Err("$type: a packed table's numbers are indices, and it declares none".into());
        }
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
    // A name above the field or in a spec it uses; `TABLE.FIELD` reaches into
    // a table.
    let named = |name: &str| -> Option<&Node> {
        let mut parts = name.split('.');
        let first = parts.next()?;
        let n = above
            .iter()
            .chain(used)
            .find(|(k, _)| k == first)
            .map(|(_, n)| n)?;
        parts.try_fold(n, |n, field| n.get(field))
    };
    let count = |c: &Node| -> Result<f64, String> {
        match c {
            Node::Num(n) => Ok(*n),
            Node::Str(s) => named(s)
                .and_then(|n| match n.val() {
                    Node::Arr(a) => Some(a.len() as f64),
                    n => n.num(),
                })
                .ok_or(format!("`{s}` names no count above it")),
            _ => Err("a count is a number, or the name of one or of a list".into()),
        }
    };
    // An element's words, or None for a bit.
    let element = |e: &Node| -> Result<Option<f64>, String> {
        match e {
            Node::Num(n) => Ok(Some(*n)),
            Node::Str(s) if s == "bit" => Ok(None),
            Node::Str(s) => named(s)
                .and_then(|n| match n {
                    Node::Obj(o) if !n.is_wrapper() => words_of(o),
                    n => n.val().num(),
                })
                .map(Some)
                .ok_or(format!("`{s}` names no packed table or number above it")),
            _ => Err("an element is a number of words, a packed table or \"bit\"".into()),
        }
    };
    match v {
        Node::Arr(a) if (1..=2).contains(&a.len()) => {
            let n = count(&a[0])?;
            Ok(
                match a.get(1).map(element).transpose()?.unwrap_or(Some(1.0)) {
                    Some(words) => n * words,
                    None => (n / WORD_BITS).ceil(),
                },
            )
        }
        Node::Arr(_) => Err("an array is [count] or [count, element]".into()),
        e => element(e)?.ok_or("a bit alone is no width".into()),
    }
}

/// The bits of a word a packed table counts in, which "bit" rounds up to.
const WORD_BITS: f64 = 32.0;

fn read_at(root: &Path, json: &str) -> Result<Node, String> {
    read(&root.join(json).to_string_lossy())
}

/// This answers the names `spec` may read beyond its own, taken from the
/// specs its `$uses` names, by their paths from `root`. Each used spec is laid
/// out over its own uses, each name must be owned by one file, and no chain of
/// uses may be circular; `json` is where `spec` comes from.
fn used(
    root: &Path,
    json: &str,
    spec: &Node,
    path: &mut Vec<String>,
) -> Result<Vec<(String, Node)>, String> {
    if path.iter().any(|p| p == json) {
        return Err(format!("{json}: uses itself through {}", path.join(" → ")));
    }
    path.push(json.to_string());
    let uses: Vec<String> = spec
        .get("$uses")
        .and_then(Node::arr)
        .map(|a| a.iter().filter_map(|u| u.str().map(String::from)).collect())
        .unwrap_or_default();
    let mut names: Vec<(String, Node)> = Vec::new();
    for u in &uses {
        let spec = read_at(root, u)?;
        let below = used(root, u, &spec, path)?;
        let Node::Obj(entries) = laid_out(spec, &below)? else {
            return Err(format!("{u}: not a table"));
        };
        for (k, v) in entries.into_iter().filter(|(k, _)| !k.starts_with('$')) {
            if names.iter().any(|(n, _)| *n == k) {
                return Err(format!(
                    "{json}: `{k}` is named in two of the specs it uses"
                ));
            }
            names.push((k, v));
        }
    }
    path.pop();
    Ok(names)
}

/// Every file that `specs` write, each read from its JSON, with paths relative
/// to `root`.
pub fn outputs_at(root: &Path, specs: &[DataSpec]) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in specs {
        outputs.extend(table_outputs(root, d, read_at(root, d.json)?)?);
    }
    Ok(outputs)
}

/// The files `d` writes from `spec`, a table a tool built in memory or read
/// from `d.json`, which the headers name as its source.
pub fn table_outputs(root: &Path, d: &DataSpec, spec: Node) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    let uses = used(root, d.json, &spec, &mut Vec::new())?;
    let spec = laid_out(spec, &uses)?;
    check_types(&spec).map_err(|e| format!("{}: {e}", d.json))?;
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
        ts_outputs(root, d, &spec, ts_path, &mut outputs)?;
    }
    if let Some(rs_path) = d.rs {
        rs_outputs(root, d, &spec, rs_path, &mut outputs)?;
    }
    if let Some((cpp_path, namespace)) = d.cpp {
        outputs.push(Output::new(
            cpp_path,
            cpp_file(d, &spec, cpp_path, namespace)?,
        ));
    }
    Ok(outputs)
}

fn head(shown: &str, json: &str, spec: &Node) -> Vec<String> {
    let mut out = vec![format!("// {shown}"), "//".into(), format!("// {}", mark(json, "xpute-gen data.rs"))];
    if let Some(doc) = spec.lines_at("$doc") {
        out.push("//".into());
        out.extend(line_comments(&doc, ""));
    }
    out
}

fn ts_outputs(
    root: &Path,
    d: &DataSpec,
    spec: &Node,
    ts_path: &str,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let mut ts = head(&package_path(root, ts_path)?, d.json, spec);
    let mut body: Vec<String> = Vec::new();
    for (name, n) in spec.named() {
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
        // A value is written as a constant, a table or a list as one too,
        // `as const`.
        let whole = matches!(n.val(), Node::Arr(_) | Node::Obj(_));
        body.push(format!(
            "export const {name} = {}{};",
            ts_literal(n, "f64", "")?,
            if whole { " as const" } else { "" }
        ));
        if type_of(n, "f64") != "enum" {
            continue;
        }
        // A list's members are its names, or its records' `name`s, numbered by
        // place; a table's, its keys by their numbers.
        let members: Vec<(String, String)> = if n.is_wrapper() {
            let a = n.val().arr().unwrap_or_default();
            a.iter()
                .enumerate()
                .map(|(k, e)| {
                    (
                        e.str().or(e.str_at("name")).unwrap_or_default().to_string(),
                        k.to_string(),
                    )
                })
                .collect()
        } else {
            let entries: Vec<(&str, &Node)> = n.named().collect();
            numbers(name, &entries, &repr_of(n)?.1)?
        };
        body.extend(ts_enum(&enum_name(name), &members));
    }
    ts.extend(body);
    outputs.push(Output::new(ts_path, file(&ts)));
    Ok(())
}

/// Whether `n` holds a list or a table whose `$type` is `enum`.
fn has_enum(n: &Node) -> bool {
    n.str_at("$type") == Some("enum")
        || match n {
            Node::Obj(entries) => entries.iter().any(|(_, v)| has_enum(v)),
            Node::Arr(a) => a.iter().any(has_enum),
            _ => false,
        }
}

fn rs_outputs(
    root: &Path,
    d: &DataSpec,
    spec: &Node,
    rs_path: &str,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let shown = package_path(root, rs_path)?;
    let mut rs = head(&shown, d.json, spec);
    rs.push(String::new());
    rs.extend(casing_note());
    rs.extend([
        "// A table is generated whole, whether or not this crate calls an entry.".to_string(),
        format!(
            "#![allow(non_upper_case_globals, non_snake_case, {}dead_code, reason = \"generated from the spec\")]",
            if has_enum(spec) { "non_camel_case_types, " } else { "" }
        ),
    ]);
    let scope = Scope {
        uses: RefCell::default(),
    };
    let mut body = Vec::new();
    for (name, n) in spec.named() {
        let mut items = Vec::new();
        rs_items(name, n, "f64", "", &mut items, &scope)?;
        if !items.is_empty() {
            body.push(String::new());
            body.extend(items);
        }
    }
    let uses = scope.uses.borrow();
    if !uses.is_empty() {
        rs.push(String::new());
        rs.extend(uses.iter().map(|u| format!("use {u};")));
    }
    rs.extend(body);
    outputs.push(Output::new(rs_path, file(&rs)));
    Ok(())
}

fn cpp_file(d: &DataSpec, spec: &Node, cpp_path: &str, namespace: &str) -> Result<String, String> {
    let mut cpp = head(cpp_path, d.json, spec);
    let lists = spec.named().any(|(_, n)| matches!(n.val(), Node::Arr(_)));
    let packed = spec.named().any(|(_, n)| match n {
        Node::Obj(t) => t
            .iter()
            .any(|(f, x)| f == "$packed" && *x == Node::Bool(true)),
        _ => false,
    });
    cpp.extend([String::new(), "#pragma once".into(), String::new()]);
    if lists {
        cpp.push("#include <array>".into());
    }
    if packed {
        cpp.push("#include <cstddef>".into());
    }
    cpp.push("#include <cstdint>".into());
    if lists {
        cpp.push("#include <string_view>".into());
    }
    cpp.extend([String::new(), format!("namespace {namespace} {{")]);
    for (name, n) in spec.named() {
        cpp_items(name, n, "f64", &mut cpp)?;
    }
    cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&cpp))
}

/// A list of names is written as a `string_view` array, and one whose `$type`
/// is `enum` also as an enum class numbered by place. Any other value that is
/// not a number is refused.
fn cpp_items(name: &str, n: &Node, ty: &str, out: &mut Vec<String>) -> Result<(), String> {
    let doc = if n.is_obj() { n.lines_at("$doc") } else { None };
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(line_comments(&doc, ""));
    }
    if n.is_obj() && !n.is_wrapper() {
        let t = members_type(n, ty);
        out.push(format!("namespace {name} {{"));
        for (k, v) in n.named() {
            cpp_items(k, v, t, out)?;
        }
        out.push(format!("}} // namespace {name}"));
        return Ok(());
    }
    if let Node::Arr(list) = if n.is_wrapper() { n.val() } else { n } {
        let names = list
            .iter()
            .map(|v| {
                v.str()
                    .ok_or(format!("{name}: a C++ list takes names only"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if type_of(n, ty) == "enum" {
            let held = repr_of(n)?.2;
            out.push(format!("enum class {} : {held} {{", enum_name(name)));
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
        (n.val(), type_of(n, ty))
    } else {
        (n, ty)
    };
    let number = val
        .num()
        .ok_or(format!("{name}: a C++ header takes numbers only"))?;
    let (ty, suffix) = match t {
        "u32" => ("std::uint32_t", "u"),
        "i32" => ("std::int32_t", ""),
        "usize" => ("std::size_t", "u"),
        other => {
            return Err(format!(
                "{name}: a C++ header takes u32, i32 and usize, not {other}"
            ))
        }
    };
    out.push(format!(
        "inline constexpr {ty} {name} = {}{suffix};",
        js_num(number)
    ));
    Ok(())
}
