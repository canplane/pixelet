// pxl-tool/spec/cmd.rs

//! The command tables: xpute's (spec/xpute/abi/cmd.json), every program's
//! commands, and a program's own (pixelet's spec/pxl/web/kernel/abi/cmd.json),
//! which names xpute's as its base — the numbers a doorbell's packet is sent
//! under, the packets the host writes and the guest reads, and the signals the
//! guest writes back, each as the side that writes it and the side that reads
//! it. The majors below xpute's `program_major` are xpute's and the rest a
//! program's, and a number is given out once across the two.

use serde::Deserialize;

use super::{file, pascal, read_as, rs_doc, snake, ts_doc, Output};

#[derive(Deserialize)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    list: bool,
    doc: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    value: u32,
    doc: Option<Vec<String>>,
    packet: Option<Vec<Field>>,
    signal: Option<Vec<Field>>,
}

#[derive(Deserialize)]
struct Section {
    section: String,
    doc: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Section(Section),
    Member(Member),
}

#[derive(Deserialize)]
struct Enum {
    name: String,
    doc: Option<Vec<String>>,
    format: String,
    default: Option<String>,
    unknown: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Extern {
    name: String,
    rs: String,
    ts: Option<String>,
    repr: Option<String>,
    unknown: Option<String>,
}

#[derive(Deserialize)]
struct Record {
    name: String,
    doc: Option<Vec<String>>,
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Spec {
    doc: Vec<String>,
    /// The table this one numbers past: a program's names xpute's.
    base: Option<String>,
    /// Where a program's majors begin; xpute's table says it.
    program_major: Option<u32>,
    #[serde(rename = "extern")]
    externs: Vec<Extern>,
    enums: Vec<Enum>,
    records: Vec<Record>,
}

/// What a field's type is: a number or a bool, a string, one of the spec's
/// enums or another spec's, or one of its records.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Bool,
    Str,
    Enum,
    Extern,
    Record,
}

/// The numbers a field can be, by the width the wire gives them.
const NUMBERS: [&str; 5] = ["u8", "u32", "i32", "u64", "f64"];

impl Spec {
    fn kind(&self, ty: &str) -> Result<Kind, String> {
        Ok(if NUMBERS.contains(&ty) {
            Kind::Number
        } else if ty == "bool" {
            Kind::Bool
        } else if ty == "str" {
            Kind::Str
        } else if self.enums.iter().any(|e| e.name == ty) {
            Kind::Enum
        } else if self.externs.iter().any(|e| e.name == ty) {
            Kind::Extern
        } else if self.records.iter().any(|r| r.name == ty) {
            Kind::Record
        } else {
            return Err(format!("cmd.json: no type {ty}"));
        })
    }

    fn extern_of(&self, ty: &str) -> Option<&Extern> {
        self.externs.iter().find(|e| e.name == ty)
    }

    fn record(&self, name: &str) -> &Record {
        self.records.iter().find(|r| r.name == name).expect("a record kind() found")
    }

    fn commands(&self) -> Result<Vec<&Member>, String> {
        let table = self.enums.iter().find(|e| e.name == "Command").ok_or("cmd.json has no Command")?;
        Ok(members(table).collect())
    }
}

fn members(e: &Enum) -> impl Iterator<Item = &Member> {
    e.entries.iter().filter_map(|m| match m {
        Entry::Member(m) => Some(m),
        Entry::Section(_) => None,
    })
}

fn literal(value: u32, format: &str) -> String {
    match format {
        "dec" => value.to_string(),
        "hex2" => format!("0x{value:02x}"),
        _ => format!("0x{value:04x}"),
    }
}

fn typed_array(ty: &str, arrays: [&str; 3], name: &str) -> Result<String, String> {
    match ty {
        "u32" => Ok(arrays[0].to_string()),
        "u64" => Ok(arrays[1].to_string()),
        "f64" => Ok(arrays[2].to_string()),
        _ => Err(format!("cmd.json: a list of {ty} ({name}) is not packed")),
    }
}

// ---- TypeScript ----

fn ts_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let one = match spec.kind(&f.ty)? {
        Kind::Number => f.ty.clone(),
        Kind::Bool => "boolean".to_string(),
        Kind::Str => "string".to_string(),
        Kind::Extern if spec.extern_of(&f.ty).is_some_and(|x| x.ts.is_none()) => "u32".to_string(),
        _ => f.ty.clone(),
    };
    Ok(if f.list {
        format!("{one}[]")
    } else if f.optional {
        format!("{one} | null")
    } else {
        one
    })
}

