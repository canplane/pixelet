// pxl-tool/spec/xpute.rs

//! xpute's errno. The numbers cross the ring, so every side must agree.

use serde::Deserialize;

use super::{file, read_as, rs_doc, ts_doc, Output};

const JSON: &str = "spec/xpute/status/errno.json";

#[derive(Deserialize)]
struct ErrnoMember {
    name: String,
    value: i32,
    doc: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ErrnoEntry {
    Section { section: String },
    Member(ErrnoMember),
}

#[derive(Deserialize)]
struct ErrnoSpec {
    doc: Vec<String>,
    entries: Vec<ErrnoEntry>,
}

pub fn errno_outputs() -> Result<Vec<Output>, String> {
    let spec: ErrnoSpec = read_as(JSON)?;
    let mut head = vec!["//".to_string(), super::generated(JSON, JSON), "//".into()];
    head.extend(spec.doc.iter().map(|l| format!("// {l}")));

    let mut ts = vec!["// @xpute/core/status/errno.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    ts.extend(["".to_string(), "export const enum Errno {".into()]);
    let mut rs = vec!["// xpute-core/status/errno.spec.rs".to_string()];
    rs.extend(head.iter().cloned());
    rs.extend(["", "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]", "#[repr(i32)]", "pub enum Errno {"].map(String::from));
    let mut run: Vec<&ErrnoMember> = Vec::new();
    let flush = |run: &mut Vec<&ErrnoMember>, rs: &mut Vec<String>| {
        let width = run.iter().map(|e| format!("{} = {},", e.name, e.value).len()).max().unwrap_or(0);
        for e in run.iter() {
            rs.push(format!("    {:<width$} // {}", format!("{} = {},", e.name, e.value), e.doc));
        }
        run.clear();
    };
    for e in &spec.entries {
        match e {
            ErrnoEntry::Section { section } => {
                flush(&mut run, &mut rs);
                rs.extend([String::new(), format!("    // ---- {section} ----")]);
                ts.extend([String::new(), format!("  // ---- {section} ----"), String::new()]);
            }
            ErrnoEntry::Member(m) => {
                run.push(m);
                ts.push(format!("  {} = {}, // {}", m.name, m.value, m.doc));
            }
        }
    }
    flush(&mut run, &mut rs);
    rs.extend(["}".to_string(), String::new()]);
    ts.extend(["}".to_string(), String::new()]);

    let tail_doc = [
        "Conventions:",
        "- errno == Errno::OK  -> success",
        "- errno != Errno::OK  -> failure",
        "- data == None        -> no payload",
    ];
    rs.push("// Result pair: (errno, data)".into());
    rs.push("//".into());
    rs.extend(tail_doc.iter().map(|l| format!("// {l}")));
    rs.extend(
        [
            "//",
            "// `None` is the \"no value / no payload\" sentinel: the empty result slot,",
            "// nothing about pointers.",
            "pub type ErrnoResult<T> = (Errno, Option<T>);",
        ]
        .map(String::from),
    );
    ts.extend(
        [
            "// Result pair: [errno, data]",
            "//",
            "// Conventions:",
            "// - errno == Errno.OK  -> success",
            "// - errno != Errno.OK  -> failure",
            "// - data === null      -> no payload",
            "//",
            "// `null` is used as the canonical \"no value / no payload\" sentinel.",
            "// It does not imply pointer semantics; it is just the empty result slot.",
            "export type ErrnoResult<T> = [Errno, T | null];",
            "",
            "/** An errno as a reader is shown it: its name and what it means. The",
            " * strings are for a person to read; an error carries the number alone. */",
            "export function strerror(errno: Errno): string {",
            "  return STRERROR[errno] ?? `errno ${errno}`;",
            "}",
            "",
            "const STRERROR: Record<number, string> = {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            ts.push(format!("  [{}]: {},", m.value, super::json_str(&format!("{}: {}", m.name, m.doc))));
        }
    }
    ts.push("};".into());
    // Lower case, as std::errc: EINVAL may be a <cerrno> macro already included.
    let mut cpp = vec!["// cpp/xpute/core/status/errno.spec.hpp".to_string()];
    cpp.extend(head.iter().cloned());
    cpp.extend(
        [
            "",
            "#pragma once",
            "",
            "#include <cstdint>",
            "#include <optional>",
            "#include <string_view>",
            "#include <utility>",
            "",
            "namespace xpute {",
            "",
            "enum class Errno : std::int32_t {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        match e {
            ErrnoEntry::Section { section } => cpp.extend([String::new(), format!("    // ---- {section} ----")]),
            ErrnoEntry::Member(m) => cpp.push(format!("    {} = {}, // {}", m.name.to_lowercase(), m.value, m.doc)),
        }
    }
    cpp.extend(["};", ""].map(String::from));
    cpp.extend(
        [
            "// An errno's name, as Rust's Display prints an error's.",
            "constexpr std::string_view errno_name(Errno e) noexcept {",
            "    switch (e) {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            cpp.push(format!("    case Errno::{}: return \"{}\";", m.name.to_lowercase(), m.name));
        }
    }
    cpp.extend(
        [
            "    }",
            "    return \"errno\";",
            "}",
            "",
            "// A result as a pair, as Rust's ErrnoResult is: the errno, and the value",
            "// where there is one.",
            "template <class T> using ErrnoResult = std::pair<Errno, std::optional<T>>;",
            "",
            "} // namespace xpute",
        ]
        .map(String::from),
    );
    Ok(vec![
        Output::new("packages/xpute/core/src/status/errno.spec.ts", file(&ts)),
        Output::new("crates/xpute/core/src/status/errno.spec.rs", file(&rs)),
        Output::new("cpp/xpute/core/status/errno.spec.hpp", file(&cpp)),
    ])
}

// ---- the runtime's fetch failures ----
//
// Errno-shaped: a closed vocabulary both ends read by number, where the
// number is the name and a retired one leaves its gap. Unlike errno each
// entry carries a paragraph, a section groups a range, and the guest wants
// a short `&'static str` per entry for its own report.

#[derive(Deserialize)]
struct VocabMember {
    name: String,
    value: String,
    str: String,
    #[serde(default)]
    default: bool,
    doc: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VocabEntry {
    Section { section: String },
    Retired { retired: Vec<String> },
    Member(VocabMember),
}

#[derive(Deserialize)]
struct VocabSpec {
    doc: Vec<String>,
    name: String,
    repr: String,
    entries: Vec<VocabEntry>,
    as_str: Vec<String>,
}

pub fn fetch_outputs() -> Result<Vec<Output>, String> {
    let spec: VocabSpec = read_as("spec/xpute/abi/fetch.json")?;
    let head = ["//".to_string(), "// GENERATED by pxl-tool/bin/spec.rs from fetch.json — do not edit.".into(), "".into()];
    let members: Vec<&VocabMember> = spec
        .entries
        .iter()
        .filter_map(|e| match e {
            VocabEntry::Member(m) => Some(m),
            _ => None,
        })
        .collect();
    let name = &spec.name;
    let repr = &spec.repr;

    let mut ts = vec!["// @xpute/runtime/abi/fetch.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    ts.extend(ts_doc(&spec.doc, ""));
    ts.push(format!("export const enum {name} {{"));
    let mut rs = vec!["// xpute-runtime/abi/fetch.spec.rs".to_string()];
    rs.extend(head.iter().cloned());
    rs.extend(rs_doc(&spec.doc, "", "///"));
    rs.extend([
        "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]".to_string(),
        format!("#[repr({repr})]"),
        format!("pub enum {name} {{"),
    ]);
    for e in &spec.entries {
        match e {
            VocabEntry::Section { section } => {
                ts.extend([String::new(), format!("  // ---- {section} ----")]);
                rs.extend([String::new(), format!("    // ---- {section} ----")]);
            }
            VocabEntry::Retired { retired } => rs.extend(retired.iter().map(|l| format!("    // {l}"))),
            VocabEntry::Member(m) => {
                ts.extend(ts_doc(&m.doc, "  "));
                ts.push(format!("  {} = {},", m.name, m.value));
                rs.extend(rs_doc(&m.doc, "    ", "///"));
                if m.default {
                    rs.push("    #[default]".into());
                }
                rs.push(format!("    {} = {},", m.name, m.value));
            }
        }
    }
    ts.extend(["}".to_string(), String::new()]);
    rs.extend([
        "}".to_string(),
        String::new(),
        format!("impl {name} {{"),
        format!(
            "    pub const ALL: [{name}; {}] = [{}];",
            members.len(),
            members.iter().map(|m| format!("{name}::{}", m.name)).collect::<Vec<_>>().join(", ")
        ),
        String::new(),
        "    /// The reason a number names, or none for a number nobody gave out.".to_string(),
        format!("    pub fn of(number: {repr}) -> Option<{name}> {{"),
        format!("        {name}::ALL.into_iter().find(|f| *f as {repr} == number)"),
        "    }".to_string(),
        String::new(),
    ]);
    rs.extend(rs_doc(&spec.as_str, "    ", "///"));
    rs.extend(["    pub fn as_str(self) -> &'static str {".to_string(), "        match self {".to_string()]);
    for m in &members {
        rs.push(format!("            {name}::{} => \"{}\",", m.name, m.str));
    }
    rs.extend(["        }", "    }", "}", ""].map(String::from));
    Ok(vec![
        Output::new("packages/xpute/runtime/src/abi/fetch.spec.ts", ts.join("\n")),
        Output::new("crates/xpute/runtime/src/abi/fetch.spec.rs", rs.join("\n")),
    ])
}
