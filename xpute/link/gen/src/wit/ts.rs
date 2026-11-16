// xpute-link-gen/wit/ts.rs

//! The link's bindings in TypeScript (wit.rs): a module per interface, its
//! types, each storing itself into a packet and loading itself from one
//! (@xpute/link cabi.ts), the side that implements it and the side that calls
//! it.

use std::fmt::Write as _;

use wit_parser::{Function, Type, TypeDefKind, TypeId, TypeOwner};

use xpute_gen::spec::{of_ident, screaming, snake, ts_enum, ts_ident, ts_number};

use super::{
    Gen, Kind, Obj, Role, cases, discriminant, escaped, is_async, kind, members, method_const,
    method_name, object_method, params, plus, rec_layout, side_trait, ts_given, type_name,
    variant_layout,
};

/// The TypeScript modules the bindings take items from, beside the link's.
const TS_ERRNO: &str = "@xpute/kit/status/errno.spec.ts";
const TS_ERROR: &str = "@xpute/kit/status/error.ts";
const TS_WORD: &str = "@xpute/kit/abi/word.ts";

/// A WIT name as a TypeScript identifier.
fn ts_name(n: &str) -> String {
    escaped(ts_ident(&snake(n)), ts_given)
}

impl Gen<'_> {
    /// `name` of the TypeScript module `module`, which the module imports by
    /// its name, as a type alone where `is_type`.
    fn ts_use(&self, module: &str, name: &str, is_type: bool) -> String {
        let mut uses = self.ts_uses.borrow_mut();
        let t = uses
            .entry(module.into())
            .or_default()
            .entry(name.into())
            .or_insert(is_type);
        *t &= is_type;
        name.into()
    }

    /// The size and alignment of `k` as TypeScript writes them: a record's or
    /// a variant's own constants, any other's numbers.
    fn ts_layout(&self, k: &Kind) -> Result<(String, String), String> {
        Ok(match k {
            Kind::Record(t) | Kind::Variant(t) => {
                (self.ts_const(*t, "SZ"), self.ts_const(*t, "ALIGN"))
            }
            _ => {
                let (sz, align) = self.layout(k)?;
                (sz.to_string(), align.to_string())
            }
        })
    }

    fn ts_params(&self, f: &Function) -> Result<Vec<(String, Kind)>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        params(f)
            .map(|p| Ok((ts_name(&p.name), kind(self.r, &p.ty, &at)?)))
            .collect()
    }

    /// A function's `result` returns its ok value and throws its error case,
    /// as jco's bindings do.
    fn ts_ret(&self, f: &Function) -> Result<String, String> {
        Ok(match self.ret(f)? {
            Some(Kind::Result(ok, _)) => ok.map_or("void".into(), |k| self.ts_ty(&k)),
            Some(k) => self.ts_ty(&k),
            None => "void".into(),
        })
    }

    /// A `result` of `ok` and `err`, tagged as it is stored, from `x`, an ok
    /// value, and from `e`, a `ComponentError`.
    fn ts_tagged(
        &self,
        ok: &Option<Box<Kind>>,
        err: &Option<Box<Kind>>,
        x: &str,
        e: &str,
    ) -> (String, String) {
        let ok = match ok {
            Some(_) => format!("{{ tag: \"ok\", val: {x} }}"),
            None => "{ tag: \"ok\" }".into(),
        };
        let err = match err {
            Some(k) => format!("{{ tag: \"err\", val: {e}.payload as {} }}", self.ts_ty(k)),
            None => "{ tag: \"err\" }".into(),
        };
        (ok, err)
    }

    /// Stores `e`, a value of `k`, at `at` with the writer `w`.
    fn ts_store(&self, w: &str, at: &str, e: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool => format!("{w}.bool({at}, {e});"),
            Kind::Number(n) => format!("{w}.{}({at}, {e});", n.rs),
            Kind::Handle => format!("{w}.u32({at}, {e});"),
            Kind::Enum(t) => format!("{w}.{}({at}, {e});", discriminant(self.case_cnt(*t))?.rs),
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(_) => format!("{w}.arr({at}, {e});"),
            Kind::List(el) => {
                let (sz, align) = self.ts_layout(el)?;
                // A list inside a list takes names of its own depth.
                let d = self.depth.get();
                self.depth.set(d + 1);
                let inner = self.ts_store(w, &format!("b{d} + i{d} * {sz}"), &format!("x{d}"), el);
                self.depth.set(d);
                format!(
                    "{{ const b{d} = {w}.list({at}, {e}.length, {sz}, {align}); if (b{d} !== null) {e}.forEach((x{d}, i{d}) => {{ {} }}); }}",
                    inner?
                )
            }
            Kind::Record(t) | Kind::Variant(t) => {
                format!("{}({w}, {at}, {e});", self.ts_path(*t, "store_"))
            }
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)])?;
                format!(
                    "if ({e} !== null) {{ {w}.u8({at}, 1); {} }}",
                    self.ts_store(w, &plus(at, off), e, el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>| match k {
                    Some(k) => self.ts_store(w, &plus(at, off), &format!("{e}.val"), k),
                    None => Ok(String::new()),
                };
                format!(
                    "if ({e}.tag === \"ok\") {{ {} }} else {{ {w}.u8({at}, 1); {} }}",
                    case(ok)?,
                    case(err)?
                )
            }
        })
    }

    /// Loads a value of `k` at `at` with the reader `r`.
    fn ts_load(&self, r: &str, at: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool => format!("{r}.bool({at})"),
            Kind::Number(n) => format!("{r}.{}({at})", n.rs),
            Kind::Handle => format!("{r}.u32({at})"),
            Kind::Str => format!("{r}.str({at})"),
            Kind::Array(n) => format!("{r}.arr({at}, {})", n.arr),
            Kind::List(el) => {
                let (sz, align) = self.ts_layout(el)?;
                format!(
                    "{r}.map({at}, {sz}, {align}, (at) => {})",
                    self.ts_load(r, "at", el)?
                )
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => {
                format!("{}({r}, {at})", self.ts_path(*t, "load_"))
            }
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)])?;
                format!(
                    "({r}.case({at}, 2) === 0 ? null : {})",
                    self.ts_load(r, &plus(at, off), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |tag: &str, k: &Option<Box<Kind>>| -> Result<String, String> {
                    Ok(match k {
                        Some(k) => format!(
                            "{{ tag: \"{tag}\" as const, val: {} }}",
                            self.ts_load(r, &plus(at, off), k)?
                        ),
                        None => format!("{{ tag: \"{tag}\" as const }}"),
                    })
                };
                format!(
                    "({r}.case({at}, 2) === 0 ? {} : {})",
                    case("ok", ok)?,
                    case("err", err)?
                )
            }
        })
    }

    /// The args `ps` as a packet: none, or their tuple's size and what writes
    /// it.
    fn ts_args(&self, ps: &[(String, Kind)]) -> Result<String, String> {
        if ps.is_empty() {
            return Ok("null".into());
        }
        let (offs, sz) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
        let writes = ps
            .iter()
            .zip(&offs)
            .map(|((n, k), at)| self.ts_store("w_", &at.to_string(), n, k))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(format!("[{sz}, (w_) => {{ {} }}]", writes.join(" ")))
    }

    /// The implementing side of an object: its interface, and the object over
    /// it.
    fn ts_export(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        let link = self.link_ts;
        let t = side_trait(o, "Host");
        let _ = writeln!(out, "export interface {t} {{");
        for f in &o.methods {
            let mut sig: Vec<String> = self
                .ts_params(f)?
                .iter()
                .map(|(n, k)| format!("{n}: {}", self.ts_ty(k)))
                .collect();
            let r = self.ts_ret(f)?;
            // An async method stops on `signal_` where its caller cancels it
            // and it can; one that can't leaves it unread.
            if is_async(f) {
                sig.push("signal_: AbortSignal".into());
            }
            let r = if is_async(f) {
                format!("Promise<{r}>")
            } else {
                r
            };
            let _ = writeln!(
                out,
                "  {}({}): {r};",
                ts_name(method_name(f)),
                sig.join(", ")
            );
        }
        // A resource's implementation is told its drop, as Rust's is by its
        // Drop; an interface is never dropped.
        if o.resource {
            let _ = writeln!(out, "  drop(): void;");
        }
        let fname = if o.resource {
            ts_name(&o.name)
        } else {
            "object".into()
        };
        let pick = if o.methods.iter().any(|f| is_async(f)) {
            format!(
                "l: Pick<{}, \"task_return\" | \"on_cancel\">, ",
                self.ts_use(link, "Callee", true)
            )
        } else {
            String::new()
        };
        let refusal_of = self.ts_use(link, "refusal_of", false);
        // A request's tag is read where a method answers later, the args
        // where one takes any, and the writer of the answer where one
        // answers a value at once; each other is named unread, as Rust's is.
        let mut answers = false;
        for f in &o.methods {
            answers |= !is_async(f) && self.ret(f)?.is_some();
        }
        let tag = if o.methods.iter().any(|f| is_async(f)) {
            "tag"
        } else {
            "_tag"
        };
        let args = if o.methods.iter().any(|f| params(f).next().is_some()) {
            "args"
        } else {
            "_args"
        };
        let ret = if answers { "ret" } else { "_ret" };
        let _ = writeln!(
            out,
            "}}\n\n/** The object over `implementation`. */\n{}function {fname}({pick}implementation: {t}): {} {{\n  return {{\n    call(m, {tag}, {args}, {ret}) {{\n      try {{\n        switch (m) {{",
            if o.resource { "export " } else { "" },
            self.ts_use(link, "LinkObject", true)
        );
        for f in &o.methods {
            let ps = self.ts_params(f)?;
            let (offs, _) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
            let mut loads = ps
                .iter()
                .zip(&offs)
                .map(|((_, kd), at)| self.ts_load("args", &at.to_string(), kd))
                .collect::<Result<Vec<_>, String>>()?;
            if is_async(f) {
                loads.push("stop.signal".into());
            }
            let call = format!(
                "implementation.{}({})",
                ts_name(method_name(f)),
                loads.join(", ")
            );
            let rk = self.ret(f)?;
            // A later answer's value, written as its frame is laid.
            let data = match &rk {
                None => "null".to_string(),
                Some(k) => format!(
                    "[{}, (w) => {{ {} }}]",
                    self.ts_layout(k)?.0,
                    self.ts_store("w", "0", "v", k)?
                ),
            };
            // A value at once, written in the frame its args came in.
            let put = match &rk {
                None => String::new(),
                Some(k) => format!(
                    "ret.start({});\n            {}\n            ",
                    self.ts_layout(k)?.0,
                    self.ts_store("ret", "0", "v", k)?
                ),
            };
            let c = method_const(o, f, ts_given);
            let _ = writeln!(out, "          case {c}: {{");
            if let Some(k @ Kind::Result(ok, err)) = &rk {
                let ce = self.ts_use(link, "ComponentError", false);
                let ty = self.ts_ty(k);
                let (tag_ok, tag_err) = self.ts_tagged(ok, err, "x", "e");
                let bind = if ok.is_some() { "(x)" } else { "()" };
                if is_async(f) {
                    let _ = writeln!(
                        out,
                        "            const stop = new AbortController();\n            {call}.then(\n              {bind} => {{\n                const v = {tag_ok} as {ty};\n                l.task_return(tag, 0, {data});\n              }},\n              (e: unknown) => {{\n                if (!(e instanceof {ce})) return l.task_return(tag, {refusal_of}(e));\n                const v = {tag_err} as {ty};\n                l.task_return(tag, 0, {data});\n              }},\n            );\n            l.on_cancel(tag, () => stop.abort());\n            return {};\n          }}",
                        self.ts_use(TS_ERRNO, "ERRNO_EINPROGRESS", false)
                    );
                } else {
                    let set = if ok.is_some() {
                        format!("const x = {call};\n              v = {tag_ok};")
                    } else {
                        format!("{call};\n              v = {tag_ok};")
                    };
                    let _ = writeln!(
                        out,
                        "            let v: {ty};\n            try {{\n              {set}\n            }} catch (e) {{\n              if (!(e instanceof {ce})) throw e;\n              v = {tag_err};\n            }}\n            {put}return 0;\n          }}"
                    );
                }
            } else if is_async(f) {
                let bind = if rk.is_none() { "()" } else { "(v)" };
                let _ = writeln!(
                    out,
                    "            const stop = new AbortController();\n            {call}.then(\n              {bind} => l.task_return(tag, 0, {data}),\n              (e: unknown) => l.task_return(tag, {refusal_of}(e)),\n            );\n            l.on_cancel(tag, () => stop.abort());\n            return {};\n          }}",
                    self.ts_use(TS_ERRNO, "ERRNO_EINPROGRESS", false)
                );
            } else if rk.is_none() {
                let _ = writeln!(
                    out,
                    "            {call};\n            return 0;\n          }}"
                );
            } else {
                let _ = writeln!(
                    out,
                    "            const v = {call};\n            {put}return 0;\n          }}"
                );
            }
        }
        let drop = if o.resource {
            "drop() {\n      implementation.drop();\n    }"
        } else {
            "drop() {}"
        };
        let _ = writeln!(
            out,
            "          default:\n            return {};\n        }}\n      }} catch (e) {{\n        return {refusal_of}(e);\n      }}\n    }},\n    {drop},\n  }};\n}}\n",
            self.ts_use(TS_ERRNO, "ERRNO_ENOSYS", false)
        );
        if !o.resource {
            let ln = self.ts_use(link, "Callee", true);
            let pick = if pick.is_empty() {
                format!("l: Pick<{ln}, \"mount_named\">")
            } else {
                format!("l: Pick<{ln}, \"mount_named\" | \"task_return\" | \"on_cancel\">")
            };
            let pass = if o.methods.iter().any(|f| is_async(f)) {
                "l, "
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "/** Mounts `implementation` under the interface's handle. */\nexport function mount({pick}, implementation: {t}): void {{\n  l.mount_named(HANDLE, object({pass}implementation));\n}}\n"
            );
        }
        Ok(())
    }

    /// The calling side of an object: its methods as functions, each taking
    /// first the link a singleton is called over, or a resource's reference.
    fn ts_import(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        let link = self.link_ts;
        let (by, to) = if o.resource {
            (
                format!("ref_: {}", self.ts_use(link, "Ref", true)),
                "ref_.link.send(ref_.handle",
            )
        } else {
            (
                format!("link_: {}", self.ts_use(link, "Sender", true)),
                "link_.send(HANDLE",
            )
        };
        let err = self.ts_use(TS_ERROR, "XputeError", false);
        for f in &o.methods {
            let ps = self.ts_params(f)?;
            let mut sig = vec![by.clone()];
            sig.extend(ps.iter().map(|(n, k)| format!("{n}: {}", self.ts_ty(k))));
            let rk = self.ret(f)?;
            let ret = self.ts_ret(f)?;
            let lift = |k: &Kind| -> Result<String, String> {
                Ok(format!(
                    "\n  if (a_.data === null) throw new {}({});\n  const r_ = new {}(a_.data);",
                    self.ts_use(TS_ERROR, "XputeError", false),
                    self.ts_use(TS_ERRNO, "ERRNO_EBADMSG", false),
                    self.ts_use(self.cabi_ts, "Lift", false)
                ) + &match k {
                    Kind::Result(ok, e) => format!(
                        "\n  const v_ = {};\n  if (v_.tag === \"err\") throw new {}({});\n  return{};",
                        self.ts_load("r_", "0", k)?,
                        self.ts_use(link, "ComponentError", false),
                        if e.is_some() { "v_.val" } else { "undefined" },
                        if ok.is_some() { " v_.val" } else { "" }
                    ),
                    _ => format!("\n  return {};", self.ts_load("r_", "0", k)?),
                })
            };
            let take = match &rk {
                None => String::new(),
                Some(k) => lift(k)?,
            };
            let name = ts_name(&object_method(o, f));
            let _ = writeln!(
                out,
                "export async function {name}({}): Promise<{ret}> {{\n  const a_ = await {to}, {}, {});\n  if (a_.result !== 0) throw new {err}(a_.result);{take}\n}}\n",
                sig.join(", "),
                method_const(o, f, ts_given),
                self.ts_args(&ps)?
            );
        }
        Ok(())
    }

    /// The interface's TypeScript modules, by `role`, none for an interface
    /// of types alone: its functions' module, and its enums' module, empty
    /// where it has none.
    pub(super) fn ts_interface(
        &self,
        objs: &[Obj],
        role: Option<Role>,
    ) -> Result<(String, String), String> {
        let link = self.link_ts;
        let mut out = String::new();
        let mut enums = String::new();
        if !objs.is_empty() {
            let _ = writeln!(
                out,
                "/** The interface's handle, named by `{}` (frame.json5 `NAMED`). */\nexport const HANDLE: {} = {:#x};\n",
                self.at(),
                self.ts_use(link, "Handle", true),
                self.handle()
            );
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "/** `{}` */\nexport const {}: {} = {k};",
                    f.name,
                    method_const(o, f, ts_given),
                    self.ts_use(link, "Method", true)
                );
            }
        }
        out.push('\n');
        self.ts_types(&mut out, &mut enums)?;
        if let Some(role) = role {
            for o in objs {
                match role {
                    Role::Implement => self.ts_export(o, &mut out)?,
                    Role::Call => self.ts_import(o, &mut out)?,
                }
            }
        }
        Ok((out, enums))
    }

    /// The imports of a TypeScript module.
    pub(super) fn ts_imports(&self) -> String {
        let mut out = String::new();
        for (from, names) in self.ts_uses.borrow().iter() {
            if names.values().all(|t| *t) {
                let _ = writeln!(
                    out,
                    "import type {{ {} }} from \"{from}\";",
                    names.keys().cloned().collect::<Vec<_>>().join(", ")
                );
            } else {
                let names: Vec<String> = names
                    .iter()
                    .map(|(n, t)| if *t { format!("type {n}") } else { n.clone() })
                    .collect();
                let _ = writeln!(out, "import {{ {} }} from \"{from}\";", names.join(", "));
            }
        }
        out
    }

    /// The record's or variant's `t` constant `<T>_<suffix>`, imported from its
    /// module where that is another.
    fn ts_const(&self, t: TypeId, suffix: &str) -> String {
        let (m, n) = self.type_path(t, ts_given);
        let name = format!("{}_{suffix}", screaming(&n));
        match m.map(|m| m.ts) {
            Some(base) => self.ts_use(&format!("{base}.ts"), &name, false),
            None => name,
        }
    }

    /// A type's name, or one of its functions' (`load_`, `store_`): its own
    /// module's, or imported from its interface's. An enum's type is its
    /// enums' module's.
    fn ts_path(&self, t: TypeId, prefix: &str) -> String {
        let (m, n) = self.type_path(t, ts_given);
        let name = format!("{prefix}{n}");
        let is_type = prefix.is_empty();
        let base = m.map(|m| m.ts);
        if is_type && matches!(self.r.types[t].kind, TypeDefKind::Enum(_)) {
            let base = base.unwrap_or(format!("./{}", self.module_ts()));
            return self.ts_use(&format!("{base}.enum.ts"), &name, true);
        }
        match base {
            Some(base) => self.ts_use(&format!("{base}.ts"), &name, is_type),
            None => name,
        }
    }

    fn ts_ty(&self, k: &Kind) -> String {
        match k {
            Kind::Bool => "boolean".into(),
            // A 64-bit integer, a bigint, by the kit's word alias.
            Kind::Number(n) if ts_number(n.rs) == "bigint" => self.ts_use(TS_WORD, n.rs, true),
            Kind::Number(n) => ts_number(n.rs).into(),
            Kind::Handle => "number".into(),
            Kind::Str => "string".into(),
            Kind::Array(n) => format!("{}<ArrayBuffer>", n.arr),
            Kind::List(e) => format!("{}[]", self.ts_ty(e)),
            Kind::Option(e) => format!("({} | null)", self.ts_ty(e)),
            Kind::Result(ok, err) => {
                let case = |tag: &str, k: &Option<Box<Kind>>| match k {
                    Some(k) => format!("{{ tag: \"{tag}\"; val: {} }}", self.ts_ty(k)),
                    None => format!("{{ tag: \"{tag}\" }}"),
                };
                format!("({} | {})", case("ok", ok), case("err", err))
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => self.ts_path(*t, ""),
        }
    }

    /// The interface's types into `out`, each enum's members into `enums`.
    fn ts_types(&self, out: &mut String, enums: &mut String) -> Result<(), String> {
        let at = self.at();
        for &t in self.r.interfaces[self.here].types.values() {
            let td = &self.r.types[t];
            if !matches!(td.owner, TypeOwner::Interface(i) if i == self.here) {
                continue;
            }
            let n = type_name(self.r, t, ts_given);
            let up = screaming(&n);
            match &td.kind {
                // An enum is read as one of its members, or refused.
                TypeDefKind::Enum(e) => {
                    let members = members(e);
                    let _ = writeln!(enums, "{}\n", ts_enum(&n, &members).join("\n"));
                    let own = format!("./{}.enum.ts", self.module_ts());
                    let _ = writeln!(
                        out,
                        "export function load_{n}(r: {}, at: number): {} {{\n  const v = {}(r.case(at, {}));\n  if (v === null) throw new {}({});\n  return v;\n}}\n",
                        self.ts_use(self.cabi_ts, "Lift", true),
                        self.ts_path(t, ""),
                        self.ts_use(&own, &of_ident(&n), false),
                        e.cases.len(),
                        self.ts_use(TS_ERROR, "XputeError", false),
                        self.ts_use(TS_ERRNO, "ERRNO_EBADMSG", false)
                    );
                }
                TypeDefKind::Record(_) => {
                    let fields = self.fields(t)?;
                    let (offs, sz, align) = rec_layout(&self.field_layouts(t)?);
                    let _ = writeln!(out, "export interface {n} {{");
                    for (f, k) in &fields {
                        let _ = writeln!(out, "  {}: {};", ts_name(f), self.ts_ty(k));
                    }
                    let writes = fields
                        .iter()
                        .zip(&offs)
                        .map(|((f, k), &off)| {
                            self.ts_store("w", &plus("at", off), &format!("v.{}", ts_name(f)), k)
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let _ = writeln!(
                        out,
                        "}}\n\nexport const {up}_SZ = {sz};\nexport const {up}_ALIGN = {align};\n\nexport function store_{n}(w: {}, at: number, v: {n}): void {{\n  {}\n}}\n\nexport function load_{n}(r: {}, at: number): {n} {{\n  return {{",
                        self.ts_use(self.cabi_ts, "Lower", true),
                        writes.join("\n  "),
                        self.ts_use(self.cabi_ts, "Lift", true)
                    );
                    for ((f, k), &off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            out,
                            "    {}: {},",
                            ts_name(f),
                            self.ts_load("r", &plus("at", off), k)?
                        );
                    }
                    let _ = writeln!(out, "  }};\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let cases = cases(self.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&self.payload_layouts(t)?)?;
                    let d = discriminant(cases.len())?.rs;
                    let alts: Vec<String> = cases
                        .iter()
                        .map(|(c, k)| match k {
                            Some(k) => format!("{{ tag: \"{c}\"; val: {} }}", self.ts_ty(k)),
                            None => format!("{{ tag: \"{c}\" }}"),
                        })
                        .collect();
                    let _ = writeln!(
                        out,
                        "export type {n} = {};\n\nexport const {up}_SZ = {sz};\nexport const {up}_ALIGN = {align};\n\nexport function store_{n}(w: {}, at: number, v: {n}): void {{\n  switch (v.tag) {{",
                        alts.join(" | "),
                        self.ts_use(self.cabi_ts, "Lower", true)
                    );
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        let payload = match kd {
                            Some(kd) => self.ts_store("w", &plus("at", off), "v.val", kd)?,
                            None => String::new(),
                        };
                        let _ = writeln!(
                            out,
                            "    case \"{c}\":\n      w.{d}(at, {k});\n      {payload}\n      return;"
                        );
                    }
                    let _ = writeln!(
                        out,
                        "  }}\n}}\n\nexport function load_{n}(r: {}, at: number): {n} {{\n  switch (r.case(at, {})) {{",
                        self.ts_use(self.cabi_ts, "Lift", true),
                        cases.len()
                    );
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(
                                    out,
                                    "    case {k}:\n      return {{ tag: \"{c}\", val: {} }};",
                                    self.ts_load("r", &plus("at", off), kd)?
                                );
                            }
                            None => {
                                let _ = writeln!(
                                    out,
                                    "    case {k}:\n      return {{ tag: \"{c}\" }};"
                                );
                            }
                        }
                    }
                    let _ = writeln!(
                        out,
                        "  }}\n  throw new {}({});\n}}\n",
                        self.ts_use(TS_ERROR, "XputeError", false),
                        self.ts_use(TS_ERRNO, "ERRNO_EBADMSG", false)
                    );
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(
                        out,
                        "export type {n} = {};\n",
                        self.ts_ty(&kind(self.r, ty, &at)?)
                    );
                }
                _ => {
                    kind(self.r, &Type::Id(t), &at)?;
                }
            }
        }
        Ok(())
    }
}
