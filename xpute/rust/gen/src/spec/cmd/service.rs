// xpute-gen/spec/cmd/service.rs

//! This writes a protocol's binding onto xpute's two ends, on top of its
//! messages. On the runtime it writes the type of each `export`'s fields, by
//! its command's number, and a handler per `import`, the program's own keyed by
//! the number too, so that no command's name reaches a bundle. On the core it
//! writes a function per `import`, and a trait the program implements for its
//! `export`s. A runtime routes a frame to the binding by its command's major.

use super::{Field, Kind, Protocol, Spec};
use crate::spec::{pascal, rs_doc, ts_doc};

fn module_name(p: &Protocol) -> String {
    p.name.to_lowercase()
}

/// The name of what the import `name`'s tag carries, where it carries one.
pub(super) fn tag_of<'a>(spec: &'a Spec, name: &str) -> Option<&'a str> {
    let c = spec.cmds().ok()?.into_iter().find(|c| c.name == name)?;
    c.tag.as_deref()
}

pub(super) fn fields_of<'a>(spec: &'a Spec, name: &str, export: bool) -> Option<&'a [Field]> {
    let c = spec.cmds().ok()?.into_iter().find(|c| c.name == name)?;
    if export {
        c.pkt.as_deref()
    } else {
        c.import.as_deref()
    }
}

/// Whether the import `name` goes with ACKREQ, to be completed on a later turn.
pub(super) fn ackreq_of(spec: &Spec, name: &str) -> bool {
    spec.cmds()
        .ok()
        .and_then(|cs| cs.into_iter().find(|c| c.name == name).map(|c| c.ackreq))
        .unwrap_or(false)
}

pub(super) fn reply_of<'a>(spec: &'a Spec, name: &str) -> Option<&'a [Field]> {
    let c = spec.cmds().ok()?.into_iter().find(|c| c.name == name)?;
    c.reply.as_deref()
}

