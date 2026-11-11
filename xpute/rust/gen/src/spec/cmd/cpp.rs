// xpute-gen/spec/cmd/cpp.rs

//! This writes a command table as a C++ core's header: its enums, a reader for
//! each packet, and a struct and a writer for each import. A C++ core writes no
//! record.

use super::service::{ackreq_of, fields_of, reply_of, tag_of};
use super::{literal, members, Entry, Field, Kind, Member, Spec, Target};
use crate::spec::{file, generated, pascal};

fn cpp_read_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let kind = spec.kind(&f.ty)?;
    if f.list {
        return Err(format!("protocol: a C++ core reads no list ({})", f.name));
    }
    let one = match kind {
        Kind::Number => cpp_number(&f.ty).to_string(),
        Kind::Str => "std::string_view".to_string(),
        Kind::Bool => "bool".to_string(),
        Kind::Path => "xpute::Path".to_string(),
        _ => {
            return Err(format!(
                "protocol: a C++ core reads no {} ({})",
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

/// A number is checked as the Rust core checks one (`abi/number.rs`), and is
/// never simply cast into its type.
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
        Kind::Path => read(format!("xpute::Path::read(*{c})")),
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

fn cpp_write_type(spec: &Spec, f: &Field) -> Result<String, String> {
    let one = match spec.kind(&f.ty)? {
        _ if f.list => return Err(format!("protocol: a C++ core writes no list ({})", f.name)),
        Kind::Number => cpp_number(&f.ty).to_string(),
        Kind::Str => "std::string_view".to_string(),
        Kind::Bool => "bool".to_string(),
        Kind::Path => "std::span<const xpute::Token>".to_string(),
        _ => {
            return Err(format!(
                "protocol: a C++ core writes no {} ({})",
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

/// The writer's method takes the field's value, or the optional itself where
/// the field is optional.
fn cpp_write(spec: &Spec, f: &Field) -> Result<String, String> {
    if spec.kind(&f.ty)? == Kind::Path {
        return Ok(format!("xpute::Path::write(w, {});", f.name));
    }
    let method = match spec.kind(&f.ty)? {
        Kind::Bool => "boolean",
        Kind::Str => "str",
        _ => f.ty.as_str(),
    };
    Ok(format!("w.{method}({});", f.name))
}

/// This writes an import, a frame the core sends: its number, the children
/// its packet is written as, its writer, and the memory it names, where it is
/// and how long (0 long for none), as `Import` in cpp/xpute/core/io/door.hpp
/// requires.
fn cpp_import(
    spec: &Spec,
    out: &mut Vec<String>,
    c: &Member,
    fields: &[Field],
) -> Result<(), String> {
    out.extend([
        String::new(),
        format!("// Command::{}'s packet.", c.name),
        format!("struct {} {{", pascal(&c.name)),
    ]);
    for f in fields {
        if let Some(doc) = &f.doc {
            out.extend(crate::spec::line_comments(doc, "    "));
        }
        out.push(format!("    {} {}{{}};", cpp_write_type(spec, f)?, f.name));
    }
    out.extend([
        String::new(),
        "    // Its number, the children it is written as, and whether it goes".to_string(),
        "    // with ACKREQ.".to_string(),
        format!("    static constexpr std::uint32_t NUMBER = {};", c.val),
        format!(
            "    static constexpr std::uint32_t FIELDS = {};",
            fields.len()
        ),
        format!("    static constexpr bool ACKREQ = {};", c.ackreq),
        String::new(),
        format!(
            "    void write(xtp::PacketWriter &{}) const noexcept {{",
            if fields.is_empty() { "" } else { "w" }
        ),
    ]);
    for f in fields {
        out.push(format!("        {}", cpp_write(spec, f)?));
    }
    out.extend([
        "    }".to_string(),
        String::new(),
        "    std::array<std::uint32_t, 2> named() const noexcept {".to_string(),
        match &c.mem {
            Some([at, len]) => format!("        return {{{at}, {len}}};"),
            None => "        return {0, 0};".to_string(),
        },
        "    }".to_string(),
        "};".to_string(),
    ]);
    Ok(())
}

pub fn emit(spec: &Spec, t: &Target, path: &str, namespace: &str) -> Result<String, String> {
    let mut out = vec![
        format!("// {path}"),
        "//".into(),
        generated(t.json),
        "//".into(),
    ];
    out.extend(crate::spec::line_comments(&spec.doc, ""));
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
            "#include <variant>",
            "",
            "#include \"xpute/kit/abi/cmd.hpp\"",
            "#include \"xpute/kit/status/error.hpp\"",
            "#include \"xpute/kit/wire/xtp/cursor.hpp\"",
            "#include \"xpute/kit/wire/xtp/encode.hpp\"",
            "#include \"xpute/core/abi/number.hpp\"",
            "#include \"xpute/core/host.hpp\"",
            "#include \"xpute/core/io/frame.hpp\"",
        ]
        .map(String::from),
    );
    if spec.has_path()? {
        out.push("#include \"xpute/core/io/fs.hpp\"".to_string());
    }
    out.extend([String::new(), format!("namespace {namespace} {{")]);
    for e in &spec.enums {
        let all: Vec<&Member> = members(e).collect();
        out.push(String::new());
        if let Some(doc) = &e.doc {
            out.extend(crate::spec::line_comments(doc, ""));
        }
        out.push(format!("enum class {} : std::uint32_t {{", e.name));
        for entry in &e.entries {
            match entry {
                Entry::Section(s) => {
                    out.extend([String::new(), format!("    // ---- {} ----", s.section)]);
                }
                Entry::Member(m) => {
                    if let Some(doc) = &m.doc {
                        out.extend(crate::spec::line_comments(doc, "    "));
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
                "template <> struct xpute::EnumInfo<{namespace}::{}> {{",
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
        return Err(format!("{}: a C++ core writes no record", t.json));
    }
    for c in spec.commands()? {
        let pascal_name = pascal(&c.name);
        let pkts = [
            (c.pkt.as_ref(), pascal_name.clone(), "packet"),
            (
                c.reply.as_ref().filter(|r| !r.is_empty()),
                format!("{pascal_name}Reply"),
                "completion's packet",
            ),
        ];
        for (pkt, name, what) in pkts {
            let Some(pkt) = pkt else {
                continue;
            };
            out.extend([
                String::new(),
                format!("// Command::{}'s {what}.", c.name),
                format!("struct {name} {{"),
            ]);
            for f in pkt {
                if let Some(doc) = &f.doc {
                    out.extend(crate::spec::line_comments(doc, "    "));
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
        if let Some(import) = &c.import {
            cpp_import(spec, &mut out, c, import)?;
        }
    }
    core_end(spec, &mut out)?;
    out.extend([String::new(), format!("}} // namespace {namespace}")]);
    Ok(file(&out))
}

/// Writes the protocol's core end, as the Rust core's module is written
/// (cmd/service.rs `rs`): a function for each `import` over the door it is
/// handed, the dispatch of each `export` to the handler's member of its name,
/// and the protocol's completions, read off the door's `Completion`.
fn core_end(spec: &Spec, out: &mut Vec<String>) -> Result<(), String> {
    let p = &spec.protocol;
    for (cmd, call) in &p.imports {
        let f = cmd.to_lowercase();
        let ty = pascal(cmd);
        let fields = fields_of(spec, cmd, false).unwrap_or(&[]);
        let tag = tag_of(spec, cmd);
        let mut params = vec!["const D &door".to_string()];
        if let Some(tag) = tag {
            params.push(format!("std::uint32_t {tag}"));
        }
        let arg = if fields.is_empty() {
            format!("{ty}{{}}")
        } else {
            params.push(format!("const {ty} &e"));
            "e".to_string()
        };
        let params = params.join(", ");
        out.push(String::new());
        let (doc, ret, body) = if *call {
            (
                "answered by the runtime before this returns",
                "Result<std::uint32_t>",
                format!("return door.call({arg});"),
            )
        } else if ackreq_of(spec, cmd) {
            match tag {
                Some(tag) => (
                    "completed on a later turn",
                    "void",
                    format!("door.import({tag}, {arg});"),
                ),
                None => (
                    "completed on a later turn under the handle it answers",
                    "std::uint32_t",
                    format!("return door.request({arg});"),
                ),
            }
        } else {
            match tag {
                Some(tag) => (
                    "posted for the runtime",
                    "void",
                    format!("door.import({tag}, {arg});"),
                ),
                None => (
                    "posted for the runtime",
                    "void",
                    format!("door.send({arg});"),
                ),
            }
        };
        out.push(format!("// Command::{cmd}, {doc}."));
        out.push(format!(
            "template <class D> {ret} {f}({params}) noexcept {{ {body} }}"
        ));
    }
    if !p.exports.is_empty() {
        out.extend([
            String::new(),
            "// `cmd` read and handed to the member of `h` of its name; ENOSYS for one this".into(),
            "// protocol does not carry.".into(),
            "template <class H> Outcome apply(H &h, std::uint32_t cmd, std::span<const std::uint8_t> pkt) noexcept {".into(),
        ]);
        for (cmd, _, _) in &p.exports {
            let f = cmd.to_lowercase();
            out.push(format!(
                "    if (cmd == static_cast<std::uint32_t>(Command::{cmd})) {{"
            ));
            match fields_of(spec, cmd, true) {
                Some(_) => out.extend([
                    format!(
                        "        Result<{}> c = {}::read(pkt);",
                        pascal(cmd),
                        pascal(cmd)
                    ),
                    "        if (!c) return {.err = c.error().errnum};".to_string(),
                    format!("        return h.{f}(*c);"),
                ]),
                None => out.push(format!("        return h.{f}();")),
            }
            out.push("    }".into());
        }
        out.extend([
            "    return {.err = Errno::enosys};".to_string(),
            "}".to_string(),
        ]);
    }
    let requests: Vec<&str> = p
        .imports
        .iter()
        .filter(|(cmd, _)| ackreq_of(spec, cmd))
        .map(|(cmd, _)| cmd.as_str())
        .collect();
    if !requests.is_empty() {
        for cmd in &requests {
            let tag = tag_of(spec, cmd).unwrap_or("handle");
            out.extend([
                String::new(),
                format!("// Command::{cmd}'s completion, under the handle it was made with, and its result."),
                format!("struct {}Completion {{", pascal(cmd)),
                format!("    std::uint32_t {tag};"),
                "    Result<std::uint32_t> res;".to_string(),
            ]);
            if reply_of(spec, cmd).is_some() {
                out.push(format!("    std::optional<{}Reply> reply;", pascal(cmd)));
            }
            out.push("};".into());
        }
        let names: Vec<String> = requests
            .iter()
            .map(|c| format!("{}Completion", pascal(c)))
            .collect();
        out.extend([
            String::new(),
            format!("using Completion = std::variant<{}>;", names.join(", ")),
            String::new(),
            "// `c` read as one of this protocol's completions, or none for another's.".into(),
            "inline std::optional<Completion> completion(const ::xpute::Completion &c) noexcept {"
                .into(),
            "    Result<std::uint32_t> res = answer_of(c.res);".into(),
        ]);
        for cmd in &requests {
            let tag = tag_of(spec, cmd).unwrap_or("handle");
            out.push(format!(
                "    if (c.cmd == static_cast<std::uint32_t>(Command::{cmd})) {{"
            ));
            if reply_of(spec, cmd).is_some() {
                out.extend([
                    format!("        std::optional<{}Reply> reply;", pascal(cmd)),
                    "        if (!c.pkt.empty()) {".to_string(),
                    format!("            if (Result<{}Reply> r = {}Reply::read(c.pkt)) reply = *r;", pascal(cmd), pascal(cmd)),
                    "        }".to_string(),
                    format!(
                        "        return Completion{{{}Completion{{.{tag} = c.tag, .res = res, .reply = reply}}}};",
                        pascal(cmd)
                    ),
                ]);
            } else {
                out.push(format!(
                    "        return Completion{{{}Completion{{.{tag} = c.tag, .res = res}}}};",
                    pascal(cmd)
                ));
            }
            out.push("    }".into());
        }
        out.extend(["    return std::nullopt;".to_string(), "}".to_string()]);
    }
    Ok(())
}
