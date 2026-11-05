// xpute-tool/spec/cmd.rs

//! A protocol over xpute's rings, xpute's own or a program's: its commands,
//! as a WIT world names them from the guest's side, `export` what the guest
//! implements and the host calls and `import` what the host implements and
//! the guest calls, and the types their packets are made of. The messages' emitters are cmd/ts.rs, cmd/rs.rs and
//! cmd/cpp.rs; cmd/service.rs writes each end's binding over xpute's ends.

mod cpp;
mod rs;
mod service;
mod ts;

use std::path::Path;

use serde::Deserialize;

use super::{beside, pascal, read_as, Output};

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

/// An enum's member; a command is one of `Command`'s, made from its
/// protocol's entry (`Cmd`).
#[derive(Deserialize)]
struct Member {
    name: String,
    val: u32,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    #[serde(skip)]
    pkt: Option<Vec<Field>>,
    #[serde(skip)]
    event: Option<Vec<Field>>,
    /// An `import`'s completion's packet, which the host writes and the guest reads.
    #[serde(skip)]
    reply: Option<Vec<Field>>,
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

/// A command as its protocol lists it. `call`: the sender waits for the answer
/// (an `export` the host calls, an `import` the host answers before it returns).
/// `ackreq`: an `import` the host completes on a later turn, a frame of its tag
/// flagged RES, its packet `reply`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cmd {
    name: String,
    val: u32,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    pkt: Option<Vec<Field>>,
    #[serde(default)]
    call: bool,
    #[serde(default)]
    ackreq: bool,
    reply: Option<Vec<Field>>,
}

/// A protocol: the commands the host sends the guest (`export`) and those the
/// guest posts to the host (`import`). `name` is the program's, for the generated
/// code's names only. The bus reads none of it; a host routes a frame by its
/// command's major.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Doc {
    #[serde(rename = "$doc")]
    doc: Vec<String>,
    name: String,
    #[serde(default)]
    export: Vec<Cmd>,
    #[serde(default)]
    import: Vec<Cmd>,
    #[serde(rename = "extern", default)]
    externs: Vec<Extern>,
    #[serde(default)]
    enums: Vec<Enum>,
    #[serde(default)]
    records: Vec<Record>,
    /// Where a program's code goes, from the JSON's own directory.
    gen: Option<Gen>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Gen {
    pub(crate) ts: String,
    pub(crate) rs: String,
}

struct Spec {
    doc: Vec<String>,
    externs: Vec<Extern>,
    enums: Vec<Enum>,
    records: Vec<Record>,
    /// Each command `Command`'s member named in `export` or `import`.
    protocol: Protocol,
    gen: Option<Gen>,
}

/// A protocol as the emitters read it.
struct Protocol {
    name: String,
    exports: Vec<(String, bool)>,
    imports: Vec<(String, bool)>,
}

