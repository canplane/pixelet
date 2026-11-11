// xpute-gen/spec/cmd/rs.rs

//! This writes a command table as Rust for a Rust core: its enums, a reader for
//! each packet and a writer for each import.

use super::{literal, members, Apart, Entry, Enum, Field, Kind, Member, RuntimeEnd, Spec, Target};
use crate::spec::{file, generated, pascal, rs_doc};

pub(super) fn rs_borrows(spec: &Spec, fields: &[Field]) -> Result<bool, String> {
    for f in fields {
        let kind = spec.kind(&f.ty)?;
        if f.list
            || kind == Kind::Str
            || kind == Kind::Path
            || (kind == Kind::Record && rs_borrows(spec, &spec.record(&f.ty).fields)?)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rs_rec(spec: &Spec, name: &str, lifetime: &str) -> Result<String, String> {
    Ok(if rs_borrows(spec, &spec.record(name).fields)? {
        format!("{name}<{lifetime}>")
    } else {
        name.to_string()
    })
}

/// The externs the protocol's fields take, each named where it lives and
/// re-exported under its name, which the generated code spells it by.
fn extern_uses(spec: &Spec) -> Result<Vec<String>, String> {
    let cmds = spec.cmds()?;
    let fields: Vec<&Field> = cmds
        .iter()
        .flat_map(|c| {
            c.pkt
                .iter()
                .chain(c.import.iter())
                .chain(c.reply.iter())
                .flatten()
        })
        .chain(spec.records.iter().flat_map(|r| r.fields.iter()))
        .collect();
    Ok(spec
        .externs
        .iter()
        .filter(|x| fields.iter().any(|f| f.ty == x.name))
        .map(|x| match x.rs.rsplit("::").next() {
            Some(last) if last == x.name => format!("pub use {};", x.rs),
            _ => format!("pub use {} as {};", x.rs, x.name),
        })
        .collect())
}

fn rs_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let one = rs_read_one_type(spec, f)?;
    Ok(if f.optional {
        format!("Option<{one}>")
    } else {
        one
    })
}

/// The type one field's value is read as, an optional one's when present.
fn rs_read_one_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if kind == Kind::Path {
        return Ok("Path<'a>".to_string());
    }
    if f.list && kind == Kind::Number {
        return Ok(format!("&'a [{}]", f.ty));
    }
    if f.list && kind == Kind::Str {
        return Ok("StrList<'a>".to_string());
    }
    if f.list || kind == Kind::Record {
        return Err(format!(
            "protocol: Rust reads no {} into a struct ({})",
            if f.list { "list" } else { "record" },
            f.name
        ));
    }
    Ok(match kind {
        Kind::Number => f.ty.clone(),
        Kind::Str => "&'a str".to_string(),
        Kind::Bool => "bool".to_string(),
        _ => f.ty.clone(),
    })
}

fn rs_read_one(spec: &Spec, f: &Field, c: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Str {
        return Ok(format!("{c}.get_strs()?"));
    }
    if f.list {
        return Ok(format!("{c}.get_array::<{}>()?", f.ty));
    }
    match kind {
        Kind::Number => {
            return Ok(match f.ty.as_str() {
                "f64" => format!("finite({c}.get_number()?)?"),
                ty => format!("integer::<{ty}>({c}.get_number()?)?"),
            })
        }
        Kind::Bool => return Ok(format!("{c}.get_bool()?")),
        Kind::Str => return Ok(format!("{c}.get_str_ref()?")),
        Kind::Path => return Ok(format!("Path::read(pkt, {c})?")),
        _ => {}
    }
    let (repr, unknown) = match spec.enums.iter().find(|e| e.name == f.ty) {
        Some(e) => (None, e.unknown.clone()),
        None => {
            let x = spec
                .extern_of(&f.ty)
                .ok_or(format!("protocol: no type {}", f.ty))?;
            (x.repr.clone(), None)
        }
    };
    let path = &f.ty;
    let member = if repr.as_deref() == Some("u8") {
        format!("{path}::of(integer::<u8>({c}.get_number()?)?)")
    } else {
        format!("{path}::of(integer::<u32>({c}.get_number()?)?)")
    };
    Ok(match unknown {
        Some(u) => format!("{member}.unwrap_or({path}::{u})"),
        None => format!("{member}.ok_or_else(|| unknown())?"),
    })
}

fn rs_read(spec: &Spec, f: &Field, i: usize) -> Result<String, String> {
    Ok(if f.optional {
        format!(
            "match f.at({i})?.opt() {{ Some(c) => Some({}), None => None }}",
            rs_read_one(spec, f, "c")?
        )
    } else {
        rs_read_one(spec, f, &format!("f.at({i})?"))?
    })
}

