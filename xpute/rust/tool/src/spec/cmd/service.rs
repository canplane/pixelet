// xpute-tool/spec/cmd/service.rs

//! A protocol's binding to xpute's two ends, over its messages: on the host
//! a stub per `export`, bound to a host, and a handler per `import`; on the
//! guest a function per `import`, and a trait the program implements for its
//! `export`s. A
//! host routes a frame to the binding by its command's major.

use super::{Field, Kind, Protocol, Spec};
use crate::spec::{pascal, rs_doc, ts_doc};

fn module_name(p: &Protocol) -> String {
    p.name.to_lowercase()
}

fn fields_of<'a>(spec: &'a Spec, name: &str, export: bool) -> Option<&'a [Field]> {
    let c = spec.commands().ok()?.into_iter().find(|c| c.name == name)?;
    if export {
        c.pkt.as_deref()
    } else {
        c.event.as_deref()
    }
}

/// The protocol's module: a function for each `import`, and a trait the
/// program implements for its `export`s, with the dispatch that reads each packet and
/// calls it.
pub(super) fn rs(spec: &Spec) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let p = &spec.protocol;
    let name = module_name(p);
    out.push(String::new());
    out.extend(rs_doc(
        &[format!(
            "The {} protocol's guest end: what it posts to the host and what it hears from it.",
            p.name
        )],
        "",
        "///",
    ));
    out.push(format!("pub mod {name} {{"));
    out.push("    use super::*;".into());
    out.push("    use xpute_guest::ipc::door::Outcome;".into());
    out.push("    use xpute_guest::machine;".into());
    out.push("    use xpute_kit::status::errno::Errno;".into());
    for (cmd, call) in &p.imports {
        let fields = fields_of(spec, cmd, false).unwrap_or(&[]);
        let ty = format!("{}Event{}", pascal(cmd), lifetime(spec, fields, false)?);
        let f = cmd.to_lowercase();
        let (param, arg) = if fields.is_empty() {
            (String::new(), format!("&{}Event {{}}", pascal(cmd)))
        } else {
            (format!("e: &{ty}"), "e".to_string())
        };
        out.push(String::new());
        if *call {
            out.push(format!(
                "    /// Command::{cmd}, answered by the host before this returns."
            ));
            out.push(format!("    pub fn {f}({param}) -> Result<u32, Errno> {{"));
            out.push(format!("        machine::door().call({arg})"));
        } else {
            out.push(format!("    /// Command::{cmd}, posted for the host."));
            out.push(format!("    pub fn {f}({param}) {{"));
            out.push(format!("        machine::door().send({arg});"));
        }
        out.push("    }".into());
    }
    if !p.exports.is_empty() {
        out.push(String::new());
        out.push("    /// What the program does with each command the host sends.".into());
        out.push("    pub trait Exports {".into());
        for (cmd, _) in &p.exports {
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
        out.push(
            "    /// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry."
                .into(),
        );
        let reads = p
            .exports
            .iter()
            .any(|(cmd, _)| fields_of(spec, cmd, true).is_some());
        out.push(format!(
            "    pub fn apply<H: Exports>(h: &mut H, cmd: u32, {}: &Option<&[u8]>) -> Outcome {{",
            if reads { "pkt" } else { "_pkt" }
        ));
        out.push("        match cmd {".into());
        for (cmd, _) in &p.exports {
            let f = cmd.to_lowercase();
            let arm = match fields_of(spec, cmd, true) {
                Some(_) => format!(
                    "h.{f}({}::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?)",
                    pascal(cmd)
                ),
                None => format!("h.{f}()"),
            };
            out.push(format!(
                "            x if x == Command::{cmd} as u32 => {arm},"
            ));
        }
        out.push("            _ => Err(MarshalError::new(Errno::ENOSYS)),".into());
        out.push("        }".into());
        out.push("    }".into());
    }
    out.push("}".into());
    Ok(out.join("\n") + "\n")
}

/// `<'_>` where the type borrows: a packet read in place (`export`), an event
/// that writes a borrowed field (`import`).
fn lifetime(spec: &Spec, fields: &[Field], read: bool) -> Result<&'static str, String> {
    let borrows = if read {
        let mut b = false;
        for f in fields {
            b |= f.list || spec.kind(&f.ty)? == Kind::Str;
        }
        b
    } else {
        super::rs::rs_borrows(spec, fields)?
    };
    Ok(if borrows { "<'_>" } else { "" })
}