/// A field as the shell writes it into a packet.
fn ts_write(spec: &Spec, f: &Field, value: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Number {
        // A list of numbers is one typed-array leaf, which the kernel reads
        // where it lies (`get_array`).
        let array = typed_array(&f.ty, ["Uint32Array", "BigUint64Array", "Float64Array"], &f.name)?;
        return Ok(format!(".{}_array({array}.from({value}))", f.ty));
    }
    if f.list || kind == Kind::Record {
        return Err(format!("cmd.json: the shell writes no {} ({})", if f.list { "list" } else { "record" }, f.name));
    }
    let method = match kind {
        Kind::Number => f.ty.as_str(),
        Kind::Bool => "bool",
        Kind::Str => "str",
        _ => "u32",
    };
    Ok(format!(".{method}({value})"))
}

/// A field as the shell reads it out of the branch `b`, at `i`.
fn ts_read(spec: &Spec, f: &Field, i: usize) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        if kind == Kind::Record {
            return Ok(format!("list(b.at_branch({i}), read_{})", snake(&f.ty)));
        }
        if kind != Kind::Number {
            return Err(format!("cmd.json: a list of {} ({}) is not written", f.ty, f.name));
        }
        let array = typed_array(&f.ty, ["U32Array", "U64Array", "F64Array"], &f.name)?;
        return Ok(format!("Array.from(b.at({i}).get<{array}>())"));
    }
    if kind == Kind::Record {
        return Ok(if f.optional {
            format!("optional(b.at({i}), read_{})", snake(&f.ty))
        } else {
            format!("read_{}(b.at_branch({i}))", snake(&f.ty))
        });
    }
    Ok(format!("b.at({i}).get<{}>()", ts_type(spec, f)?))
}

/// What the TypeScript names, gathered as its lines are written, so that an
/// import is written only where something uses it.
struct TsUses {
    words: Vec<String>,
    arrays: Vec<String>,
}

impl TsUses {
    fn add(list: &mut Vec<String>, name: String) {
        if !list.contains(&name) {
            list.push(name);
        }
    }

    fn field_lines(&mut self, spec: &Spec, fields: &[Field]) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for f in fields {
            if NUMBERS.contains(&f.ty.as_str()) {
                TsUses::add(&mut self.words, f.ty.clone());
            }
            if f.list && NUMBERS.contains(&f.ty.as_str()) {
                TsUses::add(&mut self.arrays, format!("{}Array", f.ty.to_uppercase()));
            }
            if spec.kind(&f.ty)? == Kind::Extern && spec.extern_of(&f.ty).is_some_and(|x| x.ts.is_none()) {
                TsUses::add(&mut self.words, "u32".to_string());
            }
            if let Some(doc) = &f.doc {
                out.extend(ts_doc(doc, "  "));
            }
            out.push(format!("  {}: {};", f.name, ts_type(spec, f)?));
        }
        Ok(out)
    }
}

