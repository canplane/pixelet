// xpute-tool/spec/cmd.rs

//! xpute's command table. The emitters are cmd/ts.rs, cmd/rs.rs and
//! cmd/cpp.rs.

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
/// table's numbers are each given out once, and no member takes a global's name.
fn check_numbers(spec: &Spec, json: &str) -> Result<(), String> {
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        // Each member is a top-level `export const` in TypeScript, which a
        // bundler that hoists it as `var` turns into the module's own `Object`,
        // undefined when its `Object.defineProperty` runs.
        if all.iter().any(|m| m.name == "Object") {
            return Err(format!(
                "{json}: {}'s member Object hides the global in its TypeScript module",
                e.name
            ));
        }
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
    // A table that names no majors numbers its commands as it likes.
    if majors.is_empty() {
        return Ok(());
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
    /// For a program's own table: the guest's `program` module, which each
    /// packet is read for (`FromPacket`).
    program: Option<&'static str>,
    /// For a guest that raises the table's events: the module whose `Event`
    /// each implements (`xpute_guest::ipc::door`).
    event: Option<&'static str>,
    /// For a program's own table: the host's port module, whose `Command` and
    /// `Event` each command's descriptor is, its fields by place.
    port: Option<&'static str>,
    cpp: Option<(&'static str, &'static str)>,
}

const TARGETS: [Target; 2] = [
    Target {
        json: "spec/abi/cmd.json",
        ts: "ts/host/src/abi/cmd.spec.ts",
        ts_logical: "@xpute/host/abi/cmd.spec.ts",
        rs: "rust/guest/src/abi/cmd.spec.rs",
        rs_logical: "xpute-guest/abi/cmd.spec.rs",
        number: "crate::abi::number",
        program: None,
        event: None,
        port: None,
        cpp: Some(("cpp/xpute/guest/abi/cmd.spec.hpp", "xpute::abi")),
    },
    // The example's own table, as a program's is generated.
    Target {
        json: "spec/example/cmd.json",
        ts: "ts/example/cmd.spec.ts",
        ts_logical: "ts/example/cmd.spec.ts",
        rs: "rust/example/src/cmd.spec.rs",
        rs_logical: "xpute-example/cmd.spec.rs",
        number: "xpute_guest::abi::number",
        program: Some("xpute_guest::program"),
        event: Some("xpute_guest::ipc::door"),
        port: Some("@xpute/host/port.ts"),
        cpp: Some(("cpp/example/cmd.spec.hpp", "example")),
    },
];

/// A command table and where its code goes, given by whoever owns it: xpute
/// reads it and writes nothing of the owner's own.
pub struct Table<'a> {
    /// Where the table is read from.
    pub json: &'a str,
    /// Where each output is written, its enums' modules in the folder beside
    /// the TypeScript.
    pub ts: &'a str,
    pub rs: &'a str,
    /// Each as the generated files' headers name it.
    pub json_shown: &'a str,
    pub ts_shown: &'a str,
    pub rs_shown: &'a str,
    /// Where the number checks a packet's reader calls are.
    pub number: &'a str,
    /// For a guest built with `program!`: its `program` module.
    pub program: Option<&'a str>,
    /// For a guest that raises the table's events: the module whose `Event`
    /// each implements (`xpute_guest::ipc::door`).
    pub event: Option<&'a str>,
    /// For a host that sends through a port: its port module.
    pub port: Option<&'a str>,
}

/// Where a program's C++ guest header is written (`cpp/xpute/guest/program.hpp`).
pub struct Cpp<'a> {
    pub path: &'a str,
    /// As the generated header names it.
    pub shown: &'a str,
    /// The namespace its enums and structs are declared in.
    pub namespace: &'a str,
}

/// `table`'s TypeScript and Rust.
pub fn table_outputs(table: &Table) -> Result<Vec<Output>, String> {
    outputs_of(table, None)
}

/// `table`'s TypeScript and Rust, and its C++ where `cpp_out` says.
fn outputs_of(table: &Table, cpp_out: Option<&Cpp>) -> Result<Vec<Output>, String> {
    let keep = |s: &str| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let spec: Spec = read_as(table.json)?;
    check_numbers(&spec, table.json_shown)?;
    let t = Target {
        json: keep(table.json_shown),
        ts: keep(table.ts),
        ts_logical: keep(table.ts_shown),
        rs: keep(table.rs),
        rs_logical: keep(table.rs_shown),
        number: keep(table.number),
        program: table.program.map(keep),
        event: table.event.map(keep),
        port: table.port.map(keep),
        cpp: None,
    };
    let mut out = ts::emit(&spec, &t)?;
    out.push(Output::new(t.rs, rs::emit(&spec, &t)?));
    if let Some(c) = cpp_out {
        out.push(Output::new(
            c.path,
            cpp::emit(&spec, &t, c.shown, c.namespace)?,
        ));
    }
    Ok(out)
}

/// A program's own table at `json`, its TypeScript at `ts`, its Rust at `rs`
/// and, where `cpp_out` says, its C++, for a guest built with
/// `xpute_guest::program!` or `XPUTE_PROGRAM` and a host that sends through
/// `@xpute/host`'s port; `shown` is each as the headers name it.
pub fn program_outputs(
    json: &str,
    ts: &str,
    rs: &str,
    shown: [&str; 3],
    cpp_out: Option<&Cpp>,
) -> Result<Vec<Output>, String> {
    let table = Table {
        json,
        ts,
        rs,
        json_shown: shown[0],
        ts_shown: shown[1],
        rs_shown: shown[2],
        number: "xpute_guest::abi::number",
        program: Some("xpute_guest::program"),
        event: Some("xpute_guest::ipc::door"),
        port: Some("@xpute/host/port.ts"),
    };
    outputs_of(&table, cpp_out)
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    for t in &TARGETS {
        let spec: Spec = read_as(t.json)?;
        check_numbers(&spec, t.json)?;
        out.extend(ts::emit(&spec, t)?);
        out.push(Output::new(t.rs, rs::emit(&spec, t)?));
        if let Some((path, namespace)) = t.cpp {
            out.push(Output::new(path, cpp::emit(&spec, t, path, namespace)?));
        }
    }
    Ok(out)
}
