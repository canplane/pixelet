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

fn fields_of<'a>(spec: &'a Spec, name: &str, export: bool) -> Option<&'a [Field]> {
    let c = spec.commands().ok()?.into_iter().find(|c| c.name == name)?;
    if export {
        c.pkt.as_deref()
    } else {
        c.import.as_deref()
    }
}

/// Writes the protocol's Rust module: a function for each `import`, and a
/// trait the program implements for its `export`s, with the dispatch that
/// reads each export's packet and calls the trait's method.
pub(super) fn rs(spec: &Spec) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let p = &spec.protocol;
    let name = module_name(p);
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
    out.push("    use xpute_core::ipc::door::Outcome;".into());
    out.push("    use xpute_core::machine;".into());
    out.push("    use xpute_kit::status::errno::Errno;".into());
    for (cmd, call) in &p.imports {
        let fields = fields_of(spec, cmd, false).unwrap_or(&[]);
        let ty = format!("{}{}", pascal(cmd), lifetime(spec, fields, false)?);
        let f = cmd.to_lowercase();
        let (param, arg) = if fields.is_empty() {
            (String::new(), format!("&{} {{}}", pascal(cmd)))
        } else {
            (format!("e: &{ty}"), "e".to_string())
        };
        out.push(String::new());
        if *call {
            out.push(format!(
                "    /// Command::{cmd}, answered by the runtime before this returns."
            ));
            out.push(format!("    pub fn {f}({param}) -> Result<u32, Errno> {{"));
            out.push(format!("        machine::door().call({arg})"));
        } else {
            out.push(format!("    /// Command::{cmd}, posted for the runtime."));
            out.push(format!("    pub fn {f}({param}) {{"));
            out.push(format!("        machine::door().send({arg});"));
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
        out.push(
            "    /// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry."
                .into(),
        );
        let reads = p
            .exports
            .iter()
            .any(|(cmd, _, _)| fields_of(spec, cmd, true).is_some());
        out.push(format!(
            "    pub fn apply<H: Exports>(h: &mut H, cmd: u32, {}: &Option<&[u8]>) -> Outcome {{",
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

/// Gives `<'_>` where the type borrows: with `read`, a packet read in place,
/// as an export's is; without it, an import that writes a borrowed field.
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

/// Writes one way of sending, as a type: each command that goes that way, by
/// its number, and the fields its packet takes in order.
fn ts_fields(spec: &Spec, ty: &str, cmds: &[&str]) -> Result<Vec<String>, String> {
    if cmds.is_empty() {
        return Ok(vec![format!("export type {ty} = Record<never, never>;")]);
    }
    let mut out = vec![format!("export type {ty} = {{")];
    for cmd in cmds {
        let fields = fields_of(spec, cmd, true).unwrap_or(&[]);
        let doc: Vec<String> = fields
            .iter()
            .filter_map(|f| {
                let d = f.doc.as_ref()?;
                Some(format!("@param {} {}", f.name, d.join(" ")))
            })
            .collect();
        if !doc.is_empty() {
            out.extend(ts_doc(&doc, "  "));
        }
        let mut args = Vec::new();
        for f in fields {
            args.push(format!("{}: {}", f.name, super::ts::ts_arg(spec, f)?));
        }
        out.push(format!("  [Command.{cmd}]: [{}];", args.join(", ")));
    }
    out.push("};".into());
    Ok(out)
}

/// Writes the TypeScript binding's imports and body: the fields of what the
/// runtime sends, and its handlers.
pub(super) fn ts(spec: &Spec, runtime: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut imports: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut runtime_types = vec!["Protocol", "OnImport"];
    let mut sends = false;
    let p = &spec.protocol;
    let name = module_name(p);
    if !p.exports.is_empty() {
        sends = true;
        let ty = pascal(&p.name);
        let pick = |want: &dyn Fn(bool, bool) -> bool| -> Vec<&str> {
            p.exports
                .iter()
                .filter(|(_, call, ack)| want(*call, *ack))
                .map(|(cmd, _, _)| cmd.as_str())
                .collect()
        };
        out.extend([
            String::new(),
            format!(
                "/** What the runtime sends on the {} protocol: each command by its number, and",
                p.name
            ),
            " * the fields its packet takes in order. It is sent, requested and answered".into(),
            " * later, or called and answered before it returns. */".into(),
        ]);
        out.extend(ts_fields(
            spec,
            &format!("{ty}Send"),
            &pick(&|c, a| !c && !a),
        )?);
        out.extend(ts_fields(
            spec,
            &format!("{ty}Request"),
            &pick(&|c, a| !c && a),
        )?);
        out.extend(ts_fields(spec, &format!("{ty}Call"), &pick(&|c, _| c))?);
        out.push(String::new());
        out.push(format!("/** The {} protocol over a link. */", p.name));
        out.push(format!(
            "export type {ty}Protocol = Protocol<{ty}Send, {ty}Request, {ty}Call>;"
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
                "/** What the program does with each command the core posts on the {} protocol; a",
                p.name
            ),
            " * call's answer is its return. */".into(),
            format!("export interface {iface} {{"),
        ]);
        for (cmd, call) in &p.imports {
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
                out.push(format!("  [Command.{cmd}](e: {}): {ret};", pascal(cmd)));
            } else {
                out.push(format!("  [Command.{cmd}](): {ret};"));
            }
        }
        out.push("}".into());
        out.extend([
            String::new(),
            format!(
                "/** `h` as the handler a runtime routes the {} protocol's commands to, each one read and",
                p.name
            ),
            " * handed to its member; ENOSYS for one the protocol does not carry. */".into(),
            format!("export function {name}_imports(h: {iface}): OnImport {{"),
            format!(
                "  return (_tag, cmd, {}) => {{",
                if reads { "f" } else { "_f" }
            ),
            "    switch (cmd) {".into(),
        ]);
        for (cmd, _) in &p.imports {
            let arg = if fields_of(spec, cmd, false).is_some_and(|f| !f.is_empty()) {
                format!("read[Command.{cmd}](need(f))")
            } else {
                String::new()
            };
            out.push(format!("      case Command.{cmd}:"));
            out.push(format!("        return h[Command.{cmd}]({arg});"));
        }
        out.extend([
            "      default:".into(),
            "        return Errno.ENOSYS;".into(),
            "    }".into(),
            "  };".into(),
            "}".into(),
        ]);
    }
    if !p.mem.is_empty() {
        out.extend([
            String::new(),
            format!(
                "/** The core's memory a command on the {} protocol names, where it is and how",
                p.name
            ),
            " * long; null for none. */".into(),
            format!("export function {name}_mem(cmd: u32, f: BranchCursor | null): [u32, u32] | null {{"),
            "  switch (cmd) {".into(),
        ]);
        for (cmd, [at, len]) in &p.mem {
            out.push(format!("    case Command.{cmd}: {{"));
            out.push(format!("      const e = read[Command.{cmd}](need(f));"));
            out.push(format!("      return [e.{at}, e.{len}];"));
            out.push("    }".into());
        }
        out.extend([
            "    default:".into(),
            "      return null;".into(),
            "  }".into(),
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
        runtime_types.retain(|t| *t != "OnImport");
    }
    if !sends {
        runtime_types.retain(|t| *t != "Protocol");
    }
    if !runtime_types.is_empty() {
        imports.push(format!(
            "import type {{ {} }} from \"{runtime}\";",
            runtime_types.join(", ")
        ));
    }
    Ok((imports, out))
}