fn rs_write_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        if kind == Kind::Number {
            return Ok(format!("&'a [{}]", f.ty));
        }
        if kind == Kind::Str {
            return Ok("&'a [&'a str]".to_string());
        }
        if kind != Kind::Record {
            return Err(format!(
                "protocol: a list of {} ({}) is not written",
                f.ty, f.name
            ));
        }
        return Ok(format!(
            "&'a dyn Fn(&mut dyn FnMut(&{}))",
            rs_rec(spec, &f.ty, "'_")?
        ));
    }
    if kind == Kind::Path {
        return Ok("&'a [Token<'a>]".to_string());
    }
    // An optional string is borrowed as a `&str`; a required one may be any
    // `Display`.
    if f.optional && kind == Kind::Str {
        return Ok("Option<&'a str>".to_string());
    }
    let one = match kind {
        Kind::Number => f.ty.clone(),
        Kind::Bool => "bool".to_string(),
        Kind::Str => "&'a dyn Display".to_string(),
        Kind::Record => rs_rec(spec, &f.ty, "'a")?,
        _ => f.ty.clone(),
    };
    Ok(if f.optional {
        format!("Option<{one}>")
    } else {
        one
    })
}

/// A list is walked twice, first to count its items, because an XTP branch's
/// table comes before its children.
fn rs_write(spec: &Spec, f: &Field) -> Result<Vec<String>, String> {
    let kind = spec.kind(&f.ty)?;
    let v = format!("self.{}", f.name);
    let fields = format!("{}::FIELDS", f.ty);
    if f.list {
        if kind == Kind::Number {
            return Ok(vec![format!("w.{}_array(Some({v}));", f.ty)]);
        }
        if kind == Kind::Str {
            return Ok(vec![format!("w.strs(Some({v}));")]);
        }
        return Ok(vec![
            "let mut n = 0;".to_string(),
            format!("({v})(&mut |_| n += 1);"),
            "w.branch(n, |w| {".to_string(),
            format!("    ({v})(&mut |item| {{"),
            format!("        w.branch({fields}, |w| item.write(w));"),
            "    });".to_string(),
            "});".to_string(),
        ]);
    }
    Ok(vec![match kind {
        Kind::Record if f.optional => format!("match &{v} {{ Some(r) => w.branch({fields}, |w| r.write(w)), None => w.no_branch() }};"),
        Kind::Record => format!("w.branch({fields}, |w| {v}.write(w));"),
        Kind::Str if f.optional => format!("w.str({v});"),
        Kind::Str => format!("w.str_display({v});"),
        Kind::Path => format!("Path::write(w, {v});"),
        Kind::Enum | Kind::Extern if f.optional => format!("w.u32({v}.map(|m| m as u32));"),
        Kind::Enum | Kind::Extern => format!("w.u32(Some({v} as u32));"),
        _ => {
            let method = if kind == Kind::Bool { "bool" } else { f.ty.as_str() };
            if f.optional {
                format!("w.{method}({v});")
            } else {
                format!("w.{method}(Some({v}));")
            }
        }
    }])
}

fn rs_writer(
    spec: &Spec,
    out: &mut Vec<String>,
    name: &str,
    doc: Option<&[String]>,
    fields: &[Field],
) -> Result<(), String> {
    let borrows = rs_borrows(spec, fields)?;
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(rs_doc(doc, "", "///"));
    }
    out.push(format!(
        "pub struct {name}{} {{",
        if borrows { "<'a>" } else { "" }
    ));
    for f in fields {
        if let Some(doc) = &f.doc {
            out.extend(rs_doc(doc, "    ", "///"));
        }
        out.push(format!("    pub {}: {},", f.name, rs_write_type(spec, f)?));
    }
    out.extend([
        "}".to_string(),
        String::new(),
        format!("impl {name}{} {{", if borrows { "<'_>" } else { "" }),
        "    /// The children it is written as.".to_string(),
        format!("    pub const FIELDS: u32 = {};", fields.len()),
        String::new(),
        format!(
            "    pub fn write(&self, {}: &mut PacketWriter) {{",
            if fields.is_empty() { "_" } else { "w" }
        ),
    ]);
    for f in fields {
        out.extend(
            rs_write(spec, f)?
                .into_iter()
                .map(|l| format!("        {l}")),
        );
    }
    out.extend(["    }".to_string(), "}".to_string()]);
    Ok(())
}