/// `json`'s protocol, its commands made `Command`'s members, each `export` a
/// packet the guest reads and each `import` one it writes.
fn read_spec(json: &str, shown: &str) -> Result<Spec, String> {
    let d: Doc = read_as(json)?;
    if d.enums.iter().any(|e| e.name == "Command") {
        return Err(format!(
            "{shown}: Command is made from export and import, not declared"
        ));
    }
    let mut entries = Vec::new();
    let mut of = Protocol {
        name: d.name.clone(),
        exports: Vec::new(),
        imports: Vec::new(),
    };
    for (dir, cmds) in [("export", d.export), ("import", d.import)] {
        if cmds.is_empty() {
            continue;
        }
        entries.push(Entry::Section(Section {
            section: format!("{}, {dir}", d.name),
        }));
        for c in cmds {
            if (c.ackreq || c.reply.is_some()) && (dir == "export" || c.call) {
                return Err(format!(
                    "{shown}: {} is completed later, so an import and no call",
                    c.name
                ));
            }
            if c.reply.is_some() && !c.ackreq {
                return Err(format!("{shown}: {} has a reply and no ackreq", c.name));
            }
            let (pkt, event) = if dir == "export" {
                (c.pkt, None)
            } else {
                (None, Some(c.pkt.unwrap_or_default()))
            };
            if dir == "export" {
                of.exports.push((c.name.clone(), c.call));
            } else {
                of.imports.push((c.name.clone(), c.call));
            }
            entries.push(Entry::Member(Member {
                name: c.name,
                val: c.val,
                doc: c.doc,
                pkt,
                event,
                reply: c.reply,
            }));
        }
    }
    let mut enums = d.enums;
    enums.push(Enum {
        name: "Command".into(),
        doc: Some(vec!["Every command, by the side that sends it.".into()]),
        format: "hex4".into(),
        default: None,
        unknown: None,
        entries,
    });
    Ok(Spec {
        doc: d.doc,
        externs: d.externs,
        enums,
        records: d.records,
        protocol: of,
        gen: d.gen,
    })
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
    /// For a guest that raises the table's events: the module whose `Event`
    /// each implements (`xpute_guest::ipc::door`).
    event: Option<&'static str>,
    cpp: Option<(&'static str, &'static str)>,
    /// For xpute's own protocols, where the host's TypeScript binding takes
    /// the host's types from inside its own package.
    host_ts: Option<&'static str>,
}

const TARGETS: [Target; 2] = [
    Target {
        json: "spec/abi/cmd.json",
        ts: "ts/host/src/abi/cmd.spec.ts",
        ts_logical: "@xpute/host/abi/cmd.spec.ts",
        rs: "rust/guest/src/abi/cmd.spec.rs",
        rs_logical: "xpute-guest/abi/cmd.spec.rs",
        number: "crate::abi::number",
        event: Some("crate::ipc::door"),
        cpp: Some(("cpp/xpute/guest/abi/cmd.spec.hpp", "xpute::abi")),
        host_ts: Some("../mod.ts"),
    },
    Target {
        json: "spec/abi/fs.json",
        ts: "ts/host/src/abi/fs.spec.ts",
        ts_logical: "@xpute/host/abi/fs.spec.ts",
        rs: "rust/guest/src/abi/fs.spec.rs",
        rs_logical: "xpute-guest/abi/fs.spec.rs",
        number: "crate::abi::number",
        event: Some("crate::ipc::door"),
        cpp: Some(("cpp/xpute/guest/abi/fs.spec.hpp", "xpute::abi::fs")),
        host_ts: Some("../mod.ts"),
    },
];

/// The first major a program's commands take; those below it are xpute's
/// own (spec/ipc/frame.json `PROGRAM_MAJOR`).
const PROGRAM_MAJOR: u32 = 0x10;

/// Each command's number given once across `specs`, each read from its JSON.
fn check_once(specs: &[(&Spec, &str)]) -> Result<(), String> {
    let mut seen: Vec<(u32, &str, &str)> = Vec::new();
    for (spec, json) in specs {
        for c in spec.commands()? {
            if let Some((_, name, at)) = seen.iter().find(|(v, ..)| *v == c.val) {
                return Err(format!(
                    "{json}: {} is 0x{:04x}, which {at} gives {name}",
                    c.name, c.val
                ));
            }
            seen.push((c.val, &c.name, json));
        }
    }
    Ok(())
}

/// A program's protocols, each `json` from `root`, and each one's code where
/// its `gen` says, each path in the outputs from `root` as its `json` is. A
/// command under xpute's majors is refused, and so is a number two of them
/// give.
pub fn protocol_outputs(root: &Path, jsons: &[&str]) -> Result<Vec<Output>, String> {
    let mut specs = Vec::new();
    for json in jsons {
        let spec = read_spec(&root.join(json).to_string_lossy(), json)?;
        check_numbers(&spec, json)?;
        if let Some(c) = spec
            .commands()?
            .into_iter()
            .find(|c| c.val >> 8 < PROGRAM_MAJOR)
        {
            return Err(format!(
                "{json}: {} is 0x{:04x}, under major 0x{PROGRAM_MAJOR:02x}, among xpute's own",
                c.name, c.val
            ));
        }
        specs.push((spec, *json));
    }
    let read: Vec<(&Spec, &str)> = specs.iter().map(|(s, j)| (s, *j)).collect();
    check_once(&read)?;
    let mut out = Vec::new();
    for (spec, json) in &specs {
        out.extend(one_protocol(spec, json)?);
    }
    Ok(out)
}

