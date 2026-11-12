// xpute-tool/spec/errno.rs

//! This generates xpute's errno table for TypeScript, Rust and C++ from one
//! JSON. The numbers cross between the core and its runtime, so every side
//! must agree on them.

use serde::Deserialize;

use xpute_gen::spec::{file, generated, json_str, read_as, Output};

use crate::root;

const JSON: &str = "spec/status/errno.json";

#[derive(Deserialize)]
struct ErrnoMember {
    name: String,
    val: i32,
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

pub fn outputs() -> Result<Vec<Output>, String> {
    let spec: ErrnoSpec = read_as(&root().join(JSON).to_string_lossy())?;
    let mut head = vec!["//".to_string(), generated(JSON), "//".into()];
    head.extend(spec.doc.iter().map(|l| format!("// {l}")));

    let mut ts = vec!["// @xpute/kit/status/errno.spec.ts".to_string()];
    ts.extend(head.iter().cloned());
    // TypeScript gets one constant per member, read as a namespace (`import *
    // as Errno`), and no enum: a file transpiled alone, or with its types
    // stripped, can still read the constants, and a bundler inlines the ones
    // it uses.
    ts.extend([
        String::new(),
        "import type { i32 } from \"../abi/word.ts\";".to_string(),
        String::new(),
    ]);
    let mut rs = vec!["// xpute-kit/status/errno.spec.rs".to_string()];
    rs.extend(head.iter().cloned());
    rs.extend(
        [
            "",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
            "#[repr(i32)]",
            "pub enum Errno {",
        ]
        .map(String::from),
    );
    let mut run: Vec<&ErrnoMember> = Vec::new();
    let flush = |run: &mut Vec<&ErrnoMember>, rs: &mut Vec<String>| {
        let width = run
            .iter()
            .map(|e| format!("{} = {},", e.name, e.val).len())
            .max()
            .unwrap_or(0);
        for e in run.iter() {
            rs.push(format!(
                "    {:<width$} // {}",
                format!("{} = {},", e.name, e.val),
                e.doc
            ));
        }
        run.clear();
    };
    for e in &spec.entries {
        match e {
            ErrnoEntry::Section { section } => {
                flush(&mut run, &mut rs);
                rs.extend([String::new(), format!("    // ---- {section} ----")]);
                ts.extend([
                    String::new(),
                    format!("// ---- {section} ----"),
                    String::new(),
                ]);
            }
            ErrnoEntry::Member(m) => {
                run.push(m);
                ts.push(format!("export const {} = {}; // {}", m.name, m.val, m.doc));
            }
        }
    }
    flush(&mut run, &mut rs);
    rs.extend(["}".to_string(), String::new()]);
    // A code from the other side is read into a member once, where it
    // arrives; a number no member has is the sender's error.
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
            rs.push(format!("            {} => Some(Errno::{}),", m.val, m.name));
        }
    }
    rs.extend(["            _ => None,", "        }", "    }", "}"].map(String::from));
    ts.push(String::new());

    ts.extend(
        [
            "/** An errno as a reader is shown it: its name and what it means. The",
            " * strings are for a person to read; an error carries the number alone. */",
            "export function strerror(errnum: i32): string {",
            "  return STRERROR[errnum] ?? `errno ${errnum}`;",
            "}",
            "",
            "const STRERROR: Record<number, string> = {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            ts.push(format!(
                "  [{}]: {},",
                m.val,
                json_str(&format!("{}: {}", m.name, m.doc))
            ));
        }
    }
    ts.push("};".into());
    ts.extend(
        [
            "",
            "/** An errno's name, as the Rust and C++ ends print it (`errno_name`),",
            " * or the number for one no member has. */",
            "export function errno_name(errnum: i32): string {",
            "  return ERRNO_NAME[errnum] ?? `${errnum}`;",
            "}",
            "",
            "const ERRNO_NAME: Record<number, string> = {",
        ]
        .map(String::from),
    );
    for e in &spec.entries {
        if let ErrnoEntry::Member(m) = e {
            ts.push(format!("  [{}]: {},", m.val, json_str(&m.name)));
        }
    }
    ts.push("};".into());
    ts.extend(
        [
            "",
            "/** The member `code` is, null for a number no member has. */",
            "export function errno_of(code: number): i32 | null {",
            "  return Object.hasOwn(STRERROR, code) ? code : null;",
            "}",
        ]
        .map(String::from),
    );
    // C++ spells the members in lower case, as std::errc does, because a name
    // such as EINVAL may already be a <cerrno> macro.
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
            ErrnoEntry::Section { section } => {
                cpp.extend([String::new(), format!("    // ---- {section} ----")]);
            }
            ErrnoEntry::Member(m) => cpp.push(format!(
                "    {} = {}, // {}",
                m.name.to_lowercase(),
                m.val,
                m.doc
            )),
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
            cpp.push(format!(
                "    case Errno::{}: return \"{}\";",
                m.name.to_lowercase(),
                m.name
            ));
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
            cpp.push(format!(
                "    case {}: return Errno::{};",
                m.val,
                m.name.to_lowercase()
            ));
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
        Output::new("ts/kit/src/status/errno.spec.ts", file(&ts)),
        Output::new("rust/kit/src/status/errno.spec.rs", file(&rs)),
        Output::new("cpp/xpute/kit/status/errno.spec.hpp", file(&cpp)),
    ])
}