/// Writes the protocol's core end, a module of the protocol's name over the
/// door at `door` (`xpute_core::io::door`, or `crate::io::door` inside
/// xpute-core): a function for each `import`, which sends it, calls it and
/// answers its result, or requests it and answers the handle its completion
/// comes back under; a trait the program implements for its `export`s, with
/// the dispatch that reads each export's packet and calls the trait's method;
/// and the protocol's completions, read off the door's `Completion`. A tag the
/// spec names is the caller's to give, and is the handle. With `wrap`, a
/// program's, the end is a module of its own; xpute's own protocols' files
/// are each its protocol's module already, and hold the end as it is.
pub(super) fn rs(spec: &Spec, door: &str, wrap: bool) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let p = &spec.protocol;
    let name = module_name(p);
    let host = door.replace("::io::door", "::host");
    let frame = door.replace("::io::door", "::io::frame");
    let requests: Vec<&str> = p
        .imports
        .iter()
        .filter(|(cmd, _)| ackreq_of(spec, cmd))
        .map(|(cmd, _)| cmd.as_str())
        .collect();
    out.push(String::new());
    out.extend(rs_doc(
        &[format!(
            "The {} protocol's core end: what it posts to the runtime and what it hears from it.",
            p.name
        )],
        "",
        "///",
    ));
    out.push(format!("pub mod {name} {{"));
    out.push("    use super::*;".into());
    let mut uses = vec!["door"];
    if !p.exports.is_empty() {
        uses.push("Outcome");
    }
    if !requests.is_empty() {
        uses.push("Completion as Done");
    }
    if !p.imports.is_empty() || !p.exports.is_empty() {
        out.push(format!("    use {door}::{{{}}};", uses.join(", ")));
    }
    if !requests.is_empty() {
        out.push(format!("    use {host}::answer_of;"));
    }
    out.push("    use xpute_kit::status::errno::Errno;".into());
    for (cmd, call) in &p.imports {
        let fields = fields_of(spec, cmd, false).unwrap_or(&[]);
        let ty = format!("{}{}", pascal(cmd), lifetime(spec, fields, false)?);
        let f = cmd.to_lowercase();
        let tag = tag_of(spec, cmd);
        let mut params = Vec::new();
        if let Some(tag) = tag {
            params.push(format!("{tag}: u32"));
        }
        let arg = if fields.is_empty() {
            format!("&{} {{}}", pascal(cmd))
        } else {
            params.push(format!("e: &{ty}"));
            "e".to_string()
        };
        let params = params.join(", ");
        out.push(String::new());
        if *call {
            out.push(format!(
                "    /// Command::{cmd}, answered by the runtime before this returns."
            ));
            out.push(format!("    pub fn {f}({params}) -> Result<u32, Errno> {{"));
            out.push(format!("        door().call({arg})"));
        } else if ackreq_of(spec, cmd) {
            out.push(format!(
                "    /// Command::{cmd}, completed on a later turn (`Completion::{}`).",
                pascal(cmd)
            ));
            match tag {
                Some(tag) => {
                    out.push(format!("    pub fn {f}({params}) {{"));
                    out.push(format!("        door().import({tag}, {arg});"));
                }
                None => {
                    out.push(
                        "    /// It answers the handle the completion comes back under.".into(),
                    );
                    out.push(format!("    pub fn {f}({params}) -> u32 {{"));
                    out.push(format!("        door().request({arg})"));
                }
            }
        } else {
            out.push(format!("    /// Command::{cmd}, posted for the runtime."));
            out.push(format!("    pub fn {f}({params}) {{"));
            match tag {
                Some(tag) => out.push(format!("        door().import({tag}, {arg});")),
                None => out.push(format!("        door().send({arg});")),
            }
        }
        out.push("    }".into());
    }
    if !p.exports.is_empty() {
        out.push(String::new());
        out.push("    /// What the program does with each command the runtime sends.".into());
        out.push("    pub trait Exports {".into());
        for (cmd, _, _) in &p.exports {
            let f = cmd.to_lowercase();
            match fields_of(spec, cmd, true) {
                Some(fields) => out.push(format!(
                    "        fn {f}(&mut self, c: {}{}) -> Outcome;",
                    pascal(cmd),
                    lifetime(spec, fields, true)?
                )),
                None => out.push(format!("        fn {f}(&mut self) -> Outcome;")),
            }
        }
        out.push("    }".into());
        out.push(String::new());
        let minors = minors_only(spec);
        out.push(if minors {
            "    /// `cmd`, under the program's `major`, read and handed to `h`; ENOSYS for one this protocol does not carry.".into()
        } else {
            "    /// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.".into()
        });
        let reads = p
            .exports
            .iter()
            .any(|(cmd, _, _)| fields_of(spec, cmd, true).is_some());
        out.push(format!(
            "    pub fn apply<H: Exports>(h: &mut H, {}cmd: u32, {}: &Option<&[u8]>) -> Outcome {{",
            if minors { "major: u32, " } else { "" },
            if reads { "pkt" } else { "_pkt" }
        ));
        out.push("        match cmd {".into());
        for (cmd, _, _) in &p.exports {
            let f = cmd.to_lowercase();
            let arm = match fields_of(spec, cmd, true) {
                Some(_) => format!(
                    "h.{f}({}::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?)",
                    pascal(cmd)
                ),
                None => format!("h.{f}()"),
            };
            let number = if minors {
                format!("{frame}::cmd(major, Command::{cmd} as u32)")
            } else {
                format!("Command::{cmd} as u32")
            };
            out.push(format!("            x if x == {number} => {arm},"));
        }
        out.push("            _ => Err(MarshalError::new(Errno::ENOSYS)),".into());
        out.push("        }".into());
        out.push("    }".into());
    }
    if !requests.is_empty() {
        out.push(String::new());
        out.push(
            "    /// A request's completion, under the handle it was made with, and its result."
                .into(),
        );
        out.push("    #[derive(Clone, Copy, Debug)]".into());
        out.push("    pub enum Completion {".into());
        for cmd in &requests {
            let tag = tag_of(spec, cmd).unwrap_or("handle");
            let reply = reply_of(spec, cmd)
                .map(|_| format!(", reply: Option<{}Reply>", pascal(cmd)))
                .unwrap_or_default();
            out.push(format!(
                "        {} {{ {tag}: u32, res: Result<u32, Errno>{reply} }},",
                pascal(cmd)
            ));
        }
        out.push("    }".into());
        out.push(String::new());
        out.push(
            "    /// `c` read as one of this protocol's completions, or none for another's.".into(),
        );
        out.push("    pub fn completion(c: &Done<'_>) -> Option<Completion> {".into());
        out.push("        let res = answer_of(c.res);".into());
        out.push("        match c.cmd {".into());
        for cmd in &requests {
            let tag = tag_of(spec, cmd).unwrap_or("handle");
            let reply = reply_of(spec, cmd)
                .map(|_| {
                    format!(
                        ", reply: c.pkt.and_then(|pkt| {}Reply::read(pkt).ok())",
                        pascal(cmd)
                    )
                })
                .unwrap_or_default();
            out.push(format!(
                "            x if x == Command::{cmd} as u32 => Some(Completion::{} {{ {tag}: c.tag, res{reply} }}),",
                pascal(cmd)
            ));
        }
        out.push("            _ => None,".into());
        out.push("        }".into());
        out.push("    }".into());
    }
    out.push("}".into());
    if !wrap {
        let start = out
            .iter()
            .position(|l| l.starts_with("pub mod "))
            .unwrap_or(0);
        out.remove(start);
        out.retain(|l| l != "    use super::*;");
        out.pop();
        for l in out.iter_mut().skip(start) {
            if let Some(rest) = l.strip_prefix("    ") {
                *l = rest.to_string();
            }
        }
    }
    Ok(out.join("\n") + "\n")
}

