// xpute-gen/spec/data.rs

//! A data spec is a JSON file of named values, written into the files each spec
//! names as TypeScript `as const` values, Rust constants and C++ constants. A
//! number is an f64 unless a `$type` says otherwise. A value wrapped as `{
//! "$val", "$doc" }` carries its reason in `$doc`. Each language names a value
//! in its own case (in Rust an enum's member Pascal, a constant SCREAMING, a
//! module or a field snake), save an enum whose names are copied from outside,
//! marked `"$outside": true`, whose Rust members keep their spelling. A table
//! may be laid out in words (`laid_out`), with its widths read from the specs
//! it uses. A list of names whose `$type` is `enum` is an enum, in Rust and in
//! C++ an enum class, numbered by place; a table whose `$type` is `enum` is one
//! too, numbered by its members, each a number or a table that carries its
//! `$number`. An enum is named as its key is, singular; one whose names a
//! reader takes, marked `"$names": true`, has them written in TypeScript and
//! C++ as a list under the key's plural (`STEP_NAMES` beside `STEP_NAME`), as
//! Rust's enum holds them in `name`. Every enum is held in its `$repr` (`u8`
//! where it gives none); in Rust and C++ it has `of`, a number to its member,
//! in Rust its members' names, and in TypeScript each member is a constant of
//! its own (`ts_enum`), so no name reaches a bundle. A list of tables of the
//! same fields is a list of records: an array of objects in TypeScript, and in
//! Rust an array of a struct (`rs_recs`), its row named by the list's `$row`
//! where its key is a plural (`"$row": "asset_file"`, `AssetFile`), whose
//! `name`s are the enum where the list's `$type` is `enum`, numbered by their
//! places or, where every record carries one, each by its `$number`; there the
//! TypeScript rows carry each one's `number` and no `name`, and the records of
//! an enum's table carry their `number` beside their key (`numbered`). A table
//! a tool builds in memory is written as a read one is (`table_outputs`).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use super::{
    align_star, cpp_enum, cpp_member, file, is_ts_ident, js_num, json_str, line_comments, mark,
    package_path, pascal, plural, read, rs_const, rs_doc, rs_enum, rs_field, rs_ident, rs_member, ts_doc,
    ts_enum, Includes, Member, Node, Output,
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
    ts_value(n, ty, indent, &[])
}

