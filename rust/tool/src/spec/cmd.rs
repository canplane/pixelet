// pxl-tool/spec/cmd.rs

//! The command tables, xpute's and a program's on top of it. Majors below
//! xpute's `program_major` are xpute's; a number is given out once across
//! both. The emitters
//! are cmd/ts.rs, cmd/rs.rs and cmd/cpp.rs.

mod cpp;
mod rs;
mod ts;

use serde::Deserialize;

use super::{pascal, read_as, Output};

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
    val: u32,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    pkt: Option<Vec<Field>>,
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
        self.records
            .iter()
            .find(|r| r.name == name)
            .expect("a record kind() found")
    }

    fn commands(&self) -> Result<Vec<&Member>, String> {
        let table = self
            .enums
            .iter()
            .find(|e| e.name == "Command")
            .ok_or("cmd.json has no Command")?;
        Ok(members(table).collect())
    }
}

fn members(e: &Enum) -> impl Iterator<Item = &Member> {
    e.entries.iter().filter_map(|m| match m {
        Entry::Member(m) => Some(m),
        Entry::Section(_) => None,
    })
}

fn literal(val: u32, format: &str) -> String {
    match format {
        "dec" => val.to_string(),
        "hex2" => format!("0x{val:02x}"),
        _ => format!("0x{val:04x}"),
    }
}

/// Every command is numbered by a major and a minor that cmd.json names. A
/// table's numbers are each given out once.
fn check_numbers(spec: &Spec, json: &str) -> Result<(), String> {
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        for (i, m) in all.iter().enumerate() {
            if let Some(other) = all[..i].iter().find(|o| o.val == m.val) {
                return Err(format!(
                    "{json}: {}'s {} and {} are both {}",
                    e.name,
                    m.name,
                    other.name,
                    literal(m.val, &e.format)
                ));
            }
        }
    }
    let mut cmds: Vec<&Member> = Vec::new();
    let mut minors: Vec<&Enum> = Vec::new();
    let mut majors: Vec<(u32, &str)> = Vec::new();
    for e in &spec.enums {
        for m in members(e) {
            if e.name == "Command" {
                cmds.push(m);
            } else if e.name == "Major" {
                majors.push((m.val, &m.name));
            } else if e.name.ends_with("Minor") {
                minors.push(e);
            }
        }
    }
    for c in cmds.into_iter().filter(|c| c.val != 0) {
        let major = majors
            .iter()
            .find(|(v, _)| *v == c.val >> 8)
            .map(|(_, n)| *n)
            .ok_or(format!("{json}: {} has no major", c.name))?;
        let table_name = format!("{}Minor", pascal(major));
        let minor = minors
            .iter()
            .find(|t| t.name == table_name)
            .and_then(|t| members(t).find(|m| m.val == c.val & 0xff));
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
        cpp: Some(("xpute/cpp/xpute/guest/abi/cmd.spec.hpp", "xpute::abi")),
    },
    Target {
        json: "spec/client/kernel/domain/control/cmd.json",
        ts: "ts/client/src/abi/cmd.spec.ts",
        ts_logical: "@pxl/client/abi/cmd.spec.ts",
        rs_dir: "rust/client/kernel/src/domain/control",
        rs_logical: "pxl-kernel/domain/control",
        number: "xpute_guest::abi::number",
        cpp: None,
    },
];

fn majors(spec: &Spec) -> Vec<(u32, &str)> {
    spec.enums
        .iter()
        .filter(|e| e.name == "Major")
        .flat_map(|e| members(e).map(|m| (m.val, m.name.as_str())))
        .collect()
}

/// A program's majors are past the base's `program_major`, and no command is named in both.
fn check_base(spec: &Spec, json: &str) -> Result<(), String> {
    let Some(base_path) = &spec.base else {
        return Ok(());
    };
    let base: Spec = read_as(base_path)?;
    let edge = base
        .program_major
        .ok_or(format!("{base_path}: no program_major"))?;
    for (val, name) in majors(&base) {
        if val >= edge {
            return Err(format!(
                "{base_path}: major {name} at {val:#x} is a program's"
            ));
        }
    }
    for (val, name) in majors(spec) {
        if val < edge {
            return Err(format!(
                "{json}: major {name} at {val:#x} is below {edge:#x}, xpute's"
            ));
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
        out.push(Output::new(t.ts, ts::emit(&spec, t)?));
        out.push(Output::new(
            &format!("{}/cmd.spec.rs", t.rs_dir),
            rs::emit(&spec, t)?,
        ));
        if let Some((path, namespace)) = t.cpp {
            out.push(Output::new(path, cpp::emit(&spec, t, path, namespace)?));
        }
    }
    Ok(out)
}