/// Writes the protocol's runtime end of its `import`s: the majors they take
/// (`MAJORS`), which the runtime routes them by; a trait the runtime
/// implements, a method for each import, handed its tag where the spec names
/// one, its packet read, and a copy of the memory it names; and `serve`, which
/// reads each command and hands it to the trait's method, as the core end's
/// `apply` hands an export.
pub(super) fn rs_runtime(spec: &Spec) -> Result<String, String> {
    let p = &spec.protocol;
    let mut out: Vec<String> = Vec::new();
    if p.imports.is_empty() {
        return Ok(String::new());
    }
    let cmds = spec.cmds()?;
    let names_mem = |cmd: &str| cmds.iter().any(|c| c.name == cmd && c.mem.is_some());
    let reads = |cmd: &str| fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty());
    let majors: Vec<String> = majors_of(spec)
        .into_iter()
        .map(|(major, name)| {
            name.map_or(format!("0x{major:02x}"), |n| format!("Major::{n} as u32"))
        })
        .collect();
    if !majors.is_empty() {
        out.push(String::new());
        out.push("/// The majors the runtime routes the protocol's imports by, to `serve`.".into());
        out.push(format!(
            "pub const MAJORS: [u32; {}] = [{}];",
            majors.len(),
            majors.join(", ")
        ));
    }
    out.push(String::new());
    out.push("/// What the runtime does with each command the core posts.".into());
    out.push("pub trait Imports {".into());
    for (cmd, _) in &p.imports {
        let mut params = vec!["&mut self".to_string()];
        if let Some(tag) = tag_of(spec, cmd) {
            params.push(format!("{tag}: u32"));
        }
        if reads(cmd) {
            let fields = fields_of(spec, cmd, false).unwrap_or(&[]);
            params.push(format!(
                "e: {}{}",
                pascal(cmd),
                lifetime(spec, fields, true)?
            ));
        }
        if names_mem(cmd) {
            params.push("named: &[u8]".into());
        }
        out.push(format!(
            "    fn {}({}) -> Outcome;",
            cmd.to_lowercase(),
            params.join(", ")
        ));
    }
    out.push("}".into());
    out.push(String::new());
    let minors = minors_only(spec);
    let used = |any: bool, name: &str| {
        if any {
            name.to_string()
        } else {
            format!("_{name}")
        }
    };
    let tag = used(
        p.imports.iter().any(|(c, _)| tag_of(spec, c).is_some()),
        "tag",
    );
    let pkt = used(p.imports.iter().any(|(c, _)| reads(c)), "pkt");
    let named = used(p.imports.iter().any(|(c, _)| names_mem(c)), "named");
    out.push(if minors {
        "/// `cmd`, under the program's `major`, read and handed to `h`; ENOSYS for one this protocol does not carry.".into()
    } else {
        "/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.".into()
    });
    out.push(format!(
        "pub fn serve<H: Imports>(h: &mut H, {}{tag}: u32, cmd: u32, {pkt}: &Option<&[u8]>, {named}: &[u8]) -> Outcome {{",
        if minors { "major: u32, " } else { "" },
    ));
    out.push("    match cmd {".into());
    for (cmd, _) in &p.imports {
        let mut args = Vec::new();
        if tag_of(spec, cmd).is_some() {
            args.push("tag".to_string());
        }
        if reads(cmd) {
            args.push(format!(
                "{}::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?",
                pascal(cmd)
            ));
        }
        if names_mem(cmd) {
            args.push("named".into());
        }
        let number = if minors {
            format!("xpute_core::io::frame::cmd(major, Command::{cmd} as u32)")
        } else {
            format!("Command::{cmd} as u32")
        };
        out.push(format!(
            "        x if x == {number} => h.{}({}),",
            cmd.to_lowercase(),
            args.join(", ")
        ));
    }
    out.push("        _ => Err(MarshalError::new(Errno::ENOSYS)),".into());
    out.push("    }".into());
    out.push("}".into());
    Ok(out.join("\n") + "\n")
}

