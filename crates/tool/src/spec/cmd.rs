// pxl-tool/spec/cmd.rs

//! The command tables, xpute's and a program's on top of it. Majors below
//! xpute's `program_major` are xpute's; a number is given out once across both.

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
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    value: u32,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    packet: Option<Vec<Field>>,
    signal: Option<Vec<Field>>,
}

#[derive(Deserialize)]
struct Section {
    section: String,
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
    #[serde(rename = "$doc")]
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
    repr: Option<String>,
}

#[derive(Deserialize)]
struct Record {
    name: String,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Spec {
    #[serde(rename = "$doc")]
    doc: Vec<String>,
    base: Option<String>,
    program_major: Option<u32>,
    #[serde(rename = "extern")]
    externs: Vec<Extern>,
    enums: Vec<Enum>,
    records: Vec<Record>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Bool,
    Str,
    Enum,
    Extern,
    Record,
}

const NUMBERS: [&str; 4] = ["u8", "u32", "i32", "f64"];

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

/// Only a list of u32 is packed.
fn packed(f: &Field) -> Result<(), String> {
    if f.ty == "u32" {
        Ok(())
    } else {
        Err(format!("cmd.json: a list of {} ({}) is not packed", f.ty, f.name))
    }
}

fn ts_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let one = match spec.kind(&f.ty)? {
        Kind::Number => f.ty.clone(),
        Kind::Bool => "boolean".to_string(),
        Kind::Str => "string".to_string(),
        Kind::Extern => "u32".to_string(),
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

fn ts_write(spec: &Spec, f: &Field, value: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
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

fn ts_read(spec: &Spec, f: &Field, i: usize) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        if kind == Kind::Record {
            return Ok(format!("list(b.at_branch({i}), read_{})", snake(&f.ty)));
        }
        if kind != Kind::Number {
            return Err(format!("cmd.json: a list of {} ({}) is not written", f.ty, f.name));
        }
        packed(f)?;
        return Ok(format!("Array.from(b.at({i}).get<U32Array>())"));
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
            if spec.kind(&f.ty)? == Kind::Extern {
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
    let mut out = vec![format!("// {}", t.ts_logical), "//".into(), super::generated(t.json, t.json), "".into(), "/**".into()];
    out.extend(spec.doc.iter().map(|l| if l.is_empty() { " *".to_string() } else { format!(" * {l}") }));
    out.push(" */".into());
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
    // A helper, and a cursor type, only where a reader reads through it: an unread one fails lint.
    let reads = |tail: &[String], call: &str| tail.iter().any(|l| l.contains(call));
    let mut helpers: Vec<&str> = Vec::new();
    if reads(&tail, "list(") {
        helpers.extend([
            "",
            "/** A branch of records, each read by `read`. */",
            "function list<T>(b: BranchCursor, read: (b: BranchCursor) => T): T[] {",
            "  return Array.from({ length: b.len }, (_, k) => read(b.at_branch(k)));",
            "}",
        ]);
    }
    if reads(&tail, "optional(") {
        helpers.extend([
            "",
            "/** A record that may be absent. */",
            "function optional<T>(c: NodeCursor, read: (b: BranchCursor) => T): T | null {",
            "  const present = c.opt();",
            "  return present === undefined ? null : read(present.as_branch());",
            "}",
        ]);
    }
    tail.splice(0..0, helpers.iter().map(|l| l.to_string()));
    let cursors: Vec<&str> = ["BranchCursor", "NodeCursor"].into_iter().filter(|c| reads(&tail, c)).collect();
    out.push(String::new());
    if !cursors.is_empty() {
        out.push(format!("import type {{ {} }} from \"@xpute/kit/wire/xtp/mod.ts\";", cursors.join(", ")));
    }
    out.push(format!("import type {{ {} }} from \"@xpute/kit/abi/array.ts\";", uses.arrays.join(", ")));
    out.push(format!("import type {{ {} }} from \"@xpute/kit/abi/word.ts\";", uses.words.join(", ")));
    out.push("import { packet } from \"@xpute/host/ipc/doorbell.ts\";".into());
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

fn cpp_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        return Err(format!("cmd.json: a C++ guest reads no list ({})", f.name));
    }
    let one = match kind {
        Kind::Number => cpp_number(&f.ty).to_string(),
        Kind::Str => "std::string_view".to_string(),
        Kind::Bool => "bool".to_string(),
        _ => return Err(format!("cmd.json: a C++ guest reads no {} ({})", f.ty, f.name)),
    };
    Ok(if f.optional { format!("std::optional<{one}>") } else { one })
}

fn cpp_number(ty: &str) -> &'static str {
    match ty {
        "u8" => "std::uint8_t",
        "u32" => "std::uint32_t",
        "i32" => "std::int32_t",
        _ => "double",
    }
}

fn cpp_read_one(spec: &Spec, f: &Field, c: &str, to: &str) -> Result<Vec<String>, String> {
    let (call, conv) = match spec.kind(&f.ty)? {
        Kind::Number => match f.ty.as_str() {
            "f64" => (format!("{c}->get_number()"), "*v".to_string()),
            ty => (format!("{c}->get_number()"), format!("saturate_as<{}>(*v)", cpp_number(ty))),
        },
        Kind::Bool => (format!("{c}->get_bool()"), "*v".to_string()),
        _ => (format!("{c}->get_str_ref()"), "*v".to_string()),
    };
    Ok(vec![format!("auto v = {call};"), "if (!v) return v.error();".to_string(), format!("{to} = {conv};")])
}

fn cpp_read(spec: &Spec, f: &Field, i: usize) -> Result<Vec<String>, String> {
    let mut out = vec!["{".to_string(), format!("    Result<xtp::Cursor> c = f->at({i});"), "    if (!c) return c.error();".to_string()];
    if f.optional {
        out.push("    if (!c->is_null()) {".to_string());
        out.extend(cpp_read_one(spec, f, "c", &format!("out.{}", f.name))?.into_iter().map(|l| format!("        {l}")));
        out.push("    }".to_string());
    } else {
        out.extend(cpp_read_one(spec, f, "c", &format!("out.{}", f.name))?.into_iter().map(|l| format!("    {l}")));
    }
    out.push("}".to_string());
    Ok(out)
}

fn cpp(spec: &Spec, t: &Target, path: &str, namespace: &str) -> Result<String, String> {
    let mut out = vec![format!("// {}", super::in_xpute(path)), "//".into(), super::generated(t.json, t.json), "//".into()];
    out.extend(super::line_comments(&spec.doc, ""));
    out.extend(
        [
            "",
            "#pragma once",
            "",
            "#include <array>",
            "#include <cstdint>",
            "#include <optional>",
            "#include <span>",
            "#include <string_view>",
            "",
            "#include \"../../kit/abi/cmd.hpp\"",
            "#include \"../../kit/math/scalar.hpp\"",
            "#include \"../../kit/status/error.hpp\"",
            "#include \"../../kit/wire/xtp/cursor.hpp\"",
            "#include \"../../kit/wire/xtp/encode.hpp\"",
            "",
            &format!("namespace {namespace} {{"),
        ]
        .map(String::from),
    );
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        out.push(String::new());
        if let Some(doc) = &e.doc {
            out.extend(super::line_comments(doc, ""));
        }
        out.push(format!("enum class {} : std::uint32_t {{", e.name));
        for entry in &e.entries {
            match entry {
                Entry::Section(s) => {
                    out.extend([String::new(), format!("    // ---- {} ----", s.section)]);
                }
                Entry::Member(m) => {
                    if let Some(doc) = &m.doc {
                        out.extend(super::line_comments(doc, "    "));
                    }
                    out.push(format!("    {} = {},", m.name, literal(m.value, &e.format)));
                }
            }
        }
        out.extend([
            "};".to_string(),
            String::new(),
            format!("}} // namespace {namespace}"),
            String::new(),
            format!("template <> struct xpute::Members<{namespace}::{}> {{", e.name),
            format!(
                "    static constexpr std::array<{namespace}::{0}, {1}> ALL{{{2}}};",
                e.name,
                all.len(),
                all.iter().map(|m| format!("{namespace}::{}::{}", e.name, m.name)).collect::<Vec<_>>().join(", ")
            ),
            "};".to_string(),
            String::new(),
            format!("namespace {namespace} {{"),
        ]);
    }
    if !spec.records.is_empty() {
        return Err(format!("{}: a C++ guest writes no record", t.json));
    }
    for c in spec.commands()? {
        let name = pascal(&c.name);
        if c.signal.is_some() {
            return Err(format!("{}: a C++ guest raises no signal ({})", t.json, c.name));
        }
        if let Some(packet) = &c.packet {
            out.extend([String::new(), format!("// Command::{}'s packet.", c.name), format!("struct {name} {{")]);
            for f in packet {
                if let Some(doc) = &f.doc {
                    out.extend(super::line_comments(doc, "    "));
                }
                out.push(format!("    {} {}{{}};", cpp_read_type(spec, f)?, f.name));
            }
            out.extend([
                String::new(),
                format!("    static Result<{name}> read(std::span<const std::uint8_t> pkt) noexcept {{"),
                "        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);".to_string(),
                "        if (!t) return t.error();".to_string(),
                "        Result<xtp::BranchCursor> f = t->read_branch();".to_string(),
                "        if (!f) return f.error();".to_string(),
                format!("        {name} out;"),
            ]);
            for (i, f) in packet.iter().enumerate() {
                out.extend(cpp_read(spec, f, i)?.into_iter().map(|l| format!("        {l}")));
            }
            out.extend(["        return out;".to_string(), "    }".to_string(), "};".to_string()]);
        }
    }
    out.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&out))
}

