// pxl-tool/spec/cmd/rs.rs

//! A command table as Rust: its enums, each packet's reader and each
//! signal's writer, for a Rust guest.

use super::{literal, members, Entry, Field, Kind, Member, Spec, Target};
use crate::spec;
use crate::spec::{file, pascal, rs_doc};

fn rs_borrows(spec: &Spec, fields: &[Field]) -> Result<bool, String> {
    for f in fields {
        let kind = spec.kind(&f.ty)?;
        if f.list
            || kind == Kind::Str
            || (kind == Kind::Record && rs_borrows(spec, &spec.record(&f.ty).fields)?)
        {
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
    spec.extern_of(ty)
        .map(|x| x.rs.clone())
        .unwrap_or_else(|| ty.to_string())
}

fn rs_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list && kind == Kind::Number {
        return Ok(format!("&'a [{}]", f.ty));
    }
    if f.list && kind == Kind::Str {
        return Ok("Strs<'a>".to_string());
    }
    if f.list || kind == Kind::Record {
        return Err(format!(
            "cmd.json: the kernel reads no {} ({})",
            if f.list { "list" } else { "record" },
            f.name
        ));
    }
    let one = match kind {
        Kind::Number => f.ty.clone(),
        Kind::Str => "&'a str".to_string(),
        Kind::Bool => "bool".to_string(),
        _ => rs_enum(spec, &f.ty),
    };
    Ok(if f.optional {
        format!("Option<{one}>")
    } else {
        one
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
        _ => {}
    }
    let (repr, unknown) = match spec.enums.iter().find(|e| e.name == f.ty) {
        Some(e) => (None, e.unknown.clone()),
        None => {
            let x = spec
                .extern_of(&f.ty)
                .ok_or(format!("cmd.json: no type {}", f.ty))?;
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
        if kind != Kind::Record {
            return Err(format!(
                "cmd.json: a list of {} ({}) is not written",
                f.ty, f.name
            ));
        }
        return Ok(format!(
            "&'a dyn Fn(&mut dyn FnMut(&{}))",
            rs_record(spec, &f.ty, "'_")?
        ));
    }
    if f.optional && kind == Kind::Str {
        return Err(format!(
            "cmd.json: an optional str ({}) is not written",
            f.name
        ));
    }
    let one = match kind {
        Kind::Number => f.ty.clone(),
        Kind::Bool => "bool".to_string(),
        Kind::Str => "&'a dyn Display".to_string(),
        Kind::Record => rs_record(spec, &f.ty, "'a")?,
        _ => rs_enum(spec, &f.ty),
    };
    Ok(if f.optional {
        format!("Option<{one}>")
    } else {
        one
    })
}

/// A list is walked twice, once to count, since a branch's table precedes its
/// children.
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

pub fn emit(spec: &Spec, t: &Target) -> Result<String, String> {
    let mut out = vec![
        format!("// {}/cmd.spec.rs", t.rs_logical),
        "//".into(),
        spec::generated(t.json, t.json),
        "".into(),
    ];
    out.extend(rs_doc(&spec.doc, "", "//!"));
    out.extend(
        [
            "",
            "// A list a record carries is a function handing out its items, whose",
            "// type is long to spell and is spelled once, here, for each. A table",
            "// with no signal or no enum read off the wire leaves a writer or a",
            "// number's check unused.",
            "#![allow(non_camel_case_types, clippy::type_complexity, clippy::absolute_paths, unused_imports, dead_code, reason = \"generated from the spec\")]",
            "",
            "use core::fmt::Display;",
            "",
            "use xpute_kit::status::error::MarshalError;",
        ]
        .map(String::from),
    );
    // A list of strings is read as xpute's `Strs`, named only where one is.
    let strs = spec
        .commands()?
        .iter()
        .flat_map(|c| c.pkt.iter().chain(c.signal.iter()).flatten())
        .any(|f| f.list && f.ty == "str");
    out.push(format!(
        "use xpute_kit::wire::xtp::{{PacketWriter, {}TreeReader}};",
        if strs { "Strs, " } else { "" }
    ));
    out.push(format!("use {}::{{finite, integer, unknown}};", t.number));
    for e in &spec.enums {
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
    for r in &spec.records {
        rs_writer(spec, &mut out, &r.name, r.doc.as_deref(), &r.fields)?;
    }
    for c in spec.commands()? {
        let name = pascal(&c.name);
        if let Some(pkt) = &c.pkt {
            let mut borrows = false;
            for f in pkt {
                borrows |= f.list || spec.kind(&f.ty)? == Kind::Str;
            }
            let lt = if borrows { "<'a>" } else { "" };
            out.extend([
                String::new(),
                format!("/// Command::{}'s packet.", c.name),
                "#[derive(Clone, Copy, Debug)]".to_string(),
                format!("pub struct {name}{lt} {{"),
            ]);
            for f in pkt {
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
            for (i, f) in pkt.iter().enumerate() {
                out.push(format!("            {}: {},", f.name, rs_read(spec, f, i)?));
            }
            out.extend([
                "        })".to_string(),
                "    }".to_string(),
                "}".to_string(),
            ]);
        }
        if let Some(signal) = &c.signal {
            let doc = [format!("Command::{}'s signal.", c.name)];
            rs_writer(spec, &mut out, &format!("{name}Signal"), Some(&doc), signal)?;
        }
    }
    Ok(file(&out))
}