/// Gives `<'_>` where the type borrows: with `read`, a packet read in place,
/// as an export's is; without it, an import that writes a borrowed field.
fn lifetime(spec: &Spec, fields: &[Field], read: bool) -> Result<&'static str, String> {
    let borrows = if read {
        let mut b = false;
        for f in fields {
            let kind = spec.kind(&f.ty)?;
            b |= f.list || kind == Kind::Str || kind == Kind::Path;
        }
        b
    } else {
        super::rs::rs_borrows(spec, fields)?
    };
    Ok(if borrows { "<'_>" } else { "" })
}

/// The majors the protocol's `import`s take, each with its `Major` member's
/// name where the protocol names one, and none where its commands are minors
/// under a major the program gives.
fn majors_of(spec: &Spec) -> Vec<(u32, Option<String>)> {
    let named = spec.enums.iter().find(|e| e.name == "Major");
    let Ok(cmds) = spec.cmds() else {
        return Vec::new();
    };
    let mut out: Vec<(u32, Option<String>)> = Vec::new();
    for (cmd, _) in &spec.protocol.imports {
        let Some(c) = cmds.iter().find(|c| c.name == *cmd) else {
            continue;
        };
        let major = c.val >> 8;
        if major == 0 || out.iter().any(|(m, _)| *m == major) {
            continue;
        }
        let name = named
            .and_then(|e| super::members(e).find(|m| m.val == major))
            .map(|m| m.name.clone());
        out.push((major, name));
    }
    out
}

/// Whether the protocol's commands are minors, under a major the program gives.
pub(super) fn minors_only(spec: &Spec) -> bool {
    spec.cmds()
        .map(|cs| cs.iter().all(|c| c.val >> 8 == 0))
        .unwrap_or(false)
}

