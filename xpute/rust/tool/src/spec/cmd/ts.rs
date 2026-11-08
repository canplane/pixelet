// xpute-tool/spec/cmd/ts.rs

//! This writes a command table as TypeScript for the runtime: its enums, a
//! writer for each packet and a reader for each import.

use super::{literal, Entry, Field, Kind, Spec, Target, NUMBERS};
use crate::spec::{file, generated, pascal, snake, ts_doc, Output};

fn check_packed(f: &Field) -> Result<(), String> {
    if f.ty == "u32" {
        Ok(())
    } else {
        Err(format!(
            "cmd.json: a list of {} ({}) is not packed",
            f.ty, f.name
        ))
    }
}

pub(super) fn ts_type(spec: &Spec, f: &Field) -> Result<String, String> {
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

fn ts_write(spec: &Spec, f: &Field, val: &str) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Number {
        check_packed(f)?;
        return Ok(format!(
            ".array(SequenceType.U32_ARRAY, Uint32Array.from({val}))"
        ));
    }
    if f.list && kind == Kind::Str {
        return Ok(format!(".strs({val})"));
    }
    if f.list || kind == Kind::Record {
        return Err(format!(
            "cmd.json: TypeScript writes no {} ({})",
            if f.list { "list" } else { "record" },
            f.name
        ));
    }
    let method = match kind {
        Kind::Number => f.ty.as_str(),
        Kind::Bool => "bool",
        Kind::Str => "str",
        _ => "u32",
    };
    Ok(format!(".{method}({val})"))
}

fn ts_read(spec: &Spec, f: &Field, i: usize) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        if kind == Kind::Record {
            return Ok(format!("list(b.at_branch({i}), read_{})", snake(&f.ty)));
        }
        if kind != Kind::Number {
            return Err(format!(
                "cmd.json: a list of {} ({}) is not written",
                f.ty, f.name
            ));
        }
        check_packed(f)?;
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

struct TsUse {
    words: Vec<String>,
    arrays: Vec<String>,
}

impl TsUse {
    fn add(list: &mut Vec<String>, name: String) {
        if !list.contains(&name) {
            list.push(name);
        }
    }

    /// Collects the word and array types that `fields` are spelled with.
    /// Only a reader (`read`) names a typed array for a list of numbers; a
    /// writer is handed the list as a plain array.
    fn note(&mut self, spec: &Spec, fields: &[Field], read: bool) -> Result<(), String> {
        for f in fields {
            if NUMBERS.contains(&f.ty.as_str()) {
                TsUse::add(&mut self.words, f.ty.clone());
            }
            if read && f.list && NUMBERS.contains(&f.ty.as_str()) {
                TsUse::add(&mut self.arrays, format!("{}Array", f.ty.to_uppercase()));
            }
            if spec.kind(&f.ty)? == Kind::Extern {
                TsUse::add(&mut self.words, "u32".to_string());
            }
        }
        Ok(())
    }

    fn field_lines(&mut self, spec: &Spec, fields: &[Field]) -> Result<Vec<String>, String> {
        self.note(spec, fields, true)?;
        let mut out = Vec::new();
        for f in fields {
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
        format!(
            "export function read_{name}({}: BranchCursor): {ty} {{",
            if fields.is_empty() { "_b" } else { "b" }
        ),
        format!("  return {{ {} }};", reads.join(", ")),
        "}".to_string(),
    ])
}

