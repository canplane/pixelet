// xpute-tool/spec/cmd.rs

//! This generates the code for a protocol over xpute's rings, xpute's own or a
//! program's: its commands and the types their packets are made of. A protocol
//! names its commands from the core's side, as a WIT world does: an `export` is
//! a command the core implements and the runtime calls, and an `import` is one
//! the runtime implements and the core calls. cmd/ts.rs, cmd/rs.rs and
//! cmd/cpp.rs write the messages in each language, and cmd/service.rs writes
//! each end's binding onto xpute's runtime and core.

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

/// A `Member` is a member of an enum. Each command becomes a member of the
/// `Command` enum, made from its protocol's entry (`Cmd`).
#[derive(Deserialize)]
struct Member {
    name: String,
    val: u32,
    #[serde(rename = "$doc")]
    doc: Option<Vec<String>>,
    #[serde(skip)]
    pkt: Option<Vec<Field>>,
    #[serde(skip)]
    import: Option<Vec<Field>>,
    /// The fields of an `import` that say where the memory it names lies.
    #[serde(skip)]
    mem: Option<[String; 2]>,
    /// This is the packet of an `import`'s completion, which the runtime writes
    /// and the core reads.
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

/// A `Cmd` is a command as its protocol lists it. With `call`, the sender waits
/// for the answer: the runtime calls an `export` and waits, or answers an
/// `import` before the core's call returns. With `ackreq`, the command is
/// posted with ACKREQ and answered later; the runtime completes such an
/// `import` on a later turn with a frame of the same tag flagged RES, whose
/// packet is `reply`. `mem` names the two fields of an `import` that give where
/// a region of the core's memory is and how long it is.
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
    mem: Option<[String; 2]>,
}

/// A `Doc` is a protocol: the commands the runtime sends the core (`export`)
/// and those the core posts to the runtime (`import`). `name` serves only the
/// generated code's names. Nothing on the rings reads the protocol; a runtime
/// routes a frame by its command's major alone.
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
    /// Where a program's protocol code goes, relative to the JSON's own
    /// directory.
    gen: Option<Gen>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Gen {
    pub(crate) ts: String,
    pub(crate) rs: String,
    /// Enums that a module below the protocol's own code reads. They are
    /// written into a file of their own, where that module can name them.
    pub(crate) enums: Option<Apart>,
}

/// The enums `names`, written apart into `rs`, a file relative to the JSON's
/// own directory that is the Rust module `path`. The protocol's Rust names
/// these enums through that module; its TypeScript still holds all of them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Apart {
    pub(crate) rs: String,
    pub(crate) path: String,
    pub(crate) names: Vec<String>,
}

struct Spec {
    doc: Vec<String>,
    externs: Vec<Extern>,
    enums: Vec<Enum>,
    records: Vec<Record>,
    protocol: Protocol,
    gen: Option<Gen>,
}

/// A `Protocol` is a protocol as the emitters read it.
struct Protocol {
    name: String,
    /// Each export's name, whether it is a call, and whether it is posted with
    /// ACKREQ and answered later.
    exports: Vec<(String, bool, bool)>,
    imports: Vec<(String, bool)>,
    /// Each import that names the core's memory, and its two fields.
    mem: Vec<(String, [String; 2])>,
}