/// The TypeScript's imports and body: the host's stubs and its handlers.
pub(super) fn ts(spec: &Spec, host: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut imports: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut host_types = vec!["Host", "OnEvent"];
    let mut sends = false;
    let p = &spec.protocol;
    let name = module_name(p);
    if !p.exports.is_empty() {
        sends = true;
        out.extend([
            String::new(),
            format!(
                "/** What the host sends on the {} protocol, bound to `host`: each",
                p.name
            ),
            " * command rung, or called and answered before it returns. */".into(),
            format!("export function {name}_exports(host: Host) {{"),
            "  return {".into(),
        ]);
        for (cmd, call) in &p.exports {
            let f = cmd.to_lowercase();
            let fields = fields_of(spec, cmd, true);
            let mut params = Vec::new();
            let mut args = String::new();
            for fl in fields.unwrap_or(&[]) {
                params.push(format!("{}: {}", fl.name, super::ts::ts_type(spec, fl)?));
                args += &format!(", {}", fl.name);
            }
            let (verb, ret) = if *call {
                ("call", "i32")
            } else {
                ("send", "void")
            };
            let encode = match fields {
                Some(_) => format!(", (r) => encode_{f}(r{args})"),
                None => String::new(),
            };
            let ring = format!(
                "({}): {ret} =>\n      host.{verb}(Command.{cmd}{encode})",
                params.join(", ")
            );
            out.push(format!("    {f}: {ring},"));
        }
        out.push("  };".into());
        out.push("}".into());
        out.push(String::new());
        out.push(format!("/** The {} protocol's stubs, bound. */", p.name));
        out.push(format!(
            "export type {}Exports = ReturnType<typeof {name}_exports>;",
            pascal(&p.name)
        ));
    }
    if !p.imports.is_empty() {
        let iface = format!("{}Imports", pascal(&p.name));
        let reads = p
            .imports
            .iter()
            .any(|(cmd, _)| fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()));
        out.extend([
            String::new(),
            format!(
                "/** What the program does with each command the guest posts on the {} protocol; a",
                p.name
            ),
            " * call's answer is its return. */".into(),
            format!("export interface {iface} {{"),
        ]);
        for (cmd, call) in &p.imports {
            let f = cmd.to_lowercase();
            let ret = if *call { "i32" } else { "void" };
            if let Some(doc) = spec
                .commands()?
                .into_iter()
                .find(|c| c.name == *cmd)
                .and_then(|c| c.doc.as_ref())
            {
                out.extend(ts_doc(doc, "  "));
            }
            if fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()) {
                out.push(format!("  {f}(e: {}Event): {ret};", pascal(cmd)));
            } else {
                out.push(format!("  {f}(): {ret};"));
            }
        }
        out.push("}".into());
        out.extend([
            String::new(),
            format!(
                "/** `h` as the handler a host routes the {} protocol's commands to, each one read and",
                p.name
            ),
            " * handed to its member; ENOSYS for one the protocol does not carry. */".into(),
            format!("export function {name}_imports(h: {iface}): OnEvent {{"),
            format!(
                "  return (_tag, cmd, {}) => {{",
                if reads { "f" } else { "_f" }
            ),
            "    switch (cmd) {".into(),
        ]);
        for (cmd, _) in &p.imports {
            let f = cmd.to_lowercase();
            let arg = if fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()) {
                format!("read_{f}(need(f))")
            } else {
                String::new()
            };
            out.push(format!("      case Command.{cmd}:"));
            out.push(format!("        return h.{f}({arg});"));
        }
        out.extend([
            "      default:".into(),
            "        return Errno.ENOSYS;".into(),
            "    }".into(),
            "  };".into(),
            "}".into(),
        ]);
    }
    let has_imports = !spec.protocol.imports.is_empty();
    if has_imports {
        imports.push("import * as Errno from \"@xpute/kit/status/errno.spec.ts\";".into());
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
    if !has_imports {
        host_types.retain(|t| *t != "OnEvent");
    }
    if !sends {
        host_types.retain(|t| *t != "Host");
    }
    if !host_types.is_empty() {
        imports.push(format!(
            "import type {{ {} }} from \"{host}\";",
            host_types.join(", ")
        ));
    }
    Ok((imports, out))
}