/// Writes the TypeScript binding's imports and body, the UI's end as WASI's
/// JavaScript bindings shape one: a plain function for each `export`, which
/// sends it, or requests it and answers a Promise of the reply's value, or of
/// the memory the reply names, that rejects with the errno it was refused with
/// (`{proto}_exports`); and the
/// interface the program implements for the protocol's `import`s, each member
/// answering a call's value or throwing kit's error to refuse, with the
/// protocol the runtime routes them to by major (`{proto}_imports`). A protocol
/// whose commands are minors takes its major from the program.
pub(super) fn ts(spec: &Spec, runtime: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut imports: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut types: Vec<&str> = Vec::new();
    let mut values: Vec<&str> = Vec::new();
    let p = &spec.protocol;
    let name = module_name(p);
    let ty = pascal(&p.name);
    let majors: Vec<String> = majors_of(spec)
        .into_iter()
        .map(|(major, name)| name.map_or(format!("0x{major:02x}"), |n| format!("Major.{n}")))
        .collect();
    let minors = minors_only(spec);
    let number = |cmd: &str| {
        if minors {
            format!("CMD(major, Command.{cmd})")
        } else {
            format!("Command.{cmd}")
        }
    };
    let cmds = spec.cmds()?;
    let doc_of = |cmd: &str| {
        cmds.iter()
            .find(|c| c.name == cmd)
            .and_then(|c| c.doc.as_ref())
    };
    if !p.exports.is_empty() {
        types.push("Link");
        let major = if minors { ", major: u32" } else { "" };
        out.extend([
            String::new(),
            format!(
                "/** The {} protocol's exports as the UI calls them over `link`: a send returns",
                p.name
            ),
            " * nothing, and a request a Promise of its reply's value, or of the memory its reply"
                .into(),
            " * names, which rejects with the errno it was refused with. `fits` says whether a"
                .into(),
            " * send's packet fits a frame. */".into(),
            format!("export function {name}_exports(link: Link{major}) {{"),
            "  return {".into(),
        ]);
        let mut fits = Vec::new();
        for (cmd, ack, named) in &p.exports {
            let fields = fields_of(spec, cmd, true).unwrap_or(&[]);
            let mut params = Vec::new();
            let mut args = vec![number(cmd)];
            for f in fields {
                params.push(format!("{}: {}", f.name, super::ts::ts_arg(spec, f)?));
                args.push(f.name.clone());
            }
            if let Some(doc) = doc_of(cmd) {
                out.extend(ts_doc(doc, "    "));
            }
            let f = cmd.to_lowercase();
            // A request resolves with its reply's value, or with the memory
            // its reply names.
            let call = match (ack, named) {
                (false, _) => format!("link.send({})", args.join(", ")),
                (true, false) => format!("link.request({}).then((r) => r.val)", args.join(", ")),
                (true, true) => format!("link.request({}).then((r) => r.named)", args.join(", ")),
            };
            out.push(format!("    {f}: ({}) => {call},", params.join(", ")));
            if !*ack {
                fits.push(format!(
                    "      {f}: ({}) => link.fits({}),",
                    params.join(", "),
                    args.join(", ")
                ));
            }
        }
        if !fits.is_empty() {
            out.push("    fits: {".into());
            out.extend(fits);
            out.push("    },".into());
        }
        out.extend(["  };".to_string(), "}".to_string()]);
        out.extend([
            String::new(),
            format!(
                "/** The {} protocol's exports, as `{name}_exports` binds them. */",
                p.name
            ),
            format!("export type {ty}Exports = ReturnType<typeof {name}_exports>;"),
        ]);
    }
    if !p.imports.is_empty() {
        types.extend(["Protocol", "OnImport"]);
        values.push("refusal_of");
        if majors.is_empty() {
            return Err(format!(
                "{}: imports a runtime routes need the protocol's majors",
                p.name
            ));
        }
        let iface = format!("{ty}Imports");
        let reads = p
            .imports
            .iter()
            .any(|(cmd, _)| fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()));
        out.extend([
            String::new(),
            "/** The majors the runtime routes the protocol's imports by. */".into(),
            format!(
                "export const MAJORS: readonly u32[] = [{}];",
                majors.join(", ")
            ),
            String::new(),
            format!(
                "/** What the program does with each command the core posts on the {} protocol: a",
                p.name
            ),
            " * call's member answers its value, and throws kit's error to refuse it. */".into(),
            format!("export interface {iface} {{"),
        ]);
        // A command that names the core's memory is handed a copy of it.
        let names_mem = |cmd: &str| cmds.iter().any(|c| c.name == cmd && c.mem.is_some());
        for (cmd, call) in &p.imports {
            let ret = if *call { "u32" } else { "void" };
            if let Some(doc) = doc_of(cmd) {
                out.extend(ts_doc(doc, "  "));
            }
            let mut params = Vec::new();
            if let Some(tag) = tag_of(spec, cmd) {
                params.push(format!("{tag}: u32"));
            }
            if fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()) {
                params.push(format!("e: {}", pascal(cmd)));
            }
            if names_mem(cmd) {
                params.push("named: U8Array".into());
            }
            out.push(format!("  [Command.{cmd}]({}): {ret};", params.join(", ")));
        }
        out.push("}".into());
        let tag = if p.imports.iter().any(|(cmd, _)| tag_of(spec, cmd).is_some()) {
            "tag"
        } else {
            "_tag"
        };
        let f = if reads { "f" } else { "_f" };
        let named = if p.imports.iter().any(|(cmd, _)| names_mem(cmd)) {
            ", _pkt, named"
        } else {
            ""
        };
        out.extend([
            String::new(),
            format!(
                "/** `h` as the protocol the runtime routes the {} protocol's majors to: each",
                p.name
            ),
            " * command read and handed to its member, whose refusal is answered with its".into(),
            " * errno; ENOSYS for one the protocol does not carry. */".into(),
            format!("export function {name}_imports(h: {iface}): Protocol {{"),
            format!("  const on: OnImport = ({tag}, cmd, {f}{named}) => {{"),
            "    try {".into(),
            "      switch (cmd) {".into(),
        ]);
        for (cmd, _) in &p.imports {
            let mut args = Vec::new();
            if tag_of(spec, cmd).is_some() {
                args.push("tag".to_string());
            }
            if fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()) {
                args.push(format!("read[Command.{cmd}](need(f))"));
            }
            if names_mem(cmd) {
                args.push("need(named)".into());
            }
            out.push(format!("        case Command.{cmd}:"));
            out.push(format!(
                "          return h[Command.{cmd}]({});",
                args.join(", ")
            ));
        }
        out.extend([
            "        default:".into(),
            "          return Errno.ENOSYS;".into(),
            "      }".into(),
            "    } catch (err) {".into(),
            "      return refusal_of(err);".into(),
            "    }".into(),
            "  };".into(),
            "  return { majors: MAJORS, on };".into(),
            "}".into(),
        ]);
    }
    if !p.imports.is_empty() {
        imports.push("import * as Errno from \"@xpute/kit/status/errno.spec.ts\";".into());
    }
    if out.iter().any(|l| l.contains(": Branch")) {
        imports.push("import type { Branch } from \"@xpute/kit/wire/xtp/mod.ts\";".into());
    }
    if out.iter().any(|l| l.contains("named: U8Array")) {
        imports.push("import type { U8Array } from \"@xpute/kit/abi/array.ts\";".into());
    }
    if out.iter().any(|l| l.contains("need(f)")) {
        imports.push("import { MarshalError } from \"@xpute/kit/status/error.ts\";".into());
        out.extend([
            String::new(),
            "/** A packet the command carries, which a frame without one breaks. */".into(),
            "function need<T>(f: T | null): T {".into(),
            "  if (f === null) throw new MarshalError(Errno.EBADMSG);".into(),
            "  return f;".into(),
            "}".into(),
        ]);
    }
    if !types.is_empty() {
        imports.push(format!(
            "import type {{ {} }} from \"{runtime}\";",
            types.join(", ")
        ));
    }
    if out.iter().any(|l| l.contains("CMD(major, ")) {
        values.push("CMD");
    }
    if !values.is_empty() {
        imports.push(format!(
            "import {{ {} }} from \"{runtime}\";",
            values.join(", ")
        ));
    }
    Ok((imports, out))
}