/// Reads `json`'s protocol and makes its commands members of `Command`. An
/// `export` carries a packet the core reads, and an `import` one it writes.
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
        mem: Vec::new(),
    };
    for (dir, cmds) in [("export", d.export), ("import", d.import)] {
        if cmds.is_empty() {
            continue;
        }
        entries.push(Entry::Section(Section {
            section: format!("{}, {dir}", d.name),
        }));
        for c in cmds {
            if c.ackreq && c.call {
                return Err(format!("{shown}: {} is answered later, so no call", c.name));
            }
            if c.reply.is_some() && dir == "export" {
                return Err(format!(
                    "{shown}: {} is an export, answered by its result alone",
                    c.name
                ));
            }
            if c.reply.is_some() && !c.ackreq {
                return Err(format!("{shown}: {} has a reply and no ackreq", c.name));
            }
            let (pkt, import) = if dir == "export" {
                (c.pkt, None)
            } else {
                (None, Some(c.pkt.unwrap_or_default()))
            };
            let mem = c.mem.clone();
            if let Some(m) = c.mem {
                let has = |n: &str| {
                    import
                        .as_ref()
                        .is_some_and(|p| p.iter().any(|f| f.name == n))
                };
                if !has(&m[0]) || !has(&m[1]) {
                    return Err(format!(
                        "{shown}: {}'s mem names no import's fields",
                        c.name
                    ));
                }
                of.mem.push((c.name.clone(), m));
            }
            if dir == "export" {
                of.exports.push((c.name.clone(), c.call, c.ackreq));
            } else {
                of.imports.push((c.name.clone(), c.call));
            }
            entries.push(Entry::Member(Member {
                name: c.name,
                val: c.val,
                doc: c.doc,
                pkt,
                import,
                mem,
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
            return Err(format!("protocol: no type {ty}"));
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
            .ok_or("a protocol has no Command")?;
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

/// Checks that every command's major and minor are members its protocol names,
/// that each number in a table is given out once, and that no member takes the
/// name of a global.
fn check_numbers(spec: &Spec, json: &str) -> Result<(), String> {
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        // In TypeScript each member is a top-level `export const`. A bundler
        // that hoists it as a `var` would make a member named Object shadow
        // the global in its module, and that `Object` is still undefined when
        // the module's `Object.defineProperty` runs.
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
    /// The module that holds the number checks a packet's reader calls.
    number: &'static str,
    /// For a core that raises the table's imports, the module whose `Import`
    /// trait each import implements (`xpute_core::ipc::door`).
    import: Option<&'static str>,
    cpp: Option<(&'static str, &'static str)>,
    /// For xpute's own protocols, the path inside the runtime's own package
    /// from which its TypeScript binding imports the runtime's types.
    runtime_ts: Option<&'static str>,
}

const TARGETS: [Target; 3] = [
    Target {
        json: "spec/abi/sys.json",
        ts: "ts/runtime/src/abi/sys.spec.ts",
        ts_logical: "@xpute/runtime/abi/sys.spec.ts",
        rs: "rust/core/src/abi/sys.spec.rs",
        rs_logical: "xpute-core/abi/sys.spec.rs",
        number: "crate::abi::number",
        import: Some("crate::ipc::door"),
        cpp: Some(("cpp/xpute/core/abi/sys.spec.hpp", "xpute::abi::sys")),
        runtime_ts: Some("../mod.ts"),
    },
    Target {
        json: "spec/abi/gpu.json",
        ts: "ts/runtime/src/abi/gpu.spec.ts",
        ts_logical: "@xpute/runtime/abi/gpu.spec.ts",
        rs: "rust/core/src/abi/gpu.spec.rs",
        rs_logical: "xpute-core/abi/gpu.spec.rs",
        number: "crate::abi::number",
        import: Some("crate::ipc::door"),
        cpp: Some(("cpp/xpute/core/abi/gpu.spec.hpp", "xpute::abi::gpu")),
        runtime_ts: Some("../mod.ts"),
    },
    Target {
        json: "spec/abi/fs.json",
        ts: "ts/runtime/src/abi/fs.spec.ts",
        ts_logical: "@xpute/runtime/abi/fs.spec.ts",
        rs: "rust/core/src/abi/fs.spec.rs",
        rs_logical: "xpute-core/abi/fs.spec.rs",
        number: "crate::abi::number",
        import: Some("crate::ipc::door"),
        cpp: Some(("cpp/xpute/core/abi/fs.spec.hpp", "xpute::abi::fs")),
        runtime_ts: Some("../mod.ts"),
    },
];

/// This is the first major a program's commands may take; the majors below it
/// are xpute's own. It must equal `PROGRAM_MAJOR` of spec/ipc/frame.json.
const PROGRAM_MAJOR: u32 = 0x10;

/// Checks that no two commands across `specs` share a number.
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

/// Generates a program's protocols, each into the files its `gen` names. Every
/// `json`, and every path in the outputs, is relative to `root`. A command
/// under xpute's majors is refused, and so is a number that two protocols both
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
        number: "xpute_core::abi::number",
        import: Some("xpute_core::ipc::door"),
        cpp: None,
        runtime_ts: None,
    };
    let mut out = ts::emit(spec, &t)?;
    let (imports, body) = service::ts(spec, "@xpute/runtime")?;
    if let Some(first) = out.first_mut() {
        first.text = splice_ts(&first.text, &imports, &body);
    }
    let apart = gen.enums.as_ref();
    if let Some(a) = apart {
        if let Some(n) = a
            .names
            .iter()
            .find(|n| !spec.enums.iter().any(|e| e.name == **n))
        {
            return Err(format!(
                "{json}: gen.enums names {n}, which it does not declare"
            ));
        }
        let at = beside(json, &a.rs)?;
        out.push(Output::new(&at, rs::emit_apart(spec, &t, &at, a)));
    }
    out.push(Output::new(
        t.rs,
        rs::emit(spec, &t, apart)? + &service::rs(spec)?,
    ));
    Ok(out)
}

/// Adds to the module's import from `from` each of `names` the body spells,
/// making that import where the module has none.
fn import_spelled(lines: &mut Vec<String>, names: Vec<&str>, from: &str) {
    if names.is_empty() {
        return;
    }
    let tail = format!(" }} from \"{from}\";");
    let found = lines
        .iter()
        .position(|l| l.starts_with("import type { ") && l.ends_with(&tail));
    let mut all: Vec<String> = found.map_or(Vec::new(), |i| {
        lines[i]["import type { ".len()..lines[i].len() - tail.len()]
            .split(", ")
            .map(str::to_string)
            .collect()
    });
    for n in names {
        if !all.iter().any(|a| a == n) {
            all.push(n.to_string());
        }
    }
    all.sort();
    let line = format!("import type {{ {}{tail}", all.join(", "));
    match found {
        Some(i) => lines[i] = line,
        None => {
            let last = lines
                .iter()
                .rposition(|l| l.starts_with("import "))
                .map_or(0, |i| i + 1);
            lines.insert(last, line);
        }
    }
}

/// Splices `imports` in after the module's last import and `body` at its end,
/// and adds the word and array types that `body` spells to the module's imports
/// of them.
fn splice_ts(text: &str, imports: &[String], body: &[String]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let body_text = body.join("\n");
    let words = ["f64", "i32", "u32", "u8"]
        .into_iter()
        .filter(|w| body_text.contains(&format!(": {w}")) || body_text.contains(&format!("<{w}>")))
        .collect();
    import_spelled(&mut lines, words, "@xpute/kit/abi/word.ts");
    let arrays = ["U32Array"]
        .into_iter()
        .filter(|a| body_text.contains(a))
        .collect();
    import_spelled(&mut lines, arrays, "@xpute/kit/abi/array.ts");
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
        if let (Some(runtime), Some(first)) = (t.runtime_ts, ts.first_mut()) {
            let (imports, body) = service::ts(spec, runtime)?;
            first.text = splice_ts(&first.text, &imports, &body);
        }
        out.extend(ts);
        out.push(Output::new(t.rs, rs::emit(spec, t, None)?));
        if let Some((path, namespace)) = t.cpp {
            out.push(Output::new(path, cpp::emit(spec, t, path, namespace)?));
        }
    }
    Ok(out)
}