fn rs_borrows(spec: &Spec, fields: &[Field]) -> Result<bool, String> {
    for f in fields {
        let kind = spec.kind(&f.ty)?;
        if f.list || kind == Kind::Str || (kind == Kind::Record && rs_borrows(spec, &spec.record(&f.ty).fields)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rs_record(spec: &Spec, name: &str, lifetime: &str) -> Result<String, String> {
    Ok(if rs_borrows(spec, &spec.record(name).fields)? {
        format!("{name}<{lifetime}>")
    } else {
        name.to_string()
    })
}

fn rs_enum(spec: &Spec, ty: &str) -> String {
    spec.extern_of(ty).map(|x| x.rs.clone()).unwrap_or_else(|| ty.to_string())
}

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

fn rs_read_one(spec: &Spec, f: &Field, c: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
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
        _ => {}
    }
    let (repr, unknown) = match spec.enums.iter().find(|e| e.name == f.ty) {
        Some(e) => (None, e.unknown.clone()),
        None => {
            let x = spec.extern_of(&f.ty).ok_or(format!("cmd.json: no type {}", f.ty))?;
            (x.repr.clone(), None)
        }
    };
    let path = rs_enum(spec, &f.ty);
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
        format!("match f.at({i})?.opt() {{ Some(c) => Some({}), None => None }}", rs_read_one(spec, f, "c")?)
    } else {
        rs_read_one(spec, f, &format!("f.at({i})?"))?
    })
}