/// Writes the table's module, and beside it a module of each enum's
/// constants.
pub(super) fn emit(spec: &Spec, t: &Target) -> Result<Vec<Output>, String> {
    let mut out = vec![
        format!("// {}", t.ts_logical),
        "//".into(),
        generated(t.json),
        "".into(),
        "/**".into(),
    ];
    out.extend(spec.doc.iter().map(|l| {
        if l.is_empty() {
            " *".to_string()
        } else {
            format!(" * {l}")
        }
    }));
    out.push(" */".into());
    let mut tail: Vec<String> = Vec::new();
    let mut uses = TsUse {
        words: Vec::new(),
        arrays: Vec::new(),
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
        let lower = c.name.to_lowercase();
        let pkts = [
            (c.pkt.as_ref(), lower.clone(), "packet"),
            (
                c.reply.as_ref().filter(|r| !r.is_empty()),
                format!("{lower}_reply"),
                "completion's packet",
            ),
        ];
        for (pkt, encode, what) in pkts {
            let Some(pkt) = pkt else {
                continue;
            };
            // The packet is written in place from the fields as they are handed
            // over, so no record type is made for it.
            uses.note(spec, pkt, false)?;
            let mut doc = vec![format!(
                "Command.{}'s {what}, written in place into `to`.",
                c.name
            )];
            for f in pkt {
                if let Some(d) = &f.doc {
                    if doc.len() == 1 {
                        doc.push(String::new());
                    }
                    doc.push(format!("@param {} {}", f.name, d.join(" ")));
                }
            }
            tail.push(String::new());
            tail.extend(ts_doc(&doc, ""));
            let params = pkt
                .iter()
                .map(|f| Ok(format!(", {}: {}", f.name, ts_type(spec, f)?)))
                .collect::<Result<Vec<_>, String>>()?;
            let writes = pkt
                .iter()
                .map(|f| ts_write(spec, f, &f.name))
                .collect::<Result<Vec<_>, String>>()?;
            tail.push(format!(
                "export function encode_{encode}(to: PacketRoom{}): PacketWriter {{",
                params.concat()
            ));
            tail.push(format!(
                "  return to.writer({}){};",
                pkt.len(),
                writes.concat()
            ));
            tail.push("}".into());
        }
        if let Some(import) = &c.import {
            tail.extend([
                String::new(),
                format!("/** Command.{}'s packet. */", c.name),
                format!("export interface {name} {{"),
            ]);
            tail.extend(uses.field_lines(spec, import)?);
            tail.extend(["}".to_string(), String::new()]);
            tail.extend(ts_reader(spec, &c.name.to_lowercase(), &name, import)?);
        }
    }
    uses.arrays.sort();
    uses.words.sort();
    // A helper or a cursor type is written only where a reader uses it,
    // because an unused one fails the lint.
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
    let xtp: Vec<&str> = ["BranchCursor", "NodeCursor", "PacketRoom", "PacketWriter"]
        .into_iter()
        .filter(|c| reads(&tail, c))
        .collect();
    out.push(String::new());
    for (names, from) in [
        (
            xtp.iter().map(|s| s.to_string()).collect(),
            "wire/xtp/mod.ts",
        ),
        (uses.arrays, "abi/array.ts"),
        (uses.words, "abi/word.ts"),
    ] {
        let names: Vec<String> = names;
        if !names.is_empty() {
            out.push(format!(
                "import type {{ {} }} from \"@xpute/kit/{from}\";",
                names.join(", ")
            ));
        }
    }
    // A packed list is written through its SequenceType, which is a value and
    // not a type, so it takes a plain import.
    if reads(&tail, "SequenceType.") {
        out.push("import { SequenceType } from \"@xpute/kit/wire/xtp/mod.ts\";".to_string());
    }
    let mut modules: Vec<Output> = Vec::new();
    let stem = stem_of(t.ts);
    for e in &spec.enums {
        out.push(format!(
            "import * as {} from \"./{stem}/{}.spec.ts\";",
            e.name,
            snake(&e.name)
        ));
    }
    for e in &spec.enums {
        let (path, logical) = enum_module(t, &e.name);
        modules.push(Output::new(&path, constants(e, &logical, t.json)));
        out.push(String::new());
        out.push(format!(
            "/** One of {}'s, as its enum was a type. */",
            e.name
        ));
        out.push(format!(
            "type {0} = (typeof {0})[keyof typeof {0}];",
            e.name
        ));
        out.push(format!("export {{ {} }};", e.name));
    }
    out.extend(tail);
    let mut outputs = vec![Output::new(t.ts, file(&out))];
    outputs.extend(modules);
    Ok(outputs)
}

/// The table's file name without `.spec.ts` names the folder its enums'
/// modules lie in.
fn stem_of(ts: &str) -> &str {
    let name = ts.rsplit_once('/').map_or(ts, |(_, n)| n);
    name.strip_suffix(".spec.ts").unwrap_or(name)
}

/// This is the path of an enum's module, in the folder beside the table that is
/// named after it (`cmd.spec.ts`'s enums go in `cmd/`), with its logical path.
fn enum_module(t: &Target, name: &str) -> (String, String) {
    let stem = stem_of(t.ts);
    let at = |p: &str| {
        let dir = p.rsplit_once('/').map_or("", |(d, _)| d);
        format!("{dir}/{stem}/{}.spec.ts", snake(name))
    };
    (at(t.ts), at(t.ts_logical))
}

/// Writes an enum as a module that holds one constant per member, whose value
/// is the member's literal.
fn constants(e: &super::Enum, logical: &str, json: &str) -> String {
    let mut out = vec![
        format!("// {logical}"),
        "//".into(),
        generated(json),
        String::new(),
    ];
    if let Some(doc) = &e.doc {
        out.extend(doc.iter().map(|l| format!("// {l}").trim_end().to_string()));
        out.push(String::new());
    }
    for entry in &e.entries {
        match entry {
            Entry::Section(s) => {
                out.extend([
                    String::new(),
                    format!("// ---- {} ----", s.section),
                    String::new(),
                ]);
            }
            Entry::Member(m) => {
                if let Some(doc) = &m.doc {
                    out.extend(ts_doc(doc, ""));
                }
                out.push(format!(
                    "export const {} = {};",
                    m.name,
                    literal(m.val, &e.format)
                ));
            }
        }
    }
    file(&out)
}
