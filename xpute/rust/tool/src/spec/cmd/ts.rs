// xpute-tool/spec/cmd/ts.rs

//! This writes a command table as TypeScript for the runtime: its enums and a
//! reader for each import, keyed by the command's number.

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

/// The type a caller hands for a field of a packet the runtime sends.
pub(super) fn ts_arg(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Number {
        check_packed(f)?;
        return Ok("U32Array".to_string());
    }
    if (f.list && kind != Kind::Str) || kind == Kind::Record {
        return Err(format!(
            "cmd.json: TypeScript writes no {} ({})",
            if f.list { "list" } else { "record" },
            f.name
        ));
    }
    ts_type(spec, f)
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
    fn note(&mut self, spec: &Spec, fields: &[Field]) -> Result<(), String> {
        for f in fields {
            if NUMBERS.contains(&f.ty.as_str()) {
                TsUse::add(&mut self.words, f.ty.clone());
            }
            if f.list && NUMBERS.contains(&f.ty.as_str()) {
                TsUse::add(&mut self.arrays, format!("{}Array", f.ty.to_uppercase()));
            }
            if spec.kind(&f.ty)? == Kind::Extern {
                TsUse::add(&mut self.words, "u32".to_string());
            }
        }
        Ok(())
    }

    fn field_lines(&mut self, spec: &Spec, fields: &[Field]) -> Result<Vec<String>, String> {
        self.note(spec, fields)?;
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

/// The object literal that reads `fields` from a branch `b`.
fn ts_reads(spec: &Spec, fields: &[Field]) -> Result<String, String> {
    let reads = fields
        .iter()
        .enumerate()
        .map(|(i, f)| Ok(format!("{}: {}", f.name, ts_read(spec, f, i)?)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!("{{ {} }}", reads.join(", ")))
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
    let commands = spec.commands()?;
    let nested: Vec<&str> = spec
        .records
        .iter()
        .flat_map(|r| r.fields.iter())
        .chain(commands.iter().filter_map(|c| c.import.as_ref()).flatten())
        .map(|f| f.ty.as_str())
        .collect();
    for r in &spec.records {
        tail.push(String::new());
        if let Some(doc) = &r.doc {
            tail.extend(ts_doc(doc, ""));
        }
        tail.push(format!("export interface {} {{", r.name));
        tail.extend(uses.field_lines(spec, &r.fields)?);
        tail.extend(["}".to_string(), String::new()]);
        // A record no other reader reaches is read where it lies, by its name.
        tail.push(format!(
            "{}function read_{}(b: BranchCursor): {} {{",
            if nested.contains(&r.name.as_str()) {
                ""
            } else {
                "export "
            },
            snake(&r.name),
            r.name
        ));
        tail.push(format!("  return {};", ts_reads(spec, &r.fields)?));
        tail.push("}".into());
    }
    let mut readers: Vec<String> = Vec::new();
    for c in &commands {
        let name = pascal(&c.name);
        if let Some(import) = &c.import {
            tail.extend([
                String::new(),
                format!("/** Command.{}'s packet. */", c.name),
                format!("export interface {name} {{"),
            ]);
            tail.extend(uses.field_lines(spec, import)?);
            tail.push("}".to_string());
            if !import.is_empty() {
                readers.push(format!(
                    "  [Command.{}]: (b: BranchCursor): {name} => ({}),",
                    c.name,
                    ts_reads(spec, import)?
                ));
            }
        }
    }
    if !readers.is_empty() {
        tail.extend([
            String::new(),
            "/** Each import's packet, read from its branch, by its number. */".to_string(),
            "export const read = {".to_string(),
        ]);
        tail.extend(readers);
        tail.push("};".to_string());
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
    let xtp: Vec<&str> = ["BranchCursor", "NodeCursor"]
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
