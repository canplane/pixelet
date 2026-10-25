// pxl-tool/spec/cmd/ts.rs

//! A command table as TypeScript: its enums, each packet's writer and each
//! signal's reader, for the host's side or the shell's.

use super::{literal, Entry, Field, Kind, Spec, Target, NUMBERS};
use crate::spec::{file, pascal, snake, ts_doc};

/// Only a list of u32 is packed.
fn packed(f: &Field) -> Result<(), String> {
    if f.ty == "u32" {
        Ok(())
    } else {
        Err(format!(
            "cmd.json: a list of {} ({}) is not packed",
            f.ty, f.name
        ))
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
    if f.list && kind == Kind::Number {
        packed(f)?;
        return Ok(format!(
            ".array(SequenceType.U32_ARRAY, Uint32Array.from({value}))"
        ));
    }
    if f.list && kind == Kind::Str {
        return Ok(format!(".strs({value})"));
    }
    if f.list || kind == Kind::Record {
        return Err(format!(
            "cmd.json: the shell writes no {} ({})",
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
    Ok(format!(".{method}({value})"))
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

    /// The word and array types `fields` are spelled with; a packet's list
    /// is handed in as a plain array, so names no array type (`read`).
    fn note(&mut self, spec: &Spec, fields: &[Field], read: bool) -> Result<(), String> {
        for f in fields {
            if NUMBERS.contains(&f.ty.as_str()) {
                TsUses::add(&mut self.words, f.ty.clone());
            }
            if read && f.list && NUMBERS.contains(&f.ty.as_str()) {
                TsUses::add(&mut self.arrays, format!("{}Array", f.ty.to_uppercase()));
            }
            if spec.kind(&f.ty)? == Kind::Extern {
                TsUses::add(&mut self.words, "u32".to_string());
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
        format!("export function read_{name}(b: BranchCursor): {ty} {{"),
        format!("  return {{ {} }};", reads.join(", ")),
        "}".to_string(),
    ])
}

pub fn emit(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut out = vec![
        format!("// {}", t.ts_logical),
        "//".into(),
        crate::spec::generated(t.json, t.json),
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
    let mut uses = TsUses {
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
        if let Some(packet) = &c.packet {
            // Written in place, its fields handed over as they are: no record is made for it.
            uses.note(spec, packet, false)?;
            let mut doc = vec![format!(
                "Command.{}'s packet, written in place into `to`.",
                c.name
            )];
            for f in packet {
                if let Some(d) = &f.doc {
                    if doc.len() == 1 {
                        doc.push(String::new());
                    }
                    doc.push(format!("@param {} {}", f.name, d.join(" ")));
                }
            }
            tail.push(String::new());
            tail.extend(ts_doc(&doc, ""));
            let params = packet
                .iter()
                .map(|f| Ok(format!(", {}: {}", f.name, ts_type(spec, f)?)))
                .collect::<Result<Vec<_>, String>>()?;
            let writes = packet
                .iter()
                .map(|f| ts_write(spec, f, &f.name))
                .collect::<Result<Vec<_>, String>>()?;
            tail.push(format!(
                "export function encode_{}(to: PacketRoom{}): PacketWriter {{",
                c.name.to_lowercase(),
                params.concat()
            ));
            tail.push(format!(
                "  return to.writer({}){};",
                packet.len(),
                writes.concat()
            ));
            tail.push("}".into());
        }
        if let Some(signal) = &c.signal {
            tail.extend([
                String::new(),
                format!("/** Command.{}'s signal. */", c.name),
                format!("export interface {name}Signal {{"),
            ]);
            tail.extend(uses.field_lines(spec, signal)?);
            tail.extend(["}".to_string(), String::new()]);
            tail.extend(ts_reader(
                spec,
                &c.name.to_lowercase(),
                &format!("{name}Signal"),
                signal,
            )?);
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
    // A packed list is written by its sequence type, a value rather than a type.
    if reads(&tail, "SequenceType.") {
        out.push("import { SequenceType } from \"@xpute/kit/wire/xtp/mod.ts\";".to_string());
    }
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
                        out.extend(ts_doc(doc, "  ").into_iter().enumerate().map(|(i, l)| {
                            if i == 0 {
                                l
                            } else {
                                crate::spec::align_star(l, "  ")
                            }
                        }));
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