/// Writes `fields` as the struct `name` that a packet is read into in place,
/// Command::`cmd`'s `what`.
fn read_struct(
    spec: &Spec,
    out: &mut Vec<String>,
    name: &str,
    cmd: &str,
    what: &str,
    fields: &[Field],
) -> Result<(), String> {
    let mut borrows = false;
    for f in fields {
        let kind = spec.kind(&f.ty)?;
        borrows |= f.list || kind == Kind::Str || kind == Kind::Path;
    }
    let lt = if borrows { "<'a>" } else { "" };
    out.extend([
        String::new(),
        format!("/// Command::{cmd}'s {what}."),
        "#[derive(Clone, Copy, Debug)]".to_string(),
        format!("pub struct {name}{lt} {{"),
    ]);
    for f in fields {
        if let Some(doc) = &f.doc {
            out.extend(rs_doc(doc, "    ", "///"));
        }
        out.push(format!("    pub {}: {},", f.name, rs_read_type(spec, f)?));
    }
    out.extend([
        "}".to_string(),
        String::new(),
        format!("impl{lt} {name}{lt} {{"),
    ]);
    out.push(format!(
        "    pub fn read(pkt: &{}[u8]) -> Result<{name}{lt}, MarshalError> {{",
        if borrows { "'a " } else { "" }
    ));
    out.push("        let f = TreeReader::new(pkt)?.read_branch()?;".into());
    out.push(format!("        Ok({name} {{"));
    for (i, f) in fields.iter().enumerate() {
        out.push(format!("            {}: {},", f.name, rs_read(spec, f, i)?));
    }
    out.extend([
        "        })".to_string(),
        "    }".to_string(),
        "}".to_string(),
    ]);
    Ok(())
}

/// Writes the record `r` as a view over its branch, which reads each field
/// when it is asked for: a record as its own view, and a list of records as
/// the views of its items.
fn rs_view(spec: &Spec, out: &mut Vec<String>, r: &super::Record) -> Result<(), String> {
    let name = &r.name;
    out.push(String::new());
    if let Some(doc) = &r.doc {
        out.extend(rs_doc(doc, "", "///"));
    }
    out.extend([
        "#[derive(Clone, Debug)]".to_string(),
        format!("pub struct {name}<'a>(BranchCursor<'a>);"),
        String::new(),
        format!("impl<'a> {name}<'a> {{"),
        format!("    pub fn read(pkt: &'a [u8]) -> Result<{name}<'a>, MarshalError> {{"),
        format!("        Ok({name}(TreeReader::new(pkt)?.read_branch()?))"),
        "    }".to_string(),
        String::new(),
        format!("    pub fn of(c: Cursor<'a>) -> Result<{name}<'a>, MarshalError> {{"),
        format!("        Ok({name}(c.as_branch()?))"),
        "    }".to_string(),
    ]);
    for (i, f) in r.fields.iter().enumerate() {
        let kind = spec.kind(&f.ty)?;
        let (one_ty, one) = if kind == Kind::Record && f.list {
            (
                format!(
                    "impl Iterator<Item = Result<{}<'a>, MarshalError>> + 'a",
                    f.ty
                ),
                format!(
                    "{{ let b = c.as_branch()?; (0..b.len).map(move |k| {}::of(b.at(k)?)) }}",
                    f.ty
                ),
            )
        } else if kind == Kind::Record {
            (format!("{}<'a>", f.ty), format!("{}::of(c)?", f.ty))
        } else {
            (rs_read_one_type(spec, f)?, rs_read_one(spec, f, "c")?)
        };
        let (ty, val) = if f.optional {
            (
                format!("Option<{one_ty}>"),
                format!("match self.0.at({i})?.opt() {{ Some(c) => Some({one}), None => None }}"),
            )
        } else {
            (one_ty, format!("{{ let c = self.0.at({i})?; {one} }}"))
        };
        out.push(String::new());
        if let Some(doc) = &f.doc {
            out.extend(rs_doc(doc, "    ", "///"));
        }
        out.push(format!(
            "    pub fn {}(&self) -> Result<{ty}, MarshalError> {{",
            f.name
        ));
        if kind == Kind::Path {
            out.push("        let pkt = self.0.cursor.pkt;".into());
        }
        out.extend([format!("        Ok({val})"), "    }".to_string()]);
    }
    out.push("}".to_string());
    Ok(())
}