fn ts_reader(spec: &Spec, name: &str, ty: &str, fields: &[Field]) -> Result<Vec<String>, String> {
    let reads = fields
        .iter()
        .enumerate()
        .map(|(i, f)| Ok(format!("{}: {}", f.name, ts_read(spec, f, i)?)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(vec![
        format!("export function read_{name}(b: BranchCursor): {ty} {{"),
        format!("  return {{ {} }};", reads.join(", ")),
        "}".to_string(),
    ])
}

fn ts(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut out = vec![
        format!("// {}", t.ts_logical),
        "//".into(),
        format!("// GENERATED by pxl-tool/bin/spec.rs from {} — do not edit.", t.json),
        "".into(),
        "/**".into(),
    ];
    out.extend(spec.doc.iter().map(|l| if l.is_empty() { " *".to_string() } else { format!(" * {l}") }));
    out.push(" */".into());
    // What the records and the commands are written as, after the enums they
    // name; the imports are what that uses, so an unused one is never written.
    let mut tail: Vec<String> = Vec::new();
    let mut uses = TsUses {
        words: Vec::new(),
        arrays: vec!["U8Array".to_string()],
    };
    for r in &spec.records {
        tail.push(String::new());
        if let Some(doc) = &r.doc {
            tail.extend(ts_doc(doc, ""));
        }
        tail.push(format!("export interface {} {{", r.name));
        tail.extend(uses.field_lines(spec, &r.fields)?);
        tail.extend(["}".to_string(), String::new()]);
        tail.extend(ts_reader(spec, &snake(&r.name), &r.name, &r.fields)?);
    }
    for c in spec.commands()? {
        let name = pascal(&c.name);
        if let Some(packet) = &c.packet {
            tail.extend([String::new(), format!("/** Command.{}'s packet. */", c.name), format!("export interface {name} {{")]);
            tail.extend(uses.field_lines(spec, packet)?);
            tail.extend(["}".to_string(), String::new()]);
            let writes = packet.iter().map(|f| ts_write(spec, f, &format!("p.{}", f.name))).collect::<Result<Vec<_>, String>>()?;
            tail.push(format!("export function encode_{}(p: {name}): U8Array {{", c.name.to_lowercase()));
            tail.push(format!("  return packet((b) => b{});", writes.concat()));
            tail.push("}".into());
        }
        if let Some(signal) = &c.signal {
            tail.extend([String::new(), format!("/** Command.{}'s signal. */", c.name), format!("export interface {name}Signal {{")]);
            tail.extend(uses.field_lines(spec, signal)?);
            tail.extend(["}".to_string(), String::new()]);
            tail.extend(ts_reader(spec, &c.name.to_lowercase(), &format!("{name}Signal"), signal)?);
        }
    }
    uses.arrays.sort();
    uses.words.sort();
    out.push(String::new());
    out.push("import type { BranchCursor, NodeCursor } from \"@xpute/core/wire/xtp/mod.ts\";".into());
    out.push(format!("import type {{ {} }} from \"@xpute/core/abi/array.ts\";", uses.arrays.join(", ")));
    out.push(format!("import type {{ {} }} from \"@xpute/core/abi/word.ts\";", uses.words.join(", ")));
    out.push("import { packet } from \"@xpute/runtime/ipc/doorbell.ts\";".into());
    for x in &spec.externs {
        if let Some(ts) = &x.ts {
            out.push(format!("import type {{ {} }} from \"{ts}\";", x.name));
        }
    }
    let helpers = [
        "",
        "/** A branch of records, each read by `read`. */",
        "function list<T>(b: BranchCursor, read: (b: BranchCursor) => T): T[] {",
        "  return Array.from({ length: b.len }, (_, k) => read(b.at_branch(k)));",
        "}",
        "",
        "/** A record that may be absent. */",
        "function optional<T>(c: NodeCursor, read: (b: BranchCursor) => T): T | null {",
        "  const present = c.opt();",
        "  return present === undefined ? null : read(present.as_branch());",
        "}",
    ];
    tail.splice(0..0, helpers.iter().map(|l| l.to_string()));
    for e in &spec.enums {
        out.push(String::new());
        if let Some(doc) = &e.doc {
            out.extend(ts_doc(doc, ""));
        }
        out.push(format!("export const enum {} {{", e.name));
        for entry in &e.entries {
            match entry {
                Entry::Section(s) => {
                    out.extend([String::new(), format!("  // ---- {} ----", s.section)]);
                    if let Some(doc) = &s.doc {
                        out.push("  //".into());
                        out.extend(super::line_comments(doc, "  "));
                    }
                    out.push(String::new());
                }
                Entry::Member(m) => {
                    if let Some(doc) = &m.doc {
                        out.extend(ts_doc(doc, "  ").into_iter().enumerate().map(|(i, l)| if i == 0 { l } else { super::align_star(l, "  ") }));
                    }
                    out.push(format!("  {} = {},", m.name, literal(m.value, &e.format)));
                }
            }
        }
        out.push("}".into());
    }
    out.extend(tail);
    Ok(file(&out))
}

// ---- Rust ----

/// Whether a record, as Rust writes it, borrows what it carries.
fn rs_borrows(spec: &Spec, fields: &[Field]) -> Result<bool, String> {
    for f in fields {
        let kind = spec.kind(&f.ty)?;
        if f.list || kind == Kind::Str || (kind == Kind::Record && rs_borrows(spec, &spec.record(&f.ty).fields)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A record's or a signal's type as a field names it: with its lifetime, or
/// with a fresh one where it is an item a list hands out.
fn rs_record(spec: &Spec, name: &str, lifetime: &str) -> Result<String, String> {
    Ok(if rs_borrows(spec, &spec.record(name).fields)? {
        format!("{name}<{lifetime}>")
    } else {
        name.to_string()
    })
}

/// The path a field's enum is named by.
fn rs_enum(spec: &Spec, ty: &str) -> String {
    spec.extern_of(ty).map(|x| x.rs.clone()).unwrap_or_else(|| ty.to_string())
}

/// A field as the kernel reads it, from a packet the shell wrote.
fn rs_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Number {
        return Ok(format!("&'a [{}]", f.ty));
    }
    if f.list || kind == Kind::Record {
        return Err(format!("cmd.json: the kernel reads no {} ({})", if f.list { "list" } else { "record" }, f.name));
    }
    let one = match kind {
        Kind::Number => f.ty.clone(),
        Kind::Str => "&'a str".to_string(),
        Kind::Bool => "bool".to_string(),
        _ => rs_enum(spec, &f.ty),
    };
    Ok(if f.optional { format!("Option<{one}>") } else { one })
}

/// The expression that reads one field out of the cursor `c`.
fn rs_read_one(spec: &Spec, f: &Field, c: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        return Ok(format!("{c}.get_array::<{}>()?", f.ty));
    }
    match kind {
        Kind::Number => {
            return Ok(match f.ty.as_str() {
                "u64" => format!("{c}.get_u64()?"),
                "f64" => format!("{c}.get_number()?"),
                ty => format!("{c}.get_number()? as {ty}"),
            })
        }
        Kind::Bool => return Ok(format!("{c}.get_bool()?")),
        Kind::Str => return Ok(format!("{c}.get_str_ref()?")),
        _ => {}
    }
    let (repr, unknown) = match spec.enums.iter().find(|e| e.name == f.ty) {
        Some(e) => (None, e.unknown.clone()),
        None => {
            let x = spec.extern_of(&f.ty).ok_or(format!("cmd.json: no type {}", f.ty))?;
            (x.repr.clone(), x.unknown.clone())
        }
    };
    let path = rs_enum(spec, &f.ty);
    let number = format!("{c}.get_number()? as u32");
    let member = if repr.as_deref() == Some("u8") {
        format!("u8::try_from({number}).ok().and_then({path}::of)")
    } else {
        format!("{path}::of({number})")
    };
    Ok(match unknown {
        Some(u) => format!("{member}.unwrap_or({path}::{u})"),
        None => format!("{member}.ok_or_else(|| unknown())?"),
    })
}

fn rs_read(spec: &Spec, f: &Field, i: usize) -> Result<String, String> {
    Ok(if f.optional {
        format!("match f.at({i})?.opt() {{ Some(c) => Some({}), None => None }}", rs_read_one(spec, f, "c")?)
    } else {
        rs_read_one(spec, f, &format!("f.at({i})?"))?
    })
}

/// A field as the kernel writes it. A string is anything that displays, since
/// what it names often lies in no string, and a list is a function handing
/// out its items, which the writer walks twice: once to count them, since a
/// branch's table precedes its children, and once to write them.
fn rs_write_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        if kind == Kind::Number {
            return Ok(format!("&'a [{}]", f.ty));
        }
        if kind != Kind::Record {
            return Err(format!("cmd.json: a list of {} ({}) is not written", f.ty, f.name));
        }
        return Ok(format!("&'a dyn Fn(&mut dyn FnMut(&{}))", rs_record(spec, &f.ty, "'_")?));
    }
    let one = match kind {
        Kind::Number => f.ty.clone(),
        Kind::Bool => "bool".to_string(),
        Kind::Str => "&'a dyn Display".to_string(),
        Kind::Record => rs_record(spec, &f.ty, "'a")?,
        _ => rs_enum(spec, &f.ty),
    };
    Ok(if f.optional { format!("Option<{one}>") } else { one })
}

fn rs_write(spec: &Spec, f: &Field) -> Result<Vec<String>, String> {
    let kind = spec.kind(&f.ty)?;
    let v = format!("self.{}", f.name);
    let fields = format!("{}::FIELDS", f.ty);
    if f.list {
        if kind == Kind::Number {
            return Ok(vec![format!("w.{}_array(Some({v}));", f.ty)]);
        }
        return Ok(vec![
            "let mut n = 0;".to_string(),
            format!("({v})(&mut |_| n += 1);"),
            format!("w.branch(n, |w| ({v})(&mut |item| {{"),
            format!("    w.branch({fields}, |w| item.write(w));"),
            "}));".to_string(),
        ]);
    }
    Ok(vec![match kind {
        Kind::Record if f.optional => format!("match &{v} {{ Some(r) => w.branch({fields}, |w| r.write(w)), None => w.no_branch() }};"),
        Kind::Record => format!("w.branch({fields}, |w| {v}.write(w));"),
        Kind::Str if f.optional => format!("match {v} {{ Some(s) => w.str_display(s), None => w.str(None) }};"),
        Kind::Str => format!("w.str_display({v});"),
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

fn rs_writer(spec: &Spec, out: &mut Vec<String>, name: &str, doc: Option<&[String]>, fields: &[Field]) -> Result<(), String> {
    let borrows = rs_borrows(spec, fields)?;
    out.push(String::new());
    if let Some(doc) = doc {
        out.extend(rs_doc(doc, "", "///"));
    }
    out.push(format!("pub struct {name}{} {{", if borrows { "<'a>" } else { "" }));
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
        "    pub fn write(&self, w: &mut PacketWriter) {".to_string(),
    ]);
    for f in fields {
        out.extend(rs_write(spec, f)?.into_iter().map(|l| format!("        {l}")));
    }
    out.extend(["    }".to_string(), "}".to_string()]);
    Ok(())
}

fn rs(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut out = vec![
        format!("// {}/cmd.spec.rs", t.rs_logical),
        "//".into(),
        format!("// GENERATED by pxl-tool/bin/spec.rs from {} — do not edit.", t.json),
        "".into(),
    ];
    out.extend(rs_doc(&spec.doc, "", "//!"));
    out.extend(
        [
            "",
            "// A list a record carries is a function handing out its items, whose",
            "// type is long to spell and is spelled once, here, for each. A table",
            "// with no signal or no enum read off the wire leaves a writer or",
            "// `unknown` unused.",
            "#![allow(non_camel_case_types, clippy::type_complexity, unused_imports, dead_code)]",
            "",
            "use core::fmt::Display;",
            "",
            "use xpute_core::status::errno::Errno;",
            "use xpute_core::status::error::MarshalError;",
            "use xpute_core::wire::xtp::{PacketWriter, TreeReader};",
            "",
            "/// A number an enum does not give out, where the enum has no member it reads as.",
            "#[track_caller]",
            "fn unknown() -> MarshalError {",
            "    MarshalError::new(Errno::EINVAL)",
            "}",
        ]
        .map(String::from),
    );
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        out.push(String::new());
        if let Some(doc) = &e.doc {
            out.extend(rs_doc(doc, "", "///"));
        }
        out.push(format!("#[derive(Clone, Copy, Debug, {}PartialEq, Eq)]", if e.default.is_some() { "Default, " } else { "" }));
        out.push("#[repr(u32)]".into());
        out.push(format!("pub enum {} {{", e.name));
        for entry in &e.entries {
            match entry {
                Entry::Section(s) => {
                    out.extend([String::new(), format!("    // ---- {} ----", s.section)]);
                    if let Some(doc) = &s.doc {
                        out.push("    //".into());
                        out.extend(super::line_comments(doc, "    "));
                    }
                }
                Entry::Member(m) => {
                    if let Some(doc) = &m.doc {
                        out.extend(rs_doc(doc, "    ", "///"));
                    }
                    if e.default.as_deref() == Some(m.name.as_str()) {
                        out.push("    #[default]".into());
                    }
                    out.push(format!("    {} = {},", m.name, literal(m.value, &e.format)));
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
                all.iter().map(|m| format!("{}::{}", e.name, m.name)).collect::<Vec<_>>().join(", ")
            ),
            String::new(),
            "    /// The member a number names, or none for a number nobody gave out.".to_string(),
            format!("    pub fn of(number: u32) -> Option<{}> {{", e.name),
            format!("        {}::ALL.into_iter().find(|m| *m as u32 == number)", e.name),
            "    }".to_string(),
            "}".to_string(),
        ]);
    }
    for r in &spec.records {
        rs_writer(spec, &mut out, &r.name, r.doc.as_deref(), &r.fields)?;
    }
    for c in spec.commands()? {
        let name = pascal(&c.name);
        if let Some(packet) = &c.packet {
            let mut borrows = false;
            for f in packet {
                borrows |= f.list || spec.kind(&f.ty)? == Kind::Str;
            }
            let lt = if borrows { "<'a>" } else { "" };
            out.extend([
                String::new(),
                format!("/// Command::{}'s packet.", c.name),
                "#[derive(Clone, Copy, Debug)]".to_string(),
                format!("pub struct {name}{lt} {{"),
            ]);
            for f in packet {
                if let Some(doc) = &f.doc {
                    out.extend(rs_doc(doc, "    ", "///"));
                }
                out.push(format!("    pub {}: {},", f.name, rs_read_type(spec, f)?));
            }
            out.extend(["}".to_string(), String::new(), format!("impl{lt} {name}{lt} {{")]);
            out.push(format!("    pub fn read(pkt: &{}[u8]) -> Result<{name}{lt}, MarshalError> {{", if borrows { "'a " } else { "" }));
            out.push("        let f = TreeReader::new(pkt)?.read_branch()?;".into());
            out.push(format!("        Ok({name} {{"));
            for (i, f) in packet.iter().enumerate() {
                out.push(format!("            {}: {},", f.name, rs_read(spec, f, i)?));
            }
            out.extend(["        })".to_string(), "    }".to_string(), "}".to_string()]);
        }
        if let Some(signal) = &c.signal {
            let doc = [format!("Command::{}'s signal.", c.name)];
            rs_writer(spec, &mut out, &format!("{name}Signal"), Some(&doc), signal)?;
        }
    }
    out.extend(["", "#[cfg(test)]", "#[path = \"cmd.test.rs\"]", "mod test;"].map(String::from));
    Ok(file(&out))
}

/// The command table's own test, beside it rather than inside it: every `.rs`
/// in the crates keeps its tests in a `.test.rs` at the same path, and a
/// generated file is no exception.
fn test_rs(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut cmds: Vec<&Member> = Vec::new();
    let mut minors: Vec<&Enum> = Vec::new();
    let mut majors: Vec<(u32, &str)> = Vec::new();
    for e in &spec.enums {
        for m in members(e) {
            if e.name == "Command" {
                cmds.push(m);
            } else if e.name == "Major" {
                majors.push((m.value, &m.name));
            } else if e.name.ends_with("Minor") && !minors.iter().any(|t| t.name == e.name) {
                minors.push(e);
            }
        }
    }
    let mut out: Vec<String> = vec![
        format!("// {}/cmd.test.rs", t.rs_logical),
        "//".to_string(),
        format!("// GENERATED by pxl-tool/bin/spec.rs from {} — do not edit.", t.json),
    ];
    out.extend(
        [
            "",
            "use super::*;",
            "use xpute_core::abi::cmd::cmd;",
            "",
            "#[test]",
            "fn every_number_is_its_major_and_minor() {",
            "    use Command::*;",
            "    let parts = [",
        ]
        .map(String::from),
    );
    for c in cmds {
        if c.value == 0 {
            continue;
        }
        let major = majors
            .iter()
            .rev()
            .find(|(v, _)| *v == c.value >> 8)
            .map(|(_, n)| *n)
            .ok_or(format!("cmd.json: {} has no major", c.name))?;
        let minor_value = c.value & 0xff;
        let table_name = format!("{}Minor", pascal(major));
        let table = minors.iter().find(|t| t.name == table_name);
        let entry = table.and_then(|t| members(t).find(|m| m.value == minor_value));
        let minor = match (table, entry) {
            (Some(t), Some(m)) => format!("{}::{}", t.name, m.name),
            _ => return Err(format!("cmd.json: {}'s minor is in no {table_name}", c.name)),
        };
        out.push(format!("        ({}, Major::{major} as u32, {minor} as u32),", c.name));
    }
    out.extend(
        [
            "    ];",
            "    for (c, major, minor) in parts {",
            "        assert_eq!(c as u32, cmd(major, minor), \"{c:?}\");",
            "        assert_eq!(Command::of(c as u32), Some(c));",
            "    }",
            "    assert_eq!(Command::of(0x0fff), None);",
            "}",
        ]
        .map(String::from),
    );
    Ok(file(&out))
}

/// Where a table's code is written, and the names its files open on.
struct Target {
    json: &'static str,
    ts: &'static str,
    ts_logical: &'static str,
    /// The directory the Rust files go in.
    rs_dir: &'static str,
    /// The crate and path under `src/` they are named by.
    rs_logical: &'static str,
}

const TARGETS: [Target; 2] = [
    Target {
        json: "spec/xpute/abi/cmd.json",
        ts: "packages/xpute/runtime/src/abi/cmd.spec.ts",
        ts_logical: "@xpute/runtime/abi/cmd.spec.ts",
        rs_dir: "crates/xpute/runtime/src/abi",
        rs_logical: "xpute-runtime/abi",
    },
    Target {
        json: "spec/pxl/web/kernel/abi/cmd.json",
        ts: "apps/web/src/kernel/abi/cmd.spec.ts",
        ts_logical: "@pxl/web/kernel/abi/cmd.spec.ts",
        rs_dir: "crates/pxl/web/kernel/src/abi",
        rs_logical: "pxl-kernel/abi",
    },
];

/// A table's majors, by number.
fn majors(spec: &Spec) -> Vec<(u32, &str)> {
    spec.enums.iter().filter(|e| e.name == "Major").flat_map(|e| members(e).map(|m| (m.value, m.name.as_str()))).collect()
}

/// A program's table against the base it names: its majors past the base's
/// `program_major`, the base's under it, and no command named in both.
fn check_base(spec: &Spec, json: &str) -> Result<(), String> {
    let Some(base_path) = &spec.base else { return Ok(()) };
    let base: Spec = read_as(base_path)?;
    let edge = base.program_major.ok_or(format!("{base_path}: no program_major"))?;
    for (value, name) in majors(&base) {
        if value >= edge {
            return Err(format!("{base_path}: major {name} at {value:#x} is a program's"));
        }
    }
    for (value, name) in majors(spec) {
        if value < edge {
            return Err(format!("{json}: major {name} at {value:#x} is below {edge:#x}, xpute's"));
        }
    }
    let base_names: Vec<String> = base.commands()?.iter().map(|c| c.name.clone()).collect();
    for c in spec.commands()? {
        if base_names.contains(&c.name) {
            return Err(format!("{json}: {} is {base_path}'s", c.name));
        }
    }
    Ok(())
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    for t in &TARGETS {
        let spec: Spec = read_as(t.json)?;
        check_base(&spec, t.json)?;
        out.push(Output::new(t.ts, ts(&spec, t)?));
        out.push(Output::new(&format!("{}/cmd.spec.rs", t.rs_dir), rs(&spec, t)?));
        out.push(Output::new(&format!("{}/cmd.test.rs", t.rs_dir), test_rs(&spec, t)?));
    }
    Ok(out)
}
