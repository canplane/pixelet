// xpute-gen/spec/data.rs

//! A data spec is a JSON5 file of named values in DTCG's format, its own marks
//! under a node's `$extensions` (`MARKS`), written into the files each spec
//! names as TypeScript `as const` values, Rust constants and C++ constants.
//! Every number has a `$type`, its own or its group's, which it is checked
//! against before it is written (`typed`). A token's `$description` is its
//! reason. Each language names a value in its own case (in Rust an enum's
//! member Pascal, a constant SCREAMING, a module or a field snake), save an enum whose names are copied from outside, marked
//! `"outside": true`, whose Rust members keep their spelling. A table may be
//! laid out in words (`laid_out`), with its widths read from the specs it uses;
//! a `$type` may name an enum a spec it uses declares, which Rust and
//! TypeScript reach where that spec's own output is (`Foreign`), and C++
//! refuses. A table with `"keys": "<Enum>"` holds a value for each of the
//! enum's members and is written as a list by the member's number (`keyed`); a
//! table whose `"each"` names an enum is one table for each member, named as
//! the member is (`expanded`). Under a table with `"distinct": "<field>"`, no
//! two sibling tables hold the same `<field>` (`check_distinct`). A list of
//! names whose `$type` is `enum` is an enum, in Rust and in C++ an enum class,
//! numbered by place; a table whose `$type` is `enum` is one too, numbered by
//! its members, each a number or a table that carries its `number`. An enum is
//! named as its key is, singular; one whose names a reader takes, marked
//! `"names": true`, has them written in TypeScript and C++ as a list under the
//! key's plural (`STEP_NAMES` beside `STEP_NAME`), as Rust's enum holds them in
//! `name`. Every enum is held in its `repr` (`u8` where it gives none); in Rust
//! and C++ it has `of`, a number to its member, in Rust its members' names, and
//! in TypeScript each member is a constant of its own (`ts_enum`), so no name
//! reaches a bundle. A list of tables of the same fields is a list of records:
//! an array of objects in TypeScript, and in Rust an array of a struct
//! (`rs_recs`), its row named by the list's `row` where its key is a plural
//! (`"row": "asset_file"`, `AssetFile`), a field typed by its own `$type` or
//! the list's. Where the list's `$type` is `enum`, its records'
//! `name`s are the enum, numbered by their places or, where every record marks
//! one, each by its `number` mark, and a record holds no number; there the
//! TypeScript rows carry each one's `number` and no `name`, and the records of an enum's table carry their
//! `number` beside their key (`numbered`). An enum table's records carry the
//! same fields, and one whose `$type` names another enum of the spec maps each
//! member to that enum's, written as a function of the member in every language
//! (`mapped`: `Errno::kind` in Rust, `kind_Errno` in TypeScript and C++). A
//! table a tool builds in memory is typed and written as a read one is
//! (`table_outputs`).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use super::{
    Includes, Int, MARKS, Member, Node, Output, PACKED_TYPE, align_star, cpp_enum, cpp_member,
    file, int_of, is_float, is_packed, is_ts_ident, js_num, json_str, line_comments, mark, marks,
    package, package_path, parse, pascal, plural, read, rs_const, rs_doc, rs_enum, rs_field,
    rs_ident, rs_member, rs_module, screaming, snake, ts_const, ts_doc, ts_enum, ts_module,
    ts_number, typed,
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
/// whether a record may leave it null, and the `$type` its token gives it,
/// none where it takes the list's.
#[derive(Clone, Copy, PartialEq)]
struct Field<'a> {
    kind: u8,
    list: bool,
    option: bool,
    ty: Option<&'a str>,
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
/// every member. A field may be a token whose `$type` is its own, the same in
/// every member. None where the list's members are not tables.
fn fields(a: &[Node]) -> Result<Option<Vec<(&str, Field<'_>)>>, String> {
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
            let ty = v.str_at("$type");
            let shape = match v.val() {
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
                            ty,
                        },
                        _ => return Err(format!("[{i}].{k}: a list of one kind of value")),
                    }
                }
                v => match scalar_kind(v) {
                    Some(kind) => Field {
                        kind,
                        list: false,
                        option: false,
                        ty,
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

/// Writes `n`, a table a tool built, as a TypeScript literal, its numbers
/// typed as a spec's are (`typed`).
pub fn ts_literal(n: Node, indent: &str) -> Result<String, String> {
    ts_value(&typed(n)?, "f64", indent, &[])
}

/// `ts_literal`, where a name whose `$type` is one of `enums`, the enums the
/// spec declares, is its member's number, typed by the enum, as Rust writes
/// the member.
fn ts_value(
    n: &Node,
    ty: &str,
    indent: &str,
    enums: &[(String, Vec<Member>)],
) -> Result<String, String> {
    if n.is_wrapper() {
        return ts_value(n.val(), type_of(n, ty), indent, enums);
    }
    Ok(match n {
        Node::Num(v) => {
            let ts = ts_number(ty);
            let suffix = if ts == "bigint" { "n" } else { "" };
            format!("{}{suffix} as {ts}", js_num(*v))
        }
        Node::Bool(b) => b.to_string(),
        Node::Str(s) => match enums.iter().find(|(e, _)| e == ty) {
            Some((e, members)) => {
                let m = members
                    .iter()
                    .find(|m| m.name == *s)
                    .ok_or(format!("{s} is no {e}"))?;
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
                if let Some(doc) = v.is_obj().then(|| v.doc()).flatten() {
                    lines.extend(
                        ts_doc(&doc, &inner)
                            .into_iter()
                            .enumerate()
                            .map(|(i, l)| if i == 0 { l } else { align_star(l, &inner) }),
                    );
                }
                let key = if is_ts_ident(k) {
                    k.to_string()
                } else {
                    json_str(k)
                };
                lines.push(format!("{inner}{key}: {},", ts_value(v, t, &inner, enums)?));
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
    /// The enums the specs it uses declare, which a `$type` names by their
    /// name alone too, each by its path.
    foreign: Vec<(String, Casing, String)>,
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
    int_of(ty).is_some() || is_float(ty) || ty == "str"
}

/// A member's name in the language written, made from its name in the spec.
type Casing = fn(&str) -> String;

/// How the enum `n`'s members are named in Rust: in Pascal case, or as their
/// source spells them where the enum says `"outside": true`, its names being
/// copied from outside (POSIX's `EBADMSG`).
fn casing(n: &Node) -> Casing {
    if n.mark("outside") == Some(&Node::Bool(true)) {
        rs_ident
    } else {
        rs_member
    }
}

/// Whether the enum `n`'s members are read by their names, which it marks
/// `"names": true`: its names are then written in TypeScript and C++ as a
/// list of their own, under the plural of its name (`plural`).
fn names_read(n: &Node) -> bool {
    n.mark("names") == Some(&Node::Bool(true))
}

/// What a value's marks say of how it is written in Rust: an enum's `repr`
/// and how its members are named, and the type a list of records' row takes,
/// its `row`, the singular its plural key does not give.
struct Marks<'a> {
    repr: &'static Int,
    casing: Casing,
    row: Option<&'a str>,
}

impl<'a> Marks<'a> {
    fn of(n: &'a Node) -> Result<Marks<'a>, String> {
        Ok(Marks {
            repr: repr_of(n)?,
            casing: casing(n),
            row: n.mark("row").and_then(Node::str),
        })
    }
}

/// Whether `n`'s value is a list of names.
fn is_names(n: &Node) -> bool {
    n.val()
        .arr()
        .is_some_and(|a| a.iter().all(|e| e.str().is_some()))
}

/// Refuses a `$type` that names no primitive, no enum the spec or a spec it
/// uses declares and no enum by its path (`module::Enum`), and `enum` on anything but a list or
/// a table.
fn check_types(spec: &Node, declared: &[(String, Node)]) -> Result<(), String> {
    fn walk(n: &Node, at: &str, enums: &[String]) -> Result<(), String> {
        if n.mark("repr").is_some() {
            if n.str_at("$type") != Some("enum") {
                return Err(format!("{at}: `repr` holds an enum, beside `$type` enum"));
            }
            repr_of(n).map_err(|e| format!("{at}: {e}"))?;
        }
        if let Some(r) = n.mark("row")
            && (r.str().is_none()
                || n.val()
                    .arr()
                    .and_then(|a| fields(a).ok().flatten())
                    .is_none())
        {
            return Err(format!("{at}: `row` names the row of a list of records"));
        }
        if let Some(o) = n.mark("names")
            && (n.str_at("$type") != Some("enum") || !is_names(n) || *o != Node::Bool(true))
        {
            return Err(format!(
                "{at}: `names` is true, beside `$type` enum on a list of names"
            ));
        }
        if let Some(f) = n.mark("distinct")
            && (f.str().is_none() || !n.is_obj() || n.is_wrapper())
        {
            return Err(format!("{at}: `distinct` names a field, on a table"));
        }
        if let Some(o) = n.mark("outside")
            && (n.str_at("$type") != Some("enum") || *o != Node::Bool(true))
        {
            return Err(format!("{at}: `outside` is true, beside `$type` enum"));
        }
        if let (Some("enum"), Node::Arr(a)) = (n.str_at("$type"), n.val()) {
            let r = repr_of(n).map_err(|e| format!("{at}: {e}"))?;
            if !r.holds(a.len() as f64 - 1.0) {
                let cnt = a.len();
                return Err(format!(
                    "{at}: {cnt} members are more than a {} numbers",
                    r.name
                ));
            }
            let number = |v: &Node| match v.val() {
                Node::Arr(x) => x.iter().any(|x| x.num().is_some()),
                v => v.num().is_some(),
            };
            if a.iter().any(|e| e.named().any(|(_, v)| number(v))) {
                return Err(format!(
                    "{at}: an enum's records hold no number, each marking its `number`"
                ));
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
                for (k, v) in entries
                    .iter()
                    .filter(|(k, _)| !k.starts_with('$') || k == "$value")
                {
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
    let enums: Vec<String> = declared_enums(declared).map(|(k, _)| pascal(k)).collect();
    spec.named().try_for_each(|(k, v)| walk(v, k, &enums))
}

/// The enum `ty` names, and how its members are named: one the spec declares,
/// or one by its path, whose members are in Pascal case.
fn enum_path(ty: &str, scope: &Scope) -> Option<(String, Casing)> {
    if let Some((e, v)) = scope.enums.iter().find(|(e, _)| e == ty) {
        return Some((e.clone(), *v));
    }
    if let Some((_, v, path)) = scope.foreign.iter().find(|(e, ..)| e == ty) {
        return Some((scope.name_of(path), *v));
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
        Node::Num(v) if is_float(ty) => (
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

/// The integer an enum is held in, its `repr` as written, or `u8`.
fn repr_of(n: &Node) -> Result<&'static Int, String> {
    let r = n.mark("repr").and_then(Node::str).unwrap_or("u8");
    int_of(r)
        .filter(|i| i.repr)
        .ok_or(format!("`repr` {r} is no integer an enum is held in"))
}

/// The members of `n`, the enum `name`, as every writer numbers them: a
/// list's (`list_members`), or a table's, its keys by their numbers.
pub fn enum_members(name: &str, n: &Node) -> Result<Vec<Member>, String> {
    let repr = repr_of(n)?;
    if n.is_wrapper() {
        return list_members(name, n.val().arr().unwrap_or_default(), repr, casing(n));
    }
    let entries: Vec<(&str, &Node)> = n.named().collect();
    numbers(name, &entries, repr, casing(n))
}

/// A list's members: its names, or its records' `name`s, numbered by their
/// places, or each by the `number` its record carries where every one
/// carries one, and documented by its record's `$description`.
fn list_members(name: &str, a: &[Node], repr: &Int, casing: Casing) -> Result<Vec<Member>, String> {
    let numbered = a.iter().filter(|e| e.mark("number").is_some()).count();
    if numbered != 0 && numbered != a.len() {
        return Err(format!(
            "{name}: a list's members are numbered by their places, or each by its `number`"
        ));
    }
    let keys: Vec<(&str, Node)> = a
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let number = e.mark("number").cloned().unwrap_or(Node::Num(k as f64));
            let mut m = vec![marks(vec![("number".to_string(), number)])];
            m.extend(
                e.get("$description")
                    .map(|d| ("$description".to_string(), d.clone())),
            );
            (
                e.str().or(e.str_at("name")).unwrap_or_default(),
                Node::Obj(m),
            )
        })
        .collect();
    let keys: Vec<(&str, &Node)> = keys.iter().map(|(k, v)| (*k, v)).collect();
    numbers(name, &keys, repr, casing)
}

/// An enum table's members: each is a number, or a table that carries its
/// `number` and may carry its `$description`; the numbers are distinct numbers
/// the enum's `repr` holds.
fn numbers(
    name: &str,
    keys: &[(&str, &Node)],
    repr: &Int,
    casing: Casing,
) -> Result<Vec<Member>, String> {
    let numbers: Vec<Option<f64>> = keys
        .iter()
        .map(|(_, v)| match v.mark("number") {
            Some(n) if v.is_obj() => n.num(),
            _ => v.val().num(),
        })
        .collect();
    let distinct = numbers
        .iter()
        .enumerate()
        .all(|(i, a)| numbers[..i].iter().all(|b| b != a));
    if keys.is_empty() || !distinct || !numbers.iter().all(|v| v.is_some_and(|v| repr.holds(v))) {
        return Err(format!(
            "{name}: an enum's members are distinct numbers its `repr` holds, or tables that carry one as `number`"
        ));
    }
    Ok(keys
        .iter()
        .zip(numbers)
        .map(|((k, v), n)| Member {
            ident: casing(k),
            name: k.to_string(),
            number: js_num(n.unwrap_or_default()),
            doc: v.is_obj().then(|| v.doc()).flatten().unwrap_or_default(),
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
    let doc = if n.is_obj() { n.doc() } else { None };
    let mut lines: Vec<String> = Vec::new();
    if n.is_wrapper() {
        rs_leaf(
            name,
            n.val(),
            type_of(n, ty),
            &Marks::of(n)?,
            indent,
            &mut lines,
            scope,
        )?;
    } else if !n.is_obj() {
        rs_leaf(name, n, ty, &Marks::of(n)?, indent, &mut lines, scope)?;
    } else {
        let enumed = type_of(n, ty) == "enum";
        let t = members_type(n, ty);
        let keys: Vec<(&str, &Node)> = n.named().collect();
        if enumed {
            let repr = repr_of(n)?;
            let variants = numbers(name, &keys, repr, casing(n))?;
            lines.extend(rs_enum(&pascal(name), &variants, repr.name, indent));
            lines.push(String::new());
            if let Some(map) = n.get("$map") {
                lines.extend(rs_map(&pascal(name), &variants, map, indent, scope)?);
                lines.push(String::new());
            }
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
    // A field's type is its own `$type` or the list's: a string's an enum
    // where it names one, as `rs_scalar` writes it.
    let base = |f: &Field| match f.kind {
        0 => enum_path(f.ty.unwrap_or(ty), scope)
            .map_or("&'static str".to_string(), |(path, _)| path),
        1 => "bool".to_string(),
        _ => f.ty.unwrap_or(ty).to_string(),
    };
    let tys: Vec<String> = fields
        .iter()
        .map(|(_, f)| {
            let t = if f.list {
                format!("&'static [{}]", base(f))
            } else {
                base(f)
            };
            if f.option { format!("Option<{t}>") } else { t }
        })
        .collect();
    let lit = |v: &Node, f: &Field| -> Result<String, String> {
        let t = f.ty.unwrap_or(ty);
        let value = match v.val() {
            Node::Null => return Ok("None".into()),
            Node::Arr(items) => format!(
                "&[{}]",
                items
                    .iter()
                    .map(|x| rs_scalar(x, t, scope).map(|(_, l)| l))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            v => rs_scalar(v, t, scope)?.1,
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
        let members = list_members(name, a, marks.repr, marks.casing)?;
        out.extend(rs_enum(&pascal(name), &members, marks.repr.name, indent));
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
            let members = list_members(name, a, marks.repr, marks.casing)?;
            out.extend(rs_enum(&pascal(name), &members, marks.repr.name, indent));
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

/// A field of an enum table's records whose `$type` names an enum the spec
/// declares maps each member to one of that enum's members: every member is a
/// record that names one, of that enum. So does a field that every record
/// gives as text, each member to its text (`$type` `str`). The field leaves
/// the records, and the table holds it under `$map`, the field with its enum,
/// or `str`, and what each member maps to in the table's order, which each
/// language writes as a function of the member (`rs_map`, `ts_map`,
/// `cpp_map`).
fn mapped(spec: Node) -> Result<Node, String> {
    let Node::Obj(entries) = spec else {
        return Ok(spec);
    };
    let enums: Vec<(String, Node)> = entries
        .iter()
        .filter(|(_, n)| n.str_at("$type") == Some("enum"))
        .map(|(k, n)| (pascal(k), n.clone()))
        .collect();
    entries
        .into_iter()
        .map(|(k, n)| {
            let n = map_table(&k, n, &enums)?;
            Ok((k, n))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(Node::Obj)
}

fn map_table(name: &str, n: Node, enums: &[(String, Node)]) -> Result<Node, String> {
    if n.str_at("$type") != Some("enum") || !n.is_obj() || n.is_wrapper() {
        return Ok(n);
    }
    // The fields any record types by an enum of the spec, each with that enum.
    let mut typed: Vec<(&str, &(String, Node))> = Vec::new();
    for (f, v) in n
        .named()
        .flat_map(|(_, v)| v.named().map(move |(f, _)| (f, v)))
    {
        let Some(e) = enums
            .iter()
            .find(|(e, _)| Some(e.as_str()) == field_type(v, f))
        else {
            continue;
        };
        if typed.iter().all(|(t, _)| *t != f) {
            typed.push((f, e));
        }
    }
    let mut map = Vec::new();
    for (f, (e, target)) in typed {
        let members = enum_members(e, target)?;
        let names = n
            .named()
            .map(|(k, v)| {
                let s = v.get(f).and_then(|x| x.val().str()).unwrap_or_default();
                if field_type(v, f) != Some(e.as_str()) || !members.iter().any(|m| m.name == s) {
                    return Err(format!("{name}.{k}.{f} names no member of {e}"));
                }
                Ok(Node::Str(s.to_string()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        map.push((
            f.to_string(),
            Node::Obj(vec![
                ("$type".into(), Node::Str(e.clone())),
                ("$value".into(), Node::Arr(names)),
            ]),
        ));
    }
    let text = |v: &Node, f: &str| {
        v.get(f)
            .filter(|x| x.is_wrapper() && x.str_at("$type").is_none())
            .and_then(|x| x.val().str())
            .map(str::to_string)
    };
    let first = n.named().next().map(|(_, v)| v);
    for (f, _) in first.into_iter().flat_map(Node::named) {
        let texts: Option<Vec<Node>> = n.named().map(|(_, v)| text(v, f).map(Node::Str)).collect();
        if let Some(texts) = texts {
            map.push((
                f.to_string(),
                Node::Obj(vec![
                    ("$type".into(), Node::Str("str".into())),
                    ("$value".into(), Node::Arr(texts)),
                ]),
            ));
        }
    }
    if map.is_empty() {
        return Ok(n);
    }
    let Node::Obj(entries) = n else {
        return Ok(n);
    };
    let mut out: Vec<(String, Node)> = entries
        .into_iter()
        .map(|(k, v)| match v {
            Node::Obj(fs) if !k.starts_with('$') => (
                k,
                Node::Obj(
                    fs.into_iter()
                        .filter(|(f, _)| !map.iter().any(|(m, _)| m == f))
                        .collect(),
                ),
            ),
            v => (k, v),
        })
        .collect();
    out.push(("$map".into(), Node::Obj(map)));
    Ok(Node::Obj(out))
}

/// The `$type` the record `v`'s field `f` gives itself, where it wraps its
/// value.
fn field_type<'a>(v: &'a Node, f: &str) -> Option<&'a str> {
    v.get(f)
        .filter(|x| x.is_wrapper())
        .and_then(|x| x.str_at("$type"))
}

/// The name TypeScript and C++ give the function of the enum `e` that a
/// `$map` field `f` is, as Rust's method `f` (`kind_Errno`).
fn map_ident(f: &str, e: &str) -> String {
    format!("{}_{}", snake(f), pascal(e))
}

/// The enum `e`'s `$map` in Rust: each field a `const fn` of the member.
fn rs_map(
    e: &str,
    members: &[Member],
    map: &Node,
    indent: &str,
    scope: &Scope,
) -> Result<Vec<String>, String> {
    let mut out = vec![format!("{indent}impl {e} {{")];
    for (i, (f, m)) in map.named().enumerate() {
        let ty = type_of(m, "");
        if i != 0 {
            out.push(String::new());
        }
        if ty == "str" {
            out.extend([
                format!("{indent}    /// The member's `{f}`."),
                format!(
                    "{indent}    pub const fn {}(self) -> &'static str {{",
                    rs_field(f)
                ),
                format!("{indent}        match self {{"),
            ]);
            for (mem, t) in members.iter().zip(m.val().arr().unwrap_or_default()) {
                out.push(format!(
                    "{indent}            {e}::{} => {},",
                    mem.ident,
                    json_str(t.str().unwrap_or_default())
                ));
            }
            out.extend([format!("{indent}        }}"), format!("{indent}    }}")]);
            continue;
        }
        let (to, casing) = enum_path(ty, scope).ok_or(format!("{e}.{f}: {ty} is no enum"))?;
        out.extend([
            format!("{indent}    /// The member's `{f}`."),
            format!("{indent}    pub const fn {}(self) -> {to} {{", rs_field(f)),
            format!("{indent}        match self {{"),
        ]);
        for (mem, name) in members.iter().zip(m.val().arr().unwrap_or_default()) {
            let name = name.str().unwrap_or_default();
            out.push(format!(
                "{indent}            {e}::{} => {to}::{},",
                mem.ident,
                casing(name)
            ));
        }
        out.extend([format!("{indent}        }}"), format!("{indent}    }}")]);
    }
    out.push(format!("{indent}}}"));
    Ok(out)
}

/// The enum `e`'s `$map` in TypeScript: each field a function of the member
/// (`map_ident`), from its constant to the one it names.
fn ts_map(
    e: &str,
    members: &[Member],
    map: &Node,
    enums: &[(String, Vec<Member>)],
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (f, m) in map.named() {
        let to = type_of(m, "");
        if to == "str" {
            out.extend([
                String::new(),
                format!("/** The member's `{f}`. */"),
                format!("export function {}(val: {e}): string {{", map_ident(f, e)),
                "  switch (val) {".into(),
            ]);
            for (mem, t) in members.iter().zip(m.val().arr().unwrap_or_default()) {
                out.push(format!("    case {}:", ts_const(e, mem)));
                out.push(format!(
                    "      return {};",
                    json_str(t.str().unwrap_or_default())
                ));
            }
            out.extend(["  }".to_string(), "}".into()]);
            continue;
        }
        let (_, targets) = enums
            .iter()
            .find(|(n, _)| n == to)
            .ok_or(format!("{e}.{f}: {to} is no enum"))?;
        out.extend([
            String::new(),
            format!("/** The member's `{f}`. */"),
            format!("export function {}(val: {e}): {to} {{", map_ident(f, e)),
            "  switch (val) {".into(),
        ]);
        for (mem, name) in members.iter().zip(m.val().arr().unwrap_or_default()) {
            let name = name.str().unwrap_or_default();
            let target = targets
                .iter()
                .find(|t| t.name == name)
                .ok_or(format!("{e}.{f}: {name} is no {to}"))?;
            out.push(format!("    case {}:", ts_const(e, mem)));
            out.push(format!("      return {};", ts_const(to, target)));
        }
        out.extend(["  }".to_string(), "}".into()]);
    }
    Ok(out)
}

/// The enum `e`'s `$map` in C++: each field a function of the member
/// (`map_ident`), its switch covering every member.
fn cpp_map(e: &str, members: &[Member], map: &Node, out: &mut Vec<String>, inc: &mut Includes) {
    for (f, m) in map.named() {
        let to = type_of(m, "");
        let text = to == "str";
        if text {
            inc.insert("string_view");
        }
        let ret = if text { "std::string_view" } else { to };
        out.extend([
            String::new(),
            format!("// The member's `{f}`."),
            format!("constexpr {ret} {}({e} val) noexcept {{", map_ident(f, e)),
            "    switch (val) {".into(),
        ]);
        for (mem, v) in members.iter().zip(m.val().arr().unwrap_or_default()) {
            let v = v.str().unwrap_or_default();
            let lit = if text {
                json_str(v)
            } else {
                format!("{to}::{}", cpp_member(v))
            };
            out.push(format!("    case {e}::{}: return {lit};", mem.ident));
        }
        out.extend([
            "    }".to_string(),
            "    __builtin_unreachable();".into(),
            "}".into(),
        ]);
    }
}

/// A table with `"packed": true` is laid out end to end. Each field gives its
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
/// Growing a field therefore moves everything after it. A field keeps its
/// `$description` beside its offset. Every width, count and end is a
/// `PACKED_TYPE`, or the field is refused (`word`).
fn laid_out(node: Node, used: &[(String, Node)]) -> Result<Node, String> {
    let Node::Obj(entries) = node else {
        return Ok(node);
    };
    let mut above: Vec<(String, Node)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let v = match v {
            Node::Obj(_) if is_packed(&v) => {
                Node::Obj(lay(v.entries().to_vec(), &above, used).map_err(|e| format!("{k}.{e}"))?)
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
    out.push(("$type".into(), Node::Str(PACKED_TYPE.into())));
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
        if v.is_wrapper() && v.get("$type").is_some() {
            return Err(format!(
                "{k}: a packed table's field is a `{PACKED_TYPE}`, and declares no `$type`"
            ));
        }
        match v {
            Node::Obj(_) if !v.is_wrapper() => {
                let mut section =
                    lay(v.entries().to_vec(), above, used).map_err(|e| format!("{k}.{e}"))?;
                let words = words_of(&section).unwrap_or(0.0);
                let first = section
                    .iter()
                    .take_while(|(f, _)| f.starts_with('$'))
                    .count();
                section.insert(first, ("AT".into(), Node::Num(at)));
                at = word(at + words).map_err(|e| format!("{k}: it ends at {e}"))?;
                out.push((k, Node::Obj(section)));
            }
            v => {
                let width = width(v.val(), above, used)
                    .and_then(word)
                    .map_err(|e| format!("{k}: {e}"))?;
                let end = word(at + width).map_err(|e| format!("{k}: it ends at {e}"))?;
                let off = match v.get("$description") {
                    Some(d) => Node::Obj(vec![
                        ("$description".into(), d.clone()),
                        ("$value".into(), Node::Num(at)),
                    ]),
                    None => Node::Num(at),
                };
                out.push((k, off));
                at = end;
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
        word(match c {
            Node::Num(n) => *n,
            Node::Str(s) => named(s)
                .and_then(|n| match n.val() {
                    Node::Arr(a) => Some(a.len() as f64),
                    n => n.num(),
                })
                .ok_or(format!("`{s}` names no count above it"))?,
            _ => return Err("a count is a number, or the name of one or of a list".into()),
        })
    };
    // An element's words, or None for a bit.
    let element = |e: &Node| -> Result<Option<f64>, String> {
        match e {
            Node::Num(n) => word(*n).map(Some),
            Node::Str(s) if s == "bit" => Ok(None),
            Node::Str(s) => named(s)
                .and_then(|n| match n {
                    Node::Obj(o) if !n.is_wrapper() => words_of(o),
                    n => n.val().num(),
                })
                .ok_or(format!("`{s}` names no packed table or number above it"))
                .and_then(word)
                .map(Some),
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

/// `x`, a number a packed table is laid out by: a width, a count or an end,
/// each a `PACKED_TYPE`, whole and within its width, or refused.
fn word(x: f64) -> Result<f64, String> {
    match int_of(PACKED_TYPE) {
        Some(i) if i.holds(x) => Ok(x),
        _ => Err(format!("{x} is no {PACKED_TYPE}")),
    }
}

/// The bits of a word a packed table counts in, which "bit" rounds up to.
const WORD_BITS: f64 = 32.0;

/// A table with `"keys": "<Enum>"` holds one value for each of the enum's
/// members, keyed by its name in the member's order: the enum this spec or a
/// spec it uses declares, its members numbered from 0 by place. It is written
/// as a list laid out by number, the member's number its index, in every
/// language alike (C++ an array of the value's type).
fn keyed(spec: Node, declared: &[(String, Node)]) -> Result<Node, String> {
    let Node::Obj(entries) = spec else {
        return Ok(spec);
    };
    let mut out = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let Some(e) = v.mark("keys").and_then(Node::str) else {
            out.push((k, v));
            continue;
        };
        let members = declared_members(declared, e).map_err(|err| format!("{k}: `keys` {err}"))?;
        let names: Vec<&str> = v.named().map(|(n, _)| n).collect();
        let by_place = members
            .iter()
            .enumerate()
            .all(|(i, m)| m.number == i.to_string());
        if !by_place || names != members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>() {
            return Err(format!(
                "{k}: its keys are {e}'s members, in their order, numbered by place"
            ));
        }
        let mut list: Vec<(String, Node)> = v
            .entries()
            .iter()
            .filter(|(f, _)| f.starts_with('$') && f != "$description")
            .map(|(f, x)| {
                (
                    f.clone(),
                    if f == "$extensions" {
                        without(x, "keys")
                    } else {
                        x.clone()
                    },
                )
            })
            .collect();
        let mut doc = v.doc().unwrap_or_default();
        doc.push(format!("By `{e}`'s number."));
        list.push(("$description".into(), Node::Str(doc.join("\n"))));
        list.push((
            "$value".into(),
            Node::Arr(v.named().map(|(_, x)| x.clone()).collect()),
        ));
        out.push((k, Node::Obj(list)));
    }
    Ok(Node::Obj(out))
}

/// The enums this spec or a spec it uses declares, each by its key.
fn declared_enums(declared: &[(String, Node)]) -> impl Iterator<Item = (&str, &Node)> {
    declared
        .iter()
        .filter(|(_, n)| n.str_at("$type") == Some("enum"))
        .map(|(k, n)| (k.as_str(), n))
}

/// `extensions`, a node's `$extensions`, without xpute's mark `name`; none
/// where nothing is left.
fn without(extensions: &Node, name: &str) -> Node {
    let entries = extensions
        .entries()
        .iter()
        .map(|(k, x)| match x {
            Node::Obj(m) if k == MARKS => (
                k.clone(),
                Node::Obj(m.iter().filter(|(f, _)| f != name).cloned().collect()),
            ),
            x => (k.clone(), x.clone()),
        })
        .filter(|(_, x)| *x != Node::Obj(Vec::new()))
        .collect();
    Node::Obj(entries)
}

/// The members of `e`, an enum this spec or a spec it uses declares.
fn declared_members(declared: &[(String, Node)], e: &str) -> Result<Vec<Member>, String> {
    let (key, list) = declared_enums(declared)
        .find(|(k, _)| pascal(k) == e)
        .ok_or(format!(
            "{e} is no enum this spec or a spec it uses declares"
        ))?;
    enum_members(key, list)
}

/// A table marked `"each": { "enum": "<Enum>", "number": "<field>" }` stands,
/// among its siblings, for one table of each of the enum's members, named as
/// the member is: each holds the table's own fields, then `<field>`, the
/// member's number, then the fields of the table under the member's name where
/// it has one. A table under it names a member, and a member's name a table.
/// The enum is this spec's or a spec it uses'. A name is distinct among its
/// siblings.
fn expanded(n: Node, declared: &[(String, Node)]) -> Result<Node, String> {
    let Node::Obj(entries) = n else {
        return Ok(n);
    };
    let mut out: Vec<(String, Node)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let Some(each) = v.mark("each") else {
            out.push((k, expanded(v, declared)?));
            continue;
        };
        let (Some(e), Some(numbered)) = (each.str_at("enum"), each.str_at("number")) else {
            return Err(format!(
                "{k}: `each` names an enum and the field its number takes"
            ));
        };
        let members = declared_members(declared, e).map_err(|err| format!("{k}: `each` {err}"))?;
        let names: Vec<String> = members.iter().map(|m| screaming(&m.name)).collect();
        let mut own: Vec<&(String, Node)> = Vec::new();
        let stripped = v
            .get("$extensions")
            .map(|x| ("$extensions".to_string(), without(x, "each")));
        for field @ (f, x) in v
            .entries()
            .iter()
            .filter(|(f, _)| f != "$extensions")
            .chain(&stripped)
        {
            let table = x.is_obj() && !x.is_wrapper();
            match (names.contains(f), table) {
                (true, true) => {}
                (true, false) => return Err(format!("{k}.{f}: not a table")),
                (false, true) if !f.starts_with('$') => {
                    return Err(format!("{k}.{f}: names no member of {e}"));
                }
                (false, _) => own.push(field),
            }
        }
        for m in &members {
            let name = screaming(&m.name);
            let number: f64 = m
                .number
                .parse()
                .map_err(|_| format!("{k}: {e}'s {} has no number", m.name))?;
            let mut fields: Vec<(String, Node)> = own.iter().map(|&f| f.clone()).collect();
            fields.push((numbered.to_string(), Node::Num(number)));
            for (f, x) in v.get(&name).map(Node::entries).unwrap_or_default() {
                if fields.iter().any(|(g, _)| g == f) {
                    return Err(format!("{k}.{name}: `{f}` is the table's own field too"));
                }
                fields.push((f.clone(), x.clone()));
            }
            out.push((name, expanded(Node::Obj(fields), declared)?));
        }
    }
    if let Some(i) = (1..out.len()).find(|&i| out[..i].iter().any(|(k, _)| *k == out[i].0)) {
        return Err(format!("`{}` is named twice among its siblings", out[i].0));
    }
    Ok(Node::Obj(out))
}

/// Refuses, under a table with `"distinct": "<field>"`, two sibling tables at
/// any depth that hold the same `<field>`.
fn check_distinct(n: &Node, at: &str, field: Option<&str>) -> Result<(), String> {
    let field = n.mark("distinct").and_then(Node::str).or(field);
    if let Some(f) = field {
        let held: Vec<(&str, &Node)> = n
            .named()
            .filter_map(|(k, v)| Some((k, v.get(f)?.val())))
            .collect();
        if let Some(i) = (1..held.len()).find(|&i| held[..i].iter().any(|(_, x)| *x == held[i].1)) {
            return Err(format!("{at}.{}: its {f} is a sibling's too", held[i].0));
        }
    }
    n.named()
        .filter(|(_, v)| v.is_obj() && !v.is_wrapper())
        .try_for_each(|(k, v)| check_distinct(v, &format!("{at}.{k}"), field))
}

/// An enum another spec declares, which a spec names by `uses` and a `$type`:
/// its name, how its members are named, and the outputs Rust and TypeScript
/// read it from.
struct Foreign {
    name: String,
    casing: Casing,
    rs: Option<&'static str>,
    ts: Option<&'static str>,
}

/// The enums the specs a spec uses declare (`used`, from `uses_of`), each
/// reached through the peer that writes its spec; a spec it uses that no peer
/// writes is refused.
fn foreign(used: &[Used], peers: &[DataSpec]) -> Result<Vec<Foreign>, String> {
    let mut out = Vec::new();
    for (u, names) in used {
        let p = peers
            .iter()
            .find(|p| p.json == u)
            .ok_or(format!("`uses` {u}: no spec beside this one writes it"))?;
        for (k, n) in declared_enums(names) {
            out.push(Foreign {
                name: pascal(k),
                casing: casing(n),
                rs: p.rs,
                ts: p.ts,
            });
        }
    }
    Ok(out)
}

/// Every `$type` a value under `n` names.
fn types(n: &Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(n: &Node, out: &mut BTreeSet<String>) {
        out.extend(n.str_at("$type").map(String::from));
        match n {
            Node::Obj(entries) => entries
                .iter()
                .filter(|(k, _)| !k.starts_with('$') || k == "$value")
                .for_each(|(_, v)| walk(v, out)),
            Node::Arr(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    walk(n, &mut out);
    out
}

fn read_at(root: &Path, json: &str) -> Result<Node, String> {
    read(&root.join(json).to_string_lossy())
}

/// This answers the names `spec` may read beyond its own, taken from the
/// specs its `uses` names, by their paths from `root`, each with the names it
/// gives. Each used spec is laid out over its own uses, each name must be owned
/// by one file, and no chain of uses may be circular; `json` is where `spec`
/// comes from.
fn uses_of(
    root: &Path,
    json: &str,
    spec: &Node,
    path: &mut Vec<String>,
) -> Result<Vec<Used>, String> {
    if path.iter().any(|p| p == json) {
        return Err(format!("{json}: uses itself through {}", path.join(" → ")));
    }
    path.push(json.to_string());
    let uses: Vec<String> = spec
        .mark("uses")
        .and_then(Node::arr)
        .map(|a| a.iter().filter_map(|u| u.str().map(String::from)).collect())
        .unwrap_or_default();
    let mut out: Vec<Used> = Vec::new();
    for u in &uses {
        let spec = read_at(root, u)?;
        let below = flat(&uses_of(root, u, &spec, path)?);
        let Node::Obj(entries) = laid_out(spec, &below)? else {
            return Err(format!("{u}: not a table"));
        };
        let mut names: Vec<(String, Node)> = Vec::new();
        for (k, v) in entries.into_iter().filter(|(k, _)| !k.starts_with('$')) {
            if out.iter().flat_map(|(_, n)| n).any(|(n, _)| *n == k) {
                return Err(format!(
                    "{json}: `{k}` is named in two of the specs it uses"
                ));
            }
            names.push((k, v));
        }
        out.push((u.clone(), names));
    }
    path.pop();
    Ok(out)
}

/// A spec used, by its path, with the names it gives.
type Used = (String, Vec<(String, Node)>);

/// The names the used specs give, as one list.
fn flat(used: &[Used]) -> Vec<(String, Node)> {
    used.iter()
        .flat_map(|(_, names)| names.iter().cloned())
        .collect()
}

/// Every file that `specs` write, each read from its JSON, with paths relative
/// to `root`.
pub fn outputs_at(root: &Path, specs: &[DataSpec]) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    for d in specs {
        outputs.extend(table_outputs(
            root,
            d,
            parse(&root.join(d.json).to_string_lossy())?,
            specs,
        )?);
    }
    Ok(outputs)
}

/// The files `d` writes from `spec`, a table a tool built in memory or read
/// from `d.json`, which the headers name as its source, typed once its tables
/// are laid out, keyed and expanded (`typed`). `peers` are the specs beside it, by which an enum a spec it uses
/// declares is reached in each language (`Foreign`).
pub fn table_outputs(
    root: &Path,
    d: &DataSpec,
    spec: Node,
    peers: &[DataSpec],
) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    let used = uses_of(root, d.json, &spec, &mut Vec::new())?;
    let uses = flat(&used);
    let foreign = foreign(&used, peers).map_err(|e| format!("{}: {e}", d.json))?;
    let spec = mapped(spec).map_err(|e| format!("{}: {e}", d.json))?;
    let spec = laid_out(spec, &uses)?;
    let declared: Vec<(String, Node)> = spec.entries().iter().chain(&uses).cloned().collect();
    let spec = keyed(spec, &declared).map_err(|e| format!("{}: {e}", d.json))?;
    let spec = expanded(spec, &declared).map_err(|e| format!("{}: {e}", d.json))?;
    let spec = typed(spec).map_err(|e| format!("{}: {e}", d.json))?;
    check_types(&spec, &declared).map_err(|e| format!("{}: {e}", d.json))?;
    spec.named()
        .try_for_each(|(k, v)| check_distinct(v, k, None))
        .map_err(|e| format!("{}: {e}", d.json))?;
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
        ts_outputs(root, d, &spec, &declared, &foreign, ts_path, &mut outputs)?;
    }
    if let Some(rs_path) = d.rs {
        rs_outputs(root, d, &spec, &foreign, rs_path, &mut outputs)?;
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
    let mut out = vec![
        format!("// {shown}"),
        "//".into(),
        format!("// {}", mark(json, "xpute-gen data.rs")),
    ];
    if let Some(doc) = spec.doc() {
        out.push("//".into());
        out.extend(line_comments(&doc, ""));
    }
    out
}

fn ts_outputs(
    root: &Path,
    d: &DataSpec,
    spec: &Node,
    declared: &[(String, Node)],
    foreign: &[Foreign],
    ts_path: &str,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let mut ts = head(&package_path(&|p| package(root, p), ts_path)?, d.json, spec);
    // A foreign enum a `$type` names is its spec's own output's type.
    let named = types(spec);
    let mut imports = Vec::new();
    for f in foreign.iter().filter(|f| named.contains(&f.name)) {
        let from =
            f.ts.ok_or(format!("{}: a spec it uses writes no TypeScript", d.json))?;
        let here = root.join(ts_path);
        let module = ts_module(
            here.parent().unwrap_or(root),
            Some(root),
            &|p| package(root, p),
            from,
        )?;
        imports.push(format!("import type {{ {} }} from \"{module}\";", f.name));
    }
    if !imports.is_empty() {
        ts.push(String::new());
        ts.extend(imports);
    }
    let enums = declared_enums(declared)
        .map(|(k, n)| Ok((pascal(k), enum_members(k, n)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let mut body: Vec<String> = Vec::new();
    for (name, n) in spec.named() {
        body.push(String::new());
        if let Some(doc) = n.is_obj().then(|| n.doc()).flatten() {
            body.extend(
                ts_doc(&doc, "")
                    .into_iter()
                    .enumerate()
                    .map(|(i, l)| if i == 0 { l } else { align_star(l, "") }),
            );
        }
        // A value is written as a constant, a table or a list as one too,
        // `as const`. An enum table whose members carry nothing past their
        // numbers is its members' constants alone.
        let enumed = type_of(n, "f64") == "enum";
        let names = enumed && is_names(n);
        let bare = enumed
            && (names
                || !n.is_wrapper()
                    && n.named()
                        .all(|(_, v)| !v.is_obj() || v.named().next().is_none()));
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
        if let Some(map) = n.get("$map") {
            body.extend(ts_map(&pascal(name), &members, map, &enums)?);
        }
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
                    let v = if k == "$value" {
                        rows.clone()
                    } else {
                        v.clone()
                    };
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
    foreign: &[Foreign],
    rs_path: &str,
    outputs: &mut Vec<Output>,
) -> Result<(), String> {
    let shown = package_path(&|p| package(root, p), rs_path)?;
    let mut rs = head(&shown, d.json, spec);
    // The spec's own enums, which the module names by their name alone.
    let enums = declared_enums(spec.entries())
        .map(|(k, n)| (pascal(k), casing(n)))
        .collect();
    // A foreign enum a `$type` names is its spec's own output's, `use`d by
    // its module.
    let named = types(spec);
    let here = root.join(package(root, rs_path)?.1);
    let foreign = foreign
        .iter()
        .filter(|f| named.contains(&f.name))
        .map(|f| {
            let rs =
                f.rs.ok_or(format!("{}: a spec it uses writes no Rust", d.json))?;
            Ok((
                f.name.clone(),
                f.casing,
                format!(
                    "{}::{}",
                    rs_module(&here, Some(root), &|p| package(root, p), rs)?,
                    f.name
                ),
            ))
        })
        .collect::<Result<_, String>>()?;
    let scope = Scope {
        uses: RefCell::default(),
        enums,
        foreign,
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
/// is written as a `string_view` array, and one whose `$type` is `enum` as an
/// enum, with that array under its plural where it marks `"names": true`; a
/// list of numbers is a `std::array` of its `$type`. Any other value is a leaf
/// (`cpp_leaf`).
fn cpp_items(
    name: &str,
    n: &Node,
    ty: &str,
    out: &mut Vec<String>,
    inc: &mut Includes,
) -> Result<(), String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(numbers) = n
        .val()
        .arr()
        .filter(|a| !a.is_empty())
        .and_then(|a| a.iter().map(Node::num).collect::<Option<Vec<_>>>())
    {
        let (ty, suffix) = cpp_number(name, type_of(n, ty), inc)?;
        inc.insert("array");
        let items: Vec<String> = numbers
            .iter()
            .map(|v| format!("{}{suffix}", js_num(*v)))
            .collect();
        lines.push(format!(
            "inline constexpr std::array<{ty}, {}> {name}{{{}}};",
            items.len(),
            items.join(", ")
        ));
    } else if let Some(fields) = n.val().arr().map(|a| fields(a)).transpose()?.flatten() {
        // A list of records: a struct of its fields, each a scalar, and an
        // array of its rows.
        let list = n.val().arr().unwrap_or_default();
        let marks = Marks::of(n)?;
        let r = marks.row.map_or(format!("{}Record", pascal(name)), pascal);
        let t = type_of(n, ty);
        let mut tys = Vec::with_capacity(fields.len());
        for (k, f) in &fields {
            if f.list || f.option {
                return Err(format!("{name}.{k}: a C++ record holds one value a field"));
            }
            tys.push(match f.kind {
                0 => {
                    inc.insert("string_view");
                    "std::string_view"
                }
                1 => "bool",
                _ => cpp_number(name, f.ty.unwrap_or(t), inc)?.0,
            });
        }
        let mut rows = Vec::with_capacity(list.len());
        for e in list {
            let vals = e
                .named()
                .zip(&fields)
                .map(|((_, v), (_, f))| {
                    Ok(match v.val() {
                        Node::Str(s) => json_str(s),
                        Node::Bool(b) => b.to_string(),
                        Node::Num(x) => format!(
                            "{}{}",
                            js_num(*x),
                            cpp_number(name, f.ty.unwrap_or(t), inc)?.1
                        ),
                        _ => return Err(format!("{name}: a C++ record holds scalars")),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            rows.push(format!("{{{}}}", vals.join(", ")));
        }
        inc.insert("array");
        lines.push(format!("struct {r} {{"));
        lines.extend(
            fields
                .iter()
                .zip(&tys)
                .map(|((k, _), t)| format!("    {t} {};", rs_field(k))),
        );
        lines.extend(["};".to_string(), String::new()]);
        lines.push(format!(
            "inline constexpr std::array<{r}, {}> {name}{{{{{}}}}};",
            list.len(),
            rows.join(", ")
        ));
    } else if let Node::Arr(list) = n.val() {
        let names = list
            .iter()
            .map(|v| {
                v.str()
                    .ok_or(format!("{name}: a C++ list takes names only"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let enumed = type_of(n, ty) == "enum";
        if enumed {
            let repr = repr_of(n)?;
            cpp_enum(
                name,
                &list_members(name, list, repr, cpp_member)?,
                repr.cpp,
                &mut lines,
                inc,
            );
        }
        if !enumed || names_read(n) {
            let key = if enumed {
                plural(name)
            } else {
                name.to_string()
            };
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
            let repr = repr_of(n)?;
            let keys: Vec<(&str, &Node)> = n.named().collect();
            let members = numbers(name, &keys, repr, cpp_member)?;
            cpp_enum(name, &members, repr.cpp, &mut lines, inc);
            if let Some(map) = n.get("$map") {
                cpp_map(&pascal(name), &members, map, &mut lines, inc);
            }
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
    if let Some(doc) = n.is_obj().then(|| n.doc()).flatten() {
        out.extend(line_comments(&doc, ""));
    }
    out.extend(lines);
    Ok(())
}

/// A string is a `string_view`, and a number a constant of its integer
/// `$type` (`INTS`). Any other value is refused.
fn cpp_leaf(name: &str, v: &Node, t: &str, inc: &mut Includes) -> Result<String, String> {
    if let Node::Str(s) = v {
        if !is_primitive(t) {
            return Err(format!("{name}: a C++ header takes no string typed {t}"));
        }
        inc.insert("string_view");
        return Ok(format!(
            "inline constexpr std::string_view {name} = {};",
            json_str(s)
        ));
    }
    let number = v.num().ok_or(format!(
        "{name}: a C++ header takes numbers and strings only"
    ))?;
    let (ty, suffix) = cpp_number(name, t, inc)?;
    Ok(format!(
        "inline constexpr {ty} {name} = {}{suffix};",
        js_num(number)
    ))
}

/// A number's C++ type for its integer `$type` (`INTS`), and its literal's
/// suffix, its header included.
fn cpp_number(
    name: &str,
    t: &str,
    inc: &mut Includes,
) -> Result<(&'static str, &'static str), String> {
    let i = int_of(t).ok_or(format!("{name}: a C++ header takes an integer, not {t}"))?;
    inc.insert(i.header);
    Ok((i.cpp, i.suffix))
}