/// Writes `e` as a Rust enum whose members carry their numbers, with an `of`
/// that reads a number into a member.
fn enum_decl(e: &Enum, out: &mut Vec<String>) {
    let all: Vec<&Member> = members(e).collect();
    out.push(String::new());
    if let Some(doc) = &e.doc {
        out.extend(rs_doc(doc, "", "///"));
    }
    out.push(format!(
        "#[derive(Clone, Copy, Debug, {}PartialEq, Eq)]",
        if e.default.is_some() { "Default, " } else { "" }
    ));
    out.push("#[repr(u32)]".into());
    out.push(format!("pub enum {} {{", e.name));
    for entry in &e.entries {
        match entry {
            Entry::Section(s) => {
                out.extend([String::new(), format!("    // ---- {} ----", s.section)]);
            }
            Entry::Member(m) => {
                if let Some(doc) = &m.doc {
                    out.extend(rs_doc(doc, "    ", "///"));
                }
                if e.default.as_deref() == Some(m.name.as_str()) {
                    out.push("    #[default]".into());
                }
                out.push(format!("    {} = {},", m.name, literal(m.val, &e.format)));
            }
        }
    }
    out.extend(["}".to_string(), String::new()]);
    out.extend([
        format!("impl {} {{", e.name),
        format!(
            "    pub const ALL: [{}; {}] = [{}];",
            e.name,
            all.len(),
            all.iter()
                .map(|m| format!("{}::{}", e.name, m.name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
        "    /// The member a number names, or none for a number nobody gave out.".to_string(),
        format!("    pub fn of(number: u32) -> Option<{}> {{", e.name),
        format!(
            "        {}::ALL.into_iter().find(|m| *m as u32 == number)",
            e.name
        ),
        "    }".to_string(),
        "}".to_string(),
    ]);
}

/// Writes the protocol's messages as Rust. The enums that `apart` names are
/// written into a file of their own (`emit_apart`), and this file names them
/// from there.
pub(super) fn emit(spec: &Spec, t: &Target, apart: Option<&Apart>) -> Result<String, String> {
    let mut out = vec![
        format!("// {}", t.rs_logical),
        "//".into(),
        generated(t.json),
        "".into(),
    ];
    out.extend(rs_doc(&spec.doc, "", "//!"));
    out.extend(
        [
            "",
            "// A list a record carries is a function handing out its items, whose",
            "// type is long to spell and is spelled once, here, for each. A table",
            "// with no import or no enum read off the wire leaves a writer or a",
            "// number's check unused.",
            "#![allow(non_camel_case_types, clippy::upper_case_acronyms, clippy::type_complexity, clippy::absolute_paths, unused_imports, dead_code, reason = \"generated from the spec\")]",
            "",
            "use core::fmt::Display;",
            "",
            "use xpute_kit::status::error::MarshalError;",
        ]
        .map(String::from),
    );
    // A list of strings is read as xpute's `StrList`, which is imported only where
    // a packet has such a list.
    let strs = spec
        .cmds()?
        .iter()
        .flat_map(|c| {
            c.pkt
                .iter()
                .chain(c.import.iter())
                .chain(c.reply.iter())
                .flatten()
        })
        .any(|f| f.list && f.ty == "str");
    out.push(format!(
        "use xpute_kit::wire::xtp::{{PacketWriter, {}TreeReader}};",
        if strs { "StrList, " } else { "" }
    ));
    out.push(format!("use {}::{{finite, integer, unknown}};", t.number.0));
    if spec.has_path()? {
        out.push(format!("use {}::{{Path, Token}};", t.path.0));
    }
    let names: &[String] = apart.map_or(&[], |a| &a.names);
    if let Some(a) = apart {
        let mut sorted = names.to_vec();
        sorted.sort();
        out.push(format!("pub use {}::{{{}}};", a.path, sorted.join(", ")));
    }
    out.extend(extern_uses(spec)?);
    for e in spec.enums.iter().filter(|e| !names.contains(&e.name)) {
        enum_decl(e, &mut out);
    }
    for r in &spec.records {
        rs_writer(spec, &mut out, &r.name, r.doc.as_deref(), &r.fields)?;
    }
    for c in spec.cmds()? {
        let name = pascal(&c.name);
        if let Some(pkt) = &c.pkt {
            read_struct(spec, &mut out, &name, &c.name, "packet", pkt)?;
        }
        if let Some(reply) = c.reply.as_ref().filter(|r| !r.is_empty()) {
            let reply_name = format!("{name}Reply");
            read_struct(
                spec,
                &mut out,
                &reply_name,
                &c.name,
                "completion's packet",
                reply,
            )?;
        }
        if let Some(import) = &c.import {
            let doc = [format!("Command::{}'s packet.", c.name)];
            rs_writer(spec, &mut out, &name, Some(&doc), import)?;
            if let Some(door) = t.import {
                let lt = if rs_borrows(spec, import)? {
                    "<'_>"
                } else {
                    ""
                };
                out.extend([
                    String::new(),
                    format!("impl {door}::Import for {name}{lt} {{"),
                    format!("    const NUMBER: u32 = {};", c.val),
                    format!("    const FIELDS: u32 = {};", import.len()),
                    format!("    const ACKREQ: bool = {};", c.ackreq),
                    "    fn write(&self, w: &mut PacketWriter) {".to_string(),
                    format!("        {name}::write(self, w);"),
                    "    }".to_string(),
                    "    fn named(&self) -> (u32, u32) {".to_string(),
                    match &c.mem {
                        Some([at, len]) => format!("        (self.{at}, self.{len})"),
                        None => "        (0, 0)".to_string(),
                    },
                    "    }".to_string(),
                    "}".to_string(),
                ]);
            }
        }
    }
    Ok(file(&out))
}

/// Writes the protocol's runtime end as Rust, `r` saying where: the protocol's
/// enums, named from its core end, and the externs its fields take; a view of each record; a writer for each
/// export and each import's completion; and a struct each import's packet is
/// read into.
pub(super) fn emit_runtime(spec: &Spec, t: &Target, r: &RuntimeEnd) -> Result<String, String> {
    let mut out = vec![
        format!("// {}", r.rs_logical),
        "//".into(),
        generated(t.json),
        "".into(),
    ];
    out.extend(rs_doc(&spec.doc, "", "//!"));
    out.extend(
        [
            "",
            "#![allow(non_camel_case_types, clippy::upper_case_acronyms, clippy::type_complexity, clippy::absolute_paths, unused_imports, dead_code, reason = \"generated from the spec\")]",
            "",
            "use core::fmt::Display;",
            "",
            "use xpute_core::abi::number::{finite, integer, unknown};",
            "use xpute_core::io::fs::{Path, Token};",
            "use xpute_kit::status::errno::Errno;",
            "use xpute_kit::status::error::MarshalError;",
            "use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};",
        ]
        .map(String::from),
    );
    out.push(format!("use {}::{{Export, Outcome}};", r.abi));
    let mut enums: Vec<&str> = spec.enums.iter().map(|e| e.name.as_str()).collect();
    enums.sort();
    out.push(format!("pub use {}::{{{}}};", r.core, enums.join(", ")));
    out.extend(extern_uses(spec)?);
    for rec in &spec.records {
        rs_view(spec, &mut out, rec)?;
    }
    for c in spec.cmds()? {
        let name = pascal(&c.name);
        if c.import.is_none() {
            let pkt = c.pkt.as_deref().unwrap_or(&[]);
            let doc = [format!("Command::{}'s packet.", c.name)];
            rs_writer(spec, &mut out, &name, Some(&doc), pkt)?;
            let lt = if rs_borrows(spec, pkt)? { "<'_>" } else { "" };
            out.extend([
                String::new(),
                format!("impl Export for {name}{lt} {{"),
                format!("    const NUMBER: u32 = {};", c.val),
                format!("    const FIELDS: u32 = {};", pkt.len()),
                "    fn write(&self, w: &mut PacketWriter) {".to_string(),
                format!("        {name}::write(self, w);"),
                "    }".to_string(),
                "}".to_string(),
            ]);
        }
        if let Some(reply) = c.reply.as_ref().filter(|r| !r.is_empty()) {
            let doc = [format!("Command::{}'s completion's packet.", c.name)];
            rs_writer(spec, &mut out, &format!("{name}Reply"), Some(&doc), reply)?;
        }
        if let Some(import) = c.import.as_ref().filter(|i| !i.is_empty()) {
            read_struct(spec, &mut out, &name, &c.name, "packet", import)?;
        }
    }
    Ok(file(&out))
}

/// Writes the enums `a` names, as `emit` would write them, into a file of
/// their own at `at`.
pub(super) fn emit_apart(spec: &Spec, t: &Target, at: &str, a: &Apart) -> String {
    let mut out = vec![
        format!("// {at}"),
        "//".into(),
        generated(t.json),
        "".into(),
    ];
    out.extend(rs_doc(
        &[
            format!(
                "The {} protocol's enums a module under its own code reads,",
                spec.protocol.name
            ),
            format!("which its code ({}) names from here.", t.rs),
        ],
        "",
        "//!",
    ));
    out.extend(
        [
            "",
            "#![allow(non_camel_case_types, clippy::upper_case_acronyms, dead_code, reason = \"generated from the spec\")]",
        ]
        .map(String::from),
    );
    for e in spec.enums.iter().filter(|e| a.names.contains(&e.name)) {
        enum_decl(e, &mut out);
    }
    file(&out)
}