/// A list is walked twice, once to count, since a branch's table precedes its
/// children.
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
    if f.optional && kind == Kind::Str {
        return Err(format!("cmd.json: an optional str ({}) is not written", f.name));
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
        format!("    pub fn write(&self, {}: &mut PacketWriter) {{", if fields.is_empty() { "_" } else { "w" }),
    ]);
    for f in fields {
        out.extend(rs_write(spec, f)?.into_iter().map(|l| format!("        {l}")));
    }
    out.extend(["    }".to_string(), "}".to_string()]);
    Ok(())
}

fn rs(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut out = vec![format!("// {}/cmd.spec.rs", t.rs_logical), "//".into(), super::generated(t.json, t.json), "".into()];
    out.extend(rs_doc(&spec.doc, "", "//!"));
    out.extend(
        [
            "",
            "// A list a record carries is a function handing out its items, whose",
            "// type is long to spell and is spelled once, here, for each. A table",
            "// with no signal or no enum read off the wire leaves a writer or a",
            "// number's check unused.",
            "#![allow(non_camel_case_types, clippy::type_complexity, unused_imports, dead_code)]",
            "",
            "use core::fmt::Display;",
            "",
            "use xpute_kit::status::error::MarshalError;",
            "use xpute_kit::wire::xtp::{PacketWriter, TreeReader};",
        ]
        .map(String::from),
    );
    out.push(format!("use {}::{{finite, integer, unknown}};", t.number));
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
    Ok(file(&out))
}

