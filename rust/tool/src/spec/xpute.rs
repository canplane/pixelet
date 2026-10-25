// pxl-tool/spec/xpute.rs

//! xpute's errno. The numbers cross the ring, so every side must agree.

use serde::Deserialize;

use super::{file, read_as, Output};

const JSON: &str = "xpute/spec/status/errno.json";

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

    let mut ts = vec!["// @xpute/kit/status/errno.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    ts.extend(["".to_string(), "export const enum Errno {".into()]);
    let mut rs = vec!["// xpute-kit/status/errno.spec.rs".to_string()];
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
    // A code from across the ring, read once: a number no member has is the sender's error.
    rs.extend(
        [
            "impl Errno {",
            "    /// The member `code` is, None for a number no member has.",
            "    pub const fn of(code: i32) -> Option<Errno> {",
            "        match code {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            rs.push(format!("            {} => Some(Errno::{}),", m.value, m.name));
        }
    }
    rs.extend(["            _ => None,", "        }", "    }", "}", ""].map(String::from));
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
            "export function strerror(errnum: Errno): string {",
            "  return STRERROR[errnum] ?? `errno ${errnum}`;",
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
    ts.extend(
        [
            "",
            "/** The member `code` is, null for a number no member has. */",
            "export function errno_of(code: number): Errno | null {",
            "  return Object.hasOwn(STRERROR, code) ? (code as Errno) : null;",
            "}",
        ]
        .map(String::from),
    );
    // Lower case, as std::errc: EINVAL may be a <cerrno> macro already included.
    let mut cpp = vec!["// cpp/xpute/kit/status/errno.spec.hpp".to_string()];
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
            "// The member `code` is, none for a number no member has.",
            "constexpr std::optional<Errno> errno_of(std::int32_t code) noexcept {",
            "    switch (code) {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            cpp.push(format!("    case {}: return Errno::{};", m.value, m.name.to_lowercase()));
        }
    }
    cpp.extend(
        [
            "    }",
            "    return std::nullopt;",
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
        Output::new("xpute/ts/kit/src/status/errno.spec.ts", file(&ts)),
        Output::new("xpute/rust/kit/src/status/errno.spec.rs", file(&rs)),
        Output::new("xpute/cpp/xpute/kit/status/errno.spec.hpp", file(&cpp)),
    ])
}