/// `ts_literal`, where a name whose `$type` is one of `enums`, the enums the
/// spec declares, is its member's number, typed by the enum, as Rust writes
/// the member.
fn ts_value(n: &Node, ty: &str, indent: &str, enums: &[(String, Vec<Member>)]) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_value(n.val(), type_of(n, ty), indent, enums);
    }
    Ok(match n {
        Node::Num(v) => format!("{} as number", js_num(*v)),
        Node::Bool(b) => b.to_string(),
        Node::Str(s) => match enums.iter().find(|(e, _)| e == ty) {
            Some((e, members)) => {
                let m = members.iter().find(|m| m.name == *s).ok_or(format!("{s} is no {e}"))?;
                format!("{} as {e}", m.number)
            }
            None => json_str(s),
        },
        Node::Arr(a) => format!(
            "[{}]",
            a.iter()
                .map(|e| ts_value(e, ty, indent, enums))
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
                    ts_value(v, t, &inner, enums)?
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
    /// The enums the spec declares, which a `$type` names by their name alone,
    /// each with how its members are named (`casing`).
    enums: Vec<(String, Casing)>,
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

/// The enums `spec` declares, each by its name, with how its members are
/// named.
fn enums(spec: &Node) -> Vec<(String, Casing)> {
    spec.named()
        .filter(|(_, n)| n.str_at("$type") == Some("enum"))
        .map(|(k, n)| (pascal(k), casing(n)))
        .collect()
}

/// A member's name in the language written, made from its name in the spec.
type Casing = fn(&str) -> String;

/// How the enum `n`'s members are named in Rust: in Pascal case, or as their
/// source spells them where the enum says `"$outside": true`, its names being
/// copied from outside (POSIX's `EBADMSG`).
fn casing(n: &Node) -> Casing {
    if n.get("$outside") == Some(&Node::Bool(true)) {
        rs_ident
    } else {
        rs_member
    }
}

/// Whether the enum `n`'s members are read by their names, which it marks
/// `"$names": true`: its names are then written in TypeScript and C++ as a
/// list of their own, under the plural of its name (`plural`).
fn names_read(n: &Node) -> bool {
    n.get("$names") == Some(&Node::Bool(true))
}

/// What a value's marks say of how it is written in Rust: an enum's `$repr`,
/// the numbers it holds and how its members are named, and the type a list of
/// records' row takes, its `$row`, the singular its plural key does not give.
struct Marks<'a> {
    repr: &'static str,
    range: std::ops::RangeInclusive<f64>,
    casing: Casing,
    row: Option<&'a str>,
}

impl<'a> Marks<'a> {
    fn of(n: &'a Node) -> Result<Marks<'a>, String> {
        let (repr, range, _) = repr_of(n)?;
        Ok(Marks {
            repr,
            range,
            casing: casing(n),
            row: n.str_at("$row"),
        })
    }
}

/// Whether `n`'s value is a list of names.
fn is_names(n: &Node) -> bool {
    n.val().arr().is_some_and(|a| a.iter().all(|e| e.str().is_some()))
}

/// Refuses a `$type` that names no primitive, no enum the spec declares and
/// no enum by its path (`module::Enum`), and `enum` on anything but a list or
/// a table.
fn check_types(spec: &Node) -> Result<(), String> {
    fn walk(n: &Node, at: &str, enums: &[String]) -> Result<(), String> {
        if n.get("$repr").is_some() {
            if n.str_at("$type") != Some("enum") {
                return Err(format!("{at}: `$repr` holds an enum, beside `$type` enum"));
            }
            repr_of(n).map_err(|e| format!("{at}: {e}"))?;
        }
        if let Some(r) = n.get("$row") {
            if r.str().is_none() || n.val().arr().and_then(|a| fields(a).ok().flatten()).is_none() {
                return Err(format!("{at}: `$row` names the row of a list of records"));
            }
        }
        if let Some(o) = n.get("$names") {
            if n.str_at("$type") != Some("enum") || !is_names(n) || *o != Node::Bool(true) {
                return Err(format!("{at}: `$names` is true, beside `$type` enum on a list of names"));
            }
        }
        if let Some(o) = n.get("$outside") {
            if n.str_at("$type") != Some("enum") || *o != Node::Bool(true) {
                return Err(format!("{at}: `$outside` is true, beside `$type` enum"));
            }
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
                || enums.iter().any(|e| e == ty)
                || ty.contains("::");
            if !known {
                return Err(format!("{at}: `$type` {ty} is no type the generator knows"));
            }
        }
        match n {
            Node::Obj(entries) => {
                for (k, v) in entries.iter().filter(|(k, _)| !k.starts_with('$') || k == "$val") {
                    walk(v, &format!("{at}.{k}"), enums)?;
                }
            }
            Node::Arr(a) => {
                for (i, v) in a.iter().enumerate() {
                    walk(v, &format!("{at}[{i}]"), enums)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let enums: Vec<String> = enums(spec).into_iter().map(|(e, _)| e).collect();
    spec.named().try_for_each(|(k, v)| walk(v, k, &enums))
}

/// The enum `ty` names, and how its members are named: one the spec declares,
/// or one by its path, whose members are in Pascal case.
fn enum_path(ty: &str, scope: &Scope) -> Option<(String, Casing)> {
    if let Some((e, v)) = scope.enums.iter().find(|(e, _)| e == ty) {
        return Some((e.clone(), *v));
    }
    (!is_primitive(ty) && ty.contains("::")).then(|| (scope.name_of(ty), rs_member as Casing))
}

fn rs_scalar(n: &Node, ty: &str, scope: &Scope) -> Result<(String, String), String> {
    Ok(match n {
        Node::Bool(b) => ("bool".to_string(), b.to_string()),
        Node::Str(s) => match enum_path(ty, scope) {
            None => ("&str".to_string(), json_str(s)),
            Some((path, casing)) => (path.clone(), format!("{path}::{}", casing(s))),
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
    repr_named(n.str_at("$repr").unwrap_or("u8"))
}

fn repr_named(r: &str) -> Result<Repr, String> {
    REPRS
        .iter()
        .find(|(name, ..)| *name == r)
        .map(|&(name, lo, hi, cpp)| (name, lo..=hi, cpp))
        .ok_or(format!("`$repr` {r} is no integer an enum is held in"))
}

/// The members of `n`, the enum `name`, as every writer numbers them: a
/// list's (`list_members`), or a table's, its keys by their numbers.
pub fn enum_members(name: &str, n: &Node) -> Result<Vec<Member>, String> {
    let range = repr_of(n)?.1;
    if n.is_wrapper() {
        return list_members(name, n.val().arr().unwrap_or_default(), &range, casing(n));
    }
    let entries: Vec<(&str, &Node)> = n.named().collect();
    numbers(name, &entries, &range, casing(n))
}

/// A list's members: its names, or its records' `name`s, numbered by their
/// places, or each by the `$number` its record carries where every one
/// carries one, and documented by its record's `$doc`.
fn list_members(
    name: &str,
    a: &[Node],
    range: &std::ops::RangeInclusive<f64>,
    casing: Casing,
) -> Result<Vec<Member>, String> {
    let numbered = a.iter().filter(|e| e.get("$number").is_some()).count();
    if numbered != 0 && numbered != a.len() {
        return Err(format!(
            "{name}: a list's members are numbered by their places, or each by its `$number`"
        ));
    }
    let keys: Vec<(&str, Node)> = a
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let number = e.get("$number").cloned().unwrap_or(Node::Num(k as f64));
            let mut m = vec![("$number".to_string(), number)];
            m.extend(e.get("$doc").map(|d| ("$doc".to_string(), d.clone())));
            (e.str().or(e.str_at("name")).unwrap_or_default(), Node::Obj(m))
        })
        .collect();
    let keys: Vec<(&str, &Node)> = keys.iter().map(|(k, v)| (*k, v)).collect();
    numbers(name, &keys, range, casing)
}

/// An enum table's members: each is a number, or a table that carries its
/// `$number` and may carry its `$doc`; the numbers are distinct whole numbers
/// that `range`, the enum's `$repr`, holds.
fn numbers(
    name: &str,
    keys: &[(&str, &Node)],
    range: &std::ops::RangeInclusive<f64>,
    casing: Casing,
) -> Result<Vec<Member>, String> {
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
        .map(|((k, v), n)| Member {
            ident: casing(k),
            name: k.to_string(),
            number: js_num(n.unwrap_or_default()),
            doc: v.is_obj().then(|| v.lines_at("$doc")).flatten().unwrap_or_default(),
        })
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
        rs_leaf(name, n.val(), type_of(n, ty), &Marks::of(n)?, indent, &mut lines, scope)?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, &Marks::of(n)?, indent, &mut lines, scope)?;
    } else {
        let enumed = type_of(n, ty) == "enum";
        let t = members_type(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        if enumed {
            let (repr, range, _) = repr_of(n)?;
            let variants = numbers(name, &keys, &range, casing(n))?;
            lines.extend(rs_enum(&pascal(name), &variants, repr, indent));
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
            lines.push(format!("{indent}pub mod {} {{", rs_field(name)));
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
/// ("2d-array"), and an identifier made from such a name breaks a word there.
fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphanumeric() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// This writes a list of names in Rust, as a slice of the names.
fn rs_names(name: &str, names: &[&str], indent: &str) -> String {
    let quoted: Vec<String> = names.iter().map(|n| json_str(n)).collect();
    format!(
        "{indent}pub const {}: &[&str] = &[{}];",
        rs_const(name),
        quoted.join(", ")
    )
}

/// A list of records is a struct, `<Enum>Record`, and an array of them; an
/// enum's list is also the enum of its members' `name`s, numbered by place.
fn rs_recs(
    name: &str,
    a: &[Node],
    ty: &str,
    marks: &Marks,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    let fields = fields(a)?.unwrap_or_default();
    let r = marks.row.map_or(format!("{}Record", pascal(name)), pascal);
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
            .map(|((k, v), (_, f))| Ok(format!("{}: {}", rs_field(k), lit(v, f)?)))
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
            .map(|(k, t)| format!("{indent}    pub {}: {t},", rs_field(k))),
    );
    out.extend([format!("{indent}}}"), String::new()]);
    out.push(format!(
        "{indent}pub const {}: [{r}; {}] = [{}];",
        rs_const(name),
        a.len(),
        rows.join(", ")
    ));
    if ty == "enum" {
        let names: Vec<&str> = a.iter().filter_map(|e| e.str_at("name")).collect();
        if !names.iter().all(|n| is_name(n)) {
            return Err(format!("{name}: an enum's members are names"));
        }
        out.push(String::new());
        let members = list_members(name, a, &marks.range, marks.casing)?;
        out.extend(rs_enum(&pascal(name), &members, marks.repr, indent));
    }
    Ok(())
}

fn rs_leaf(
    name: &str,
    v: &Node,
    ty: &str,
    marks: &Marks,
    indent: &str,
    out: &mut Vec<String>,
    scope: &Scope,
) -> Result<(), String> {
    if let Node::Arr(a) = v {
        if fields(a)?.is_some() {
            return rs_recs(name, a, ty, marks, indent, out, scope);
        }
        let strings = matches!(a.first(), Some(Node::Str(_)));
        if strings && enum_path(ty, scope).is_none() {
            let names: Vec<&str> = a.iter().map(|e| e.str().unwrap_or_default()).collect();
            if ty != "enum" {
                out.push(rs_names(name, &names, indent));
                return Ok(());
            }
            if !a.iter().all(|e| e.str().is_some_and(is_name)) {
                return Err(format!("{name}: an enum's members are names"));
            }
            let members = list_members(name, a, &marks.range, marks.casing)?;
            out.extend(rs_enum(&pascal(name), &members, marks.repr, indent));
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
            rs_const(name),
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
        rs_const(name)
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
fn uses_of(
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
        let below = uses_of(root, u, &spec, path)?;
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
    let uses = uses_of(root, d.json, &spec, &mut Vec::new())?;
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
    let enums = spec
        .named()
        .filter(|(_, n)| n.str_at("$type") == Some("enum"))
        .map(|(k, n)| Ok((pascal(k), enum_members(k, n)?)))
        .collect::<Result<Vec<_>, String>>()?;
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
        // `as const`. An enum table whose members carry nothing past their
        // numbers is its members' constants alone.
        let enumed = type_of(n, "f64") == "enum";
        let names = enumed && is_names(n);
        let bare = enumed
            && (names
                || !n.is_wrapper() && n.named().all(|(_, v)| !v.is_obj() || v.named().next().is_none()));
        let members = if enumed {
            enum_members(name, n)?
        } else {
            Vec::new()
        };
        if names && names_read(n) {
            body.push(format!(
                "export const {} = {} as const;",
                plural(name),
                ts_value(n.val(), "f64", "", &enums)?
            ));
            body.push(String::new());
        }
        if !bare {
            let whole = matches!(n.val(), Node::Arr(_) | Node::Obj(_));
            body.push(format!(
                "export const {name} = {}{};",
                ts_value(&numbered(n, &members), "f64", "", &enums)?,
                if whole { " as const" } else { "" }
            ));
        }
        if !enumed {
            continue;
        }
        // Its constants follow the doc where it stands for the table.
        let consts = ts_enum(&pascal(name), &members);
        body.extend(consts.into_iter().skip(usize::from(bare)));
    }
    ts.extend(body);
    outputs.push(Output::new(ts_path, file(&ts)));
    Ok(())
}

/// `n` as TypeScript writes it: an enum's list of records with each record's
/// member number in place of its name, which TypeScript does not write; an
/// enum's table of records with each record's member number beside its key;
/// any other value as it is.
fn numbered(n: &Node, members: &[Member]) -> Node {
    let number = |m: &Member| Node::Num(m.number.parse().unwrap_or_default());
    let rows = match n.val() {
        Node::Arr(rows) if !members.is_empty() && rows.iter().all(Node::is_obj) => Node::Arr(
            rows.iter()
                .zip(members)
                .map(|(row, m)| {
                    let mut out = vec![("number".to_string(), number(m))];
                    out.extend(row.entries().iter().filter(|(k, _)| k != "name").cloned());
                    Node::Obj(out)
                })
                .collect(),
        ),
        Node::Obj(entries) if !members.is_empty() && n.named().all(|(_, v)| v.is_obj()) => {
            Node::Obj(
                entries
                    .iter()
                    .map(|(k, v)| match members.iter().find(|m| m.name == *k) {
                        Some(m) => {
                            let mut out = vec![("number".to_string(), number(m))];
                            out.extend(v.entries().iter().cloned());
                            (k.clone(), Node::Obj(out))
                        }
                        None => (k.clone(), v.clone()),
                    })
                    .collect(),
            )
        }
        _ => return n.clone(),
    };
    match n {
        Node::Obj(entries) if n.is_wrapper() => Node::Obj(
            entries
                .iter()
                .map(|(k, v)| {
                    let v = if k == "$val" { rows.clone() } else { v.clone() };
                    (k.clone(), v)
                })
                .collect(),
        ),
        _ => rows,
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
    let scope = Scope {
        uses: RefCell::default(),
        enums: enums(spec),
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
    let mut body = Vec::new();
    let mut includes = BTreeSet::new();
    for (name, n) in spec.named() {
        cpp_items(name, n, "f64", &mut body, &mut includes)?;
    }
    let mut cpp = head(cpp_path, d.json, spec);
    cpp.extend([String::new(), "#pragma once".into(), String::new()]);
    cpp.extend(includes.iter().map(|h| format!("#include <{h}>")));
    cpp.extend([String::new(), format!("namespace {namespace} {{")]);
    cpp.extend(body);
    cpp.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&cpp))
}

/// A table is a namespace, and one whose `$type` is `enum` also an enum
/// (`cpp_enum`), its members' own values in the namespace, each under its
/// member's name; a table that holds no value writes nothing. A list of names
/// is written as a `string_view` array, and one whose `$type` is `enum` also
/// as an enum. Any other value is a leaf (`cpp_leaf`).
fn cpp_items(
    name: &str,
    n: &Node,
    ty: &str,
    out: &mut Vec<String>,
    inc: &mut Includes,
) -> Result<(), String> {
    let mut lines: Vec<String> = Vec::new();
    if let Node::Arr(list) = n.val() {
        let names = list
            .iter()
            .map(|v| v.str().ok_or(format!("{name}: a C++ list takes names only")))
            .collect::<Result<Vec<_>, _>>()?;
        let enumed = type_of(n, ty) == "enum";
        if enumed {
            let (_, range, repr) = repr_of(n)?;
            cpp_enum(name, &list_members(name, list, &range, cpp_member)?, repr, &mut lines, inc);
        }
        if !enumed || names_read(n) {
            let key = if enumed { plural(name) } else { name.to_string() };
            inc.extend(["array", "string_view"]);
            let quoted: Vec<String> = names.iter().map(|v| json_str(v)).collect();
            lines.extend(enumed.then(String::new));
            lines.push(format!(
                "inline constexpr std::array<std::string_view, {}> {key}{{{}}};",
                names.len(),
                quoted.join(", ")
            ));
        }
    } else if n.is_obj() && !n.is_wrapper() {
        let enumed = type_of(n, ty) == "enum";
        let t = members_type(n, ty);
        let mut body = Vec::new();
        for (k, v) in n.named() {
            if enumed && (!v.is_obj() || v.is_wrapper()) {
                continue;
            }
            let k = if enumed { cpp_member(k) } else { k.to_string() };
            cpp_items(&k, v, t, &mut body, inc)?;
        }
        if enumed {
            let (_, range, repr) = repr_of(n)?;
            let keys: Vec<(&str, &Node)> = n.named().collect();
            cpp_enum(name, &numbers(name, &keys, &range, cpp_member)?, repr, &mut lines, inc);
        }
        if !body.is_empty() {
            if enumed {
                lines.push(String::new());
            }
            lines.push(format!("namespace {name} {{"));
            lines.extend(body);
            lines.push(format!("}} // namespace {name}"));
        }
    } else {
        lines.push(cpp_leaf(name, n.val(), type_of(n, ty), inc)?);
    }
    if lines.is_empty() {
        return Ok(());
    }
    out.push(String::new());
    if let Some(doc) = n.is_obj().then(|| n.lines_at("$doc")).flatten() {
        out.extend(line_comments(&doc, ""));
    }
    out.extend(lines);
    Ok(())
}

/// A string is a `string_view`, and a number a constant of its `$type`, u32,
/// i32 or usize. Any other value is refused.
fn cpp_leaf(name: &str, v: &Node, t: &str, inc: &mut Includes) -> Result<String, String> {
    if let Node::Str(s) = v {
        if !is_primitive(t) {
            return Err(format!("{name}: a C++ header takes no string typed {t}"));
        }
        inc.insert("string_view");
        return Ok(format!("inline constexpr std::string_view {name} = {};", json_str(s)));
    }
    let number = v
        .num()
        .ok_or(format!("{name}: a C++ header takes numbers and strings only"))?;
    let (ty, header, suffix) = match t {
        "u32" => ("std::uint32_t", "cstdint", "u"),
        "i32" => ("std::int32_t", "cstdint", ""),
        "usize" => ("std::size_t", "cstddef", "u"),
        other => {
            return Err(format!(
                "{name}: a C++ header takes u32, i32 and usize, not {other}"
            ))
        }
    };
    inc.insert(header);
    Ok(format!("inline constexpr {ty} {name} = {}{suffix};", js_num(number)))
}
