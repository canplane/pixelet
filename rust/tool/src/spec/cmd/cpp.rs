// pxl-tool/spec/cmd/cpp.rs

//! A command table as a C++ guest's header: its enums and each packet's
//! reader; a C++ guest raises no event and writes no record.

use super::{literal, members, Entry, Field, Kind, Member, Spec, Target};
use crate::spec;
use crate::spec::{file, pascal};

fn cpp_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        return Err(format!("cmd.json: a C++ guest reads no list ({})", f.name));
    }
    let one = match kind {
        Kind::Number => cpp_number(&f.ty).to_string(),
        Kind::Str => "std::string_view".to_string(),
        Kind::Bool => "bool".to_string(),
        _ => {
            return Err(format!(
                "cmd.json: a C++ guest reads no {} ({})",
                f.ty, f.name
            ))
        }
    };
    Ok(if f.optional {
        format!("std::optional<{one}>")
    } else {
        one
    })
}

fn cpp_number(ty: &str) -> &'static str {
    match ty {
        "u8" => "std::uint8_t",
        "u32" => "std::uint32_t",
        "i32" => "std::int32_t",
        _ => "double",
    }
}

/// A number is checked as the Rust guest's is (`abi/number.rs`), never cast into its type.
fn cpp_read_one(spec: &Spec, f: &Field, c: &str, to: &str) -> Result<Vec<String>, String> {
    let read = |call: String| {
        vec![
            format!("auto v = {call};"),
            "if (!v) return v.error();".to_string(),
        ]
    };
    let mut out = match spec.kind(&f.ty)? {
        Kind::Number => {
            let mut out = read(format!("{c}->get_number()"));
            out.push(match f.ty.as_str() {
                "f64" => "auto n = finite(*v);".to_string(),
                ty => format!("auto n = integer<{}>(*v);", cpp_number(ty)),
            });
            out.extend([
                "if (!n) return n.error();".to_string(),
                format!("{to} = *n;"),
            ]);
            return Ok(out);
        }
        Kind::Bool => read(format!("{c}->get_bool()")),
        _ => read(format!("{c}->get_str_ref()")),
    };
    out.push(format!("{to} = *v;"));
    Ok(out)
}

fn cpp_read(spec: &Spec, f: &Field, i: usize) -> Result<Vec<String>, String> {
    let mut out = vec![
        "{".to_string(),
        format!("    Result<xtp::Cursor> c = f->at({i});"),
        "    if (!c) return c.error();".to_string(),
    ];
    if f.optional {
        out.push("    if (!c->is_null()) {".to_string());
        out.extend(
            cpp_read_one(spec, f, "c", &format!("out.{}", f.name))?
                .into_iter()
                .map(|l| format!("        {l}")),
        );
        out.push("    }".to_string());
    } else {
        out.extend(
            cpp_read_one(spec, f, "c", &format!("out.{}", f.name))?
                .into_iter()
                .map(|l| format!("    {l}")),
        );
    }
    out.push("}".to_string());
    Ok(out)
}

pub fn emit(spec: &Spec, t: &Target, path: &str, namespace: &str) -> Result<String, String> {
    let mut out = vec![
        format!("// {}", spec::in_xpute(path)),
        "//".into(),
        spec::generated(t.json, t.json),
        "//".into(),
    ];
    out.extend(spec::line_comments(&spec.doc, ""));
    out.extend(
        [
            "",
            "#pragma once",
            "",
            "#include <array>",
            "#include <cstdint>",
            "#include <optional>",
            "#include <span>",
            "#include <string_view>",
            "",
            "#include \"xpute/kit/abi/cmd.hpp\"",
            "#include \"xpute/kit/status/error.hpp\"",
            "#include \"xpute/kit/wire/xtp/cursor.hpp\"",
            "#include \"xpute/kit/wire/xtp/encode.hpp\"",
            "#include \"number.hpp\"",
            "",
            &format!("namespace {namespace} {{"),
        ]
        .map(String::from),
    );
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        out.push(String::new());
        if let Some(doc) = &e.doc {
            out.extend(spec::line_comments(doc, ""));
        }
        out.push(format!("enum class {} : std::uint32_t {{", e.name));
        for entry in &e.entries {
            match entry {
                Entry::Section(s) => {
                    out.extend([String::new(), format!("    // ---- {} ----", s.section)]);
                }
                Entry::Member(m) => {
                    if let Some(doc) = &m.doc {
                        out.extend(spec::line_comments(doc, "    "));
                    }
                    out.push(format!("    {} = {},", m.name, literal(m.val, &e.format)));
                }
            }
        }
        out.extend([
            "};".to_string(),
            String::new(),
            format!("}} // namespace {namespace}"),
            String::new(),
            format!(
                "template <> struct xpute::Members<{namespace}::{}> {{",
                e.name
            ),
            format!(
                "    static constexpr std::array<{namespace}::{0}, {1}> ALL{{{2}}};",
                e.name,
                all.len(),
                all.iter()
                    .map(|m| format!("{namespace}::{}::{}", e.name, m.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "};".to_string(),
            String::new(),
            format!("namespace {namespace} {{"),
        ]);
    }
    if !spec.records.is_empty() {
        return Err(format!("{}: a C++ guest writes no record", t.json));
    }
    for c in spec.commands()? {
        let name = pascal(&c.name);
        if c.event.is_some() {
            return Err(format!(
                "{}: a C++ guest raises no event ({})",
                t.json, c.name
            ));
        }
        if let Some(pkt) = &c.pkt {
            out.extend([
                String::new(),
                format!("// Command::{}'s packet.", c.name),
                format!("struct {name} {{"),
            ]);
            for f in pkt {
                if let Some(doc) = &f.doc {
                    out.extend(spec::line_comments(doc, "    "));
                }
                out.push(format!("    {} {}{{}};", cpp_read_type(spec, f)?, f.name));
            }
            out.extend([
                String::new(),
                format!(
                    "    static Result<{name}> read(std::span<const std::uint8_t> pkt) noexcept {{"
                ),
                "        Result<xtp::TreeReader> t = xtp::TreeReader::make(pkt);".to_string(),
                "        if (!t) return t.error();".to_string(),
                "        Result<xtp::BranchCursor> f = t->read_branch();".to_string(),
                "        if (!f) return f.error();".to_string(),
                format!("        {name} out;"),
            ]);
            for (i, f) in pkt.iter().enumerate() {
                out.extend(
                    cpp_read(spec, f, i)?
                        .into_iter()
                        .map(|l| format!("        {l}")),
                );
            }
            out.extend([
                "        return out;".to_string(),
                "    }".to_string(),
                "};".to_string(),
            ]);
        }
    }
    out.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&out))
}