fn one_protocol(spec: &Spec, json: &str) -> Result<Vec<Output>, String> {
    let keep = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    let gen = spec
        .gen
        .as_ref()
        .ok_or(format!("{json}: says nowhere its code goes (gen)"))?;
    let (ts_at, rs_at) = (keep(beside(json, &gen.ts)?), keep(beside(json, &gen.rs)?));
    let t = Target {
        json: keep(json.to_string()),
        ts: ts_at,
        ts_logical: ts_at,
        rs: rs_at,
        rs_logical: rs_at,
        number: "xpute_guest::abi::number",
        event: Some("xpute_guest::ipc::door"),
        cpp: None,
        host_ts: None,
    };
    let mut out = ts::emit(spec, &t)?;
    let (imports, body) = service::ts(spec, "@xpute/host")?;
    if let Some(first) = out.first_mut() {
        first.text = splice_ts(&first.text, &imports, &body);
    }
    out.push(Output::new(t.rs, rs::emit(spec, &t)? + &service::rs(spec)?));
    Ok(out)
}

/// `imports` after the module's last import, `body` at its end, and the word
/// types the body spells added to the words' import.
fn splice_ts(text: &str, imports: &[String], body: &[String]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let body_text = body.join("\n");
    for l in lines.iter_mut() {
        if let Some(names) = l
            .strip_prefix("import type { ")
            .and_then(|r| r.strip_suffix(" } from \"@xpute/kit/abi/word.ts\";"))
        {
            let mut all: Vec<String> = names.split(", ").map(str::to_string).collect();
            for w in ["f64", "i32", "u32", "u8"] {
                if body_text.contains(&format!(": {w}")) && !all.iter().any(|a| a == w) {
                    all.push(w.to_string());
                }
            }
            all.sort();
            *l = format!(
                "import type {{ {} }} from \"@xpute/kit/abi/word.ts\";",
                all.join(", ")
            );
        }
    }
    let last = lines
        .iter()
        .rposition(|l| l.starts_with("import "))
        .map_or(0, |i| i + 1);
    lines.splice(last..last, imports.iter().cloned());
    lines.extend(body.iter().cloned());
    lines.join("\n") + "\n"
}

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    let mut specs = Vec::new();
    for t in &TARGETS {
        let spec = read_spec(t.json, t.json)?;
        check_numbers(&spec, t.json)?;
        if let Some(c) = spec
            .commands()?
            .into_iter()
            .find(|c| c.val >> 8 >= PROGRAM_MAJOR)
        {
            return Err(format!(
                "{}: {} is 0x{:04x}, past xpute's majors",
                t.json, c.name, c.val
            ));
        }
        specs.push((spec, t));
    }
    let read: Vec<(&Spec, &str)> = specs.iter().map(|(s, t)| (s, t.json)).collect();
    check_once(&read)?;
    for (spec, t) in &specs {
        let mut ts = ts::emit(spec, t)?;
        if let (Some(host), Some(first)) = (t.host_ts, ts.first_mut()) {
            let (imports, body) = service::ts(spec, host)?;
            first.text = splice_ts(&first.text, &imports, &body);
        }
        out.extend(ts);
        out.push(Output::new(t.rs, rs::emit(spec, t)?));
        if let Some((path, namespace)) = t.cpp {
            out.push(Output::new(path, cpp::emit(spec, t, path, namespace)?));
        }
    }
    Ok(out)
}
