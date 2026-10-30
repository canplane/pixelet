// pxl-tool/spec/cmd.rs

//! pixelet's command table. The emitters are cmd/ts.rs and cmd/rs.rs.

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
    event: Option<Vec<Field>>,
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
    rs: &'static str,
    rs_logical: &'static str,
    /// Where the number checks a packet's reader calls are.
    number: &'static str,
}

const TARGET: Target = Target {
    json: "spec/client/kernel/domain/control/cmd.json",
    ts: "ts/client/src/abi/cmd.spec.ts",
    ts_logical: "@pxl/client/abi/cmd.spec.ts",
    rs: "rust/client/kernel/src/domain/control/cmd.spec.rs",
    rs_logical: "pxl-kernel/domain/control/cmd.spec.rs",
    number: "xpute_guest::abi::number",
};

pub fn outputs() -> Result<Vec<Output>, String> {
    let t = &TARGET;
    let spec: Spec = read_as(t.json)?;
    check_numbers(&spec, t.json)?;
    let mut out = ts::emit(&spec, t)?;
    out.push(Output::new(t.rs, rs::emit(&spec, t)?));
    Ok(out)
}