/// Every command is numbered by a major and a minor that cmd.json names.
fn check_numbers(spec: &Spec, json: &str) -> Result<(), String> {
    let mut cmds: Vec<&Member> = Vec::new();
    let mut minors: Vec<&Enum> = Vec::new();
    let mut majors: Vec<(u32, &str)> = Vec::new();
    for e in &spec.enums {
        for m in members(e) {
            if e.name == "Command" {
                cmds.push(m);
            } else if e.name == "Major" {
                majors.push((m.value, &m.name));
            } else if e.name.ends_with("Minor") {
                minors.push(e);
            }
        }
    }
    for c in cmds.into_iter().filter(|c| c.value != 0) {
        let major = majors.iter().find(|(v, _)| *v == c.value >> 8).map(|(_, n)| *n).ok_or(format!("{json}: {} has no major", c.name))?;
        let table_name = format!("{}Minor", pascal(major));
        let minor = minors.iter().find(|t| t.name == table_name).and_then(|t| members(t).find(|m| m.value == c.value & 0xff));
        if minor.is_none() {
            return Err(format!("{json}: {}'s minor is in no {table_name}", c.name));
        }
    }
    Ok(())
}

struct Target {
    json: &'static str,
    ts: &'static str,
    ts_logical: &'static str,
    rs_dir: &'static str,
    rs_logical: &'static str,
    /// Where the number checks a packet's reader calls are.
    number: &'static str,
    cpp: Option<(&'static str, &'static str)>,
}

const TARGETS: [Target; 2] = [
    Target {
        json: "xpute/spec/abi/cmd.json",
        ts: "xpute/ts/host/src/abi/cmd.spec.ts",
        ts_logical: "@xpute/host/abi/cmd.spec.ts",
        rs_dir: "xpute/rust/guest/src/abi",
        rs_logical: "xpute-guest/abi",
        number: "crate::abi::number",
        cpp: Some(("xpute/cpp/guest/abi/cmd.spec.hpp", "xpute::abi")),
    },
    Target {
        json: "spec/client/kernel/abi/cmd.json",
        ts: "apps/web/src/kernel/abi/cmd.spec.ts",
        ts_logical: "@pxl/web/kernel/abi/cmd.spec.ts",
        rs_dir: "crates/client/kernel/src/abi",
        rs_logical: "pxl-kernel/abi",
        number: "xpute_guest::abi::number",
        cpp: None,
    },
];

fn majors(spec: &Spec) -> Vec<(u32, &str)> {
    spec.enums.iter().filter(|e| e.name == "Major").flat_map(|e| members(e).map(|m| (m.value, m.name.as_str()))).collect()
}

/// A program's majors are past the base's `program_major`, and no command is named in both.
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
        check_numbers(&spec, t.json)?;
        out.push(Output::new(t.ts, ts(&spec, t)?));
        out.push(Output::new(&format!("{}/cmd.spec.rs", t.rs_dir), rs(&spec, t)?));
        if let Some((path, namespace)) = t.cpp {
            out.push(Output::new(path, cpp(&spec, t, path, namespace)?));
        }
    }
    Ok(out)
}
