// xpute-link-gen/wit.rs

//! This writes the link's bindings from WIT (xpute/link/wit/types.wit), in Rust
//! and in TypeScript. For each interface it writes its methods' numbers, its
//! types, the side that serves it and the side that calls it: a singleton
//! interface's functions are one object's methods, under its named handle, the
//! FNV-1a hash of `package/interface` folded below frame.json's `NAMED`, and
//! each resource's methods another's. A method's number is its place among its
//! object's, in declaration order. No two interfaces a target reaches share a
//! named handle.
//!
//! It reads the subset the link's WIT names. Anything else is an error: a
//! resource's constructor or static, a variant case with no payload or two
//! that cross as one type, more than 255 methods to an object.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use wit_parser::{Function, FunctionKind, InterfaceId, Param, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use xpute_gen::spec::{mark, package, package_path, read, rs_enum, rs_ident, ts_enum, ts_ident, Output};

/// One package's bindings on one side, and where they go.
pub struct WitTarget<'a> {
    /// The package's WIT directory.
    pub wit: &'a str,
    /// The Rust module file, which declares a module per interface in the
    /// directory of its name.
    pub rs: Option<&'a str>,
    /// The TypeScript directory, a module per interface.
    pub ts: Option<&'a str>,
    /// The directory of the crate of the side the Rust runs on, whose `link`
    /// opens, calls and sends and whose `wait` waits; the link's own items
    /// come from xpute-link. The Rust is written per side, as two Rust sides
    /// share one process on a device; the TypeScript is side-agnostic, taking
    /// the link it sends on as an argument, from `link.ts` beside xpute:link's
    /// bindings.
    pub side: &'a str,
}

/// What a binding's header names as its writer.
const TOOL: &str = "xpute-link-gen wit.rs";

/// One kind of object an interface defines: its singleton, of the
/// interface's functions, or a resource, of its methods.
struct Obj<'a> {
    /// The interface's name, or the resource's.
    name: String,
    resource: bool,
    methods: Vec<&'a Function>,
}

fn objs<'a>(r: &'a Resolve, i: InterfaceId, at: &str, drop: u32) -> Result<Vec<Obj<'a>>, String> {
    let iface = &r.interfaces[i];
    let mut single = Obj {
        name: iface.name.clone().unwrap_or_default(),
        resource: false,
        methods: Vec::new(),
    };
    let mut res: BTreeMap<TypeId, Obj> = BTreeMap::new();
    let mut order = Vec::new();
    for &t in iface.types.values() {
        if matches!(r.types[t].kind, TypeDefKind::Resource) && r.types[t].owner == TypeOwner::Interface(i) {
            order.push(t);
            let name = r.types[t].name.clone().unwrap_or_default();
            res.insert(t, Obj { name, resource: true, methods: Vec::new() });
        }
    }
    for f in iface.functions.values() {
        match f.kind {
            FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => single.methods.push(f),
            FunctionKind::Method(t) | FunctionKind::AsyncMethod(t) => {
                res.get_mut(&t).ok_or(format!("{at}.{}: a method of no resource here", f.name))?.methods.push(f);
            }
            _ => return Err(format!("{at}.{}: the link carries no constructor, static, getter or setter", f.name)),
        }
    }
    for f in &single.methods {
        if ["open", "mount", "uri", "handle", "object"].contains(&method_name(f)) {
            return Err(format!("{at}.{}: a name the bindings keep for the module", f.name));
        }
    }
    // The names the generated code's own locals take, a lent span's among
    // them; and an async call lends one buffer, which its answer hands back.
    for f in iface.functions.values() {
        if let Some(p) = f.params.iter().find(|p| ["ref", "a", "b", "l", "w", "m", "r", "data", "tag", "args", "link", "handle", "lent", "stop", "signal"].contains(&p.name.as_str())) {
            return Err(format!("{at}.{}: `{}`, a name the bindings keep for their own", f.name, p.name));
        }
        let lents: Vec<String> = f.params.iter().filter(|p| is_lent(r, &p.ty)).map(|p| rs_ident(&p.name)).collect();
        if let Some(p) = f.params.iter().find(|p| lents.iter().any(|l| [format!("{l}_at"), format!("{l}_len")].contains(&rs_ident(&p.name)))) {
            return Err(format!("{at}.{}: `{}`, a name the bindings keep for a lent span", f.name, p.name));
        }
        if lents.len() > 1 && is_async(f) {
            return Err(format!("{at}.{}: an async call lends one buffer", f.name));
        }
    }
    let mut out = Vec::new();
    if !single.methods.is_empty() {
        out.push(single);
    }
    out.extend(order.into_iter().filter_map(|t| res.remove(&t)));
    for o in &out {
        if o.methods.len() > drop as usize {
            return Err(format!("{at}/{}: more than {drop} methods", o.name));
        }
    }
    Ok(out)
}

fn is_async(f: &Function) -> bool {
    matches!(f.kind, FunctionKind::AsyncFreestanding | FunctionKind::AsyncMethod(_))
}

/// A method's own name, its resource's prefix dropped.
fn method_name(f: &Function) -> &str {
    f.name.rsplit(['.', ']']).next().unwrap_or(&f.name)
}

/// A method's constant: a resource's prefixed by it.
fn screaming(o: &Obj, f: &Function) -> String {
    let n = if o.resource { format!("{}-{}", o.name, method_name(f)) } else { method_name(f).to_string() };
    n.replace('-', "_").to_ascii_uppercase()
}

/// A method's args, `self` dropped: the handle names it.
fn params(f: &Function) -> impl Iterator<Item = &Param> {
    f.params.iter().filter(|p| p.name != "self")
}

/// Whether `ty` is xpute:link's `types.lent`, the one place raw memory enters
/// WIT.
fn is_lent(r: &Resolve, ty: &Type) -> bool {
    let Type::Id(mut t) = *ty else { return false };
    while let TypeDefKind::Type(Type::Id(n)) = r.types[t].kind {
        t = n;
    }
    let td = &r.types[t];
    let TypeOwner::Interface(i) = td.owner else { return false };
    td.name.as_deref() == Some("lent")
        && r.interfaces[i].name.as_deref() == Some("types")
        && package_name(r, i) == "xpute:link"
}

fn check_func(r: &Resolve, f: &Function, at: &str) -> Result<(), String> {
    for p in &f.params {
        check_type(r, &p.ty, at)?;
    }
    if let Some(t) = &f.result {
        check_type(r, t, at)?;
    }
    Ok(())
}

/// Fails on what the link does not carry.
fn check_type(r: &Resolve, ty: &Type, at: &str) -> Result<(), String> {
    match ty {
        Type::Bool | Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::S8 | Type::S16 | Type::S32 | Type::S64 => Ok(()),
        Type::F32 | Type::F64 | Type::String => Ok(()),
        Type::Id(t) => match &r.types[*t].kind {
            TypeDefKind::Record(rec) => {
                for f in &rec.fields {
                    check_type(r, &f.ty, at)?;
                }
                Ok(())
            }
            TypeDefKind::Variant(v) => {
                let mut seen = BTreeSet::new();
                for c in &v.cases {
                    let ty = c.ty.as_ref().ok_or(format!("{at}: a variant's case carries no payload"))?;
                    check_type(r, ty, at)?;
                    let class = node_class(&kind(r, ty)).ok_or(format!("{at}: a variant's case is optional"))?;
                    if !seen.insert(class) {
                        return Err(format!("{at}: two of a variant's cases cross as {class}s"));
                    }
                }
                Ok(())
            }
            TypeDefKind::Enum(e) if e.cases.len() <= 256 => Ok(()),
            TypeDefKind::List(t) | TypeDefKind::Option(t) => check_type(r, t, at),
            TypeDefKind::Type(t) => check_type(r, t, at),
            TypeDefKind::Resource | TypeDefKind::Handle(_) => Ok(()),
            k => Err(format!("{at}: the link carries no {}", k.as_str())),
        },
        t => Err(format!("{at}: the link carries no {t:?}")),
    }
}


/// What a type is, as the link carries it.
#[derive(Clone)]
enum Kind {
    Bool,
    Int { rs: &'static str, signed: bool },
    U64,
    /// An s64, which crosses as the u64 of its bits: TypeScript writes every
    /// bigint as a u64.
    S64,
    Float { rs: &'static str },
    Str,
    /// A list of fixed-size numbers, which crosses as XTP's packed array of
    /// them: its element in Rust, and its typed array in TypeScript.
    Array { rs: &'static str, ts: &'static str },
    List(Box<Kind>),
    Option(Box<Kind>),
    Record(TypeId),
    Enum(TypeId),
    Handle,
}

fn kind(r: &Resolve, ty: &Type) -> Kind {
    match ty {
        Type::Bool => Kind::Bool,
        Type::U8 => Kind::Int { rs: "u8", signed: false },
        Type::U16 => Kind::Int { rs: "u16", signed: false },
        Type::U32 => Kind::Int { rs: "u32", signed: false },
        Type::U64 => Kind::U64,
        Type::S64 => Kind::S64,
        Type::S8 => Kind::Int { rs: "i8", signed: true },
        Type::S16 => Kind::Int { rs: "i16", signed: true },
        Type::S32 => Kind::Int { rs: "i32", signed: true },
        Type::F32 => Kind::Float { rs: "f32" },
        Type::F64 => Kind::Float { rs: "f64" },
        Type::String => Kind::Str,
        Type::Id(t) => match &r.types[*t].kind {
            TypeDefKind::Record(_) | TypeDefKind::Variant(_) => Kind::Record(*t),
            TypeDefKind::Enum(_) => Kind::Enum(*t),
            // A list crosses as XTP's packed array where both sides' writers
            // take one (kit's TypeScript `Arg` takes u8 and u32 arrays), and as
            // a branch otherwise.
            TypeDefKind::List(e) => match e {
                Type::U8 => Kind::Array { rs: "u8", ts: "U8Array" },
                Type::U32 => Kind::Array { rs: "u32", ts: "U32Array" },
                e => Kind::List(Box::new(kind(r, e))),
            },
            TypeDefKind::Option(e) => Kind::Option(Box::new(kind(r, e))),
            TypeDefKind::Type(e) => kind(r, e),
            _ => Kind::Handle,
        },
        _ => Kind::Handle,
    }
}

fn pascal(s: &str) -> String {
    s.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_ascii_uppercase().to_string() + c.as_str())
        })
        .collect()
}

/// An enum's members as the enum writers take them (xpute-gen `rs_enum`,
/// `ts_enum`): each one's identifier, its name, and its place. Its name is its
/// identifier, which is also its TypeScript key, so the Rust `name` and the
/// TypeScript `<e>_name` answer one string.
fn members(e: &wit_parser::Enum) -> Vec<(String, String, String)> {
    e.cases
        .iter()
        .enumerate()
        .map(|(k, c)| (pascal(&c.name), pascal(&c.name), k.to_string()))
        .collect()
}

/// The interface a type of another one is reached through: its name in the
/// bindings, its Rust module and its TypeScript module.
struct Owner {
    alias: String,
    rs: String,
    ts: String,
}

fn package_name(r: &Resolve, i: InterfaceId) -> String {
    r.interfaces[i]
        .package
        .map(|p| format!("{}:{}", r.packages[p].name.namespace, r.packages[p].name.name))
        .unwrap_or_default()
}

struct Gen<'a> {
    r: &'a Resolve,
    here: InterfaceId,
    side: &'a str,
    /// The package's `namespace:package`.
    pkg: String,
    /// The bit that marks a named handle (frame.json `NAMED`).
    named_bit: u32,
    /// The other interfaces a TypeScript module takes types from.
    uses: RefCell<BTreeMap<String, String>>,
    /// The items a Rust module takes from other crates, each `use`d at its
    /// head so the body names it by its name.
    rs_uses: RefCell<BTreeSet<String>>,
    /// Where each other package's bindings are, by its `namespace:package`:
    /// its Rust module and its TypeScript directory.
    externs: &'a [(String, String, String)],
}

impl Gen<'_> {
    fn iname(&self) -> String {
        self.r.interfaces[self.here].name.clone().unwrap_or_default()
    }

    /// The interface's named handle (`named`).
    fn handle(&self) -> u32 {
        named(&format!("{}/{}", self.pkg, self.iname()), self.named_bit)
    }

    fn kinds(&self, f: &Function) -> Vec<(String, Kind)> {
        params(f).map(|p| (rs_ident(&p.name), kind(self.r, &p.ty))).collect()
    }

    fn ret(&self, f: &Function) -> Option<Kind> {
        f.result.as_ref().map(|t| kind(self.r, t))
    }

    // ---- Rust ----

    /// The Rust of the args `ps`: none, or how many and what writes them,
    /// each of `lents` lending memory, written as the lent record of its
    /// (at, len).
    fn rs_args(&self, ps: &[(String, Kind)], lents: &[String]) -> String {
        if ps.is_empty() {
            return "None".into();
        }
        let writes: Vec<String> = ps
            .iter()
            .map(|(n, k)| {
                if lents.contains(n) {
                    format!("{} {{ at: {n}_at, len: {n}_len }}.write(w);", self.rs_ty(k, true))
                } else {
                    self.rs_write(n, k)
                }
            })
            .collect();
        let w = format!("|w: &mut PacketWriter| {{ {} }}", writes.join(" "));
        format!("Some(({}, &{w}))", ps.len())
    }

    /// The Rust that answers `v`, a method's value.
    fn rs_reply(&self, rk: &Option<Kind>) -> String {
        match rk {
            None => "Reply::from(0)".into(),
            Some(Kind::Bool) => "Reply::from(i32::from(v))".into(),
            Some(k) if in_result(k) => "Reply::from(v as i32)".into(),
            Some(k) => format!(
                "Reply {{ result: 0, data: Some((1, Box::new(move |w: &mut PacketWriter| {{ {} }}))) }}",
                self.rs_write("v", k)
            ),
        }
    }

    /// The Rust of the answer's decoding, from the result `r` and the return
    /// data `data`.
    fn rs_answer(&self, k: &Option<Kind>) -> String {
        match k {
            None => "xpute_link::answer(r).map(|_| ())".into(),
            Some(Kind::Bool) => "xpute_link::answer(r).map(|v| v != 0)".into(),
            Some(k) if in_result(k) => format!("xpute_link::answer(r).map(|v| v as {})", self.rs_ty(k, true)),
            Some(k) => format!(
                "xpute_link::answer(r).and_then(|_| {{ let p = data.as_deref().ok_or(Errno::EBADMSG)?; TreeReader::new(p).and_then(|t| t.read_branch()).and_then(|b| b.at(0)).and_then(|c| {}).map_err(|e| e.errnum) }})",
                self.rs_read("c", k, true)
            ),
        }
    }

    /// The serving side of an object: its trait, and the object over it.
    fn rs_export(&self, o: &Obj, out: &mut String) {
        let t = pascal(&o.name);
        let _ = writeln!(out, "pub trait {t} {{");
        for f in &o.methods {
            let ps: Vec<String> = self.kinds(f).iter().map(|(n, k)| format!("{n}: {}", self.rs_ty(k, true))).collect();
            let ret = self.ret(f).map_or("()".into(), |k| self.rs_ty(&k, true));
            let name = rs_ident(method_name(f));
            if is_async(f) {
                let _ = writeln!(out, "    fn {name}(&self, {}task_return: Return<{ret}>);", ps.iter().map(|p| format!("{p}, ")).collect::<String>());
            } else {
                let _ = writeln!(out, "    fn {name}(&self, {}) -> Result<{ret}, Errno>;", ps.join(", "));
            }
        }
        let _ = writeln!(out, "}}\n\nstruct {t}Object(Rc<dyn {t}>);\n\nimpl Object for {t}Object {{\n    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {{\n        let run = || -> Result<Reply, MarshalError> {{\n            Ok(match m {{");
        for f in &o.methods {
            let ps = self.kinds(f);
            let _ = writeln!(out, "                {} => {{", screaming(o, f));
            if !ps.is_empty() {
                let _ = writeln!(out, "                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;");
            }
            for (k, (n, kd)) in ps.iter().enumerate() {
                let _ = writeln!(out, "                    let {n} = b.at({k}).and_then(|c| {})?;", self.rs_read("c", kd, true));
            }
            let names: String = ps.iter().map(|(n, _)| format!("{n}, ")).collect();
            let name = rs_ident(method_name(f));
            let rk = self.ret(f);
            if is_async(f) {
                let done = format!(
                    "Return::new(tag, link::on_cancel, Box::new(move |got| {{ let reply = match got {{ Ok(v) => {}, Err(e) => Reply::from(e as i32) }}; link::task_return(tag, reply.result, reply.data.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)))) }}))",
                    self.rs_reply(&rk)
                );
                let _ = writeln!(out, "                    self.0.{name}({names}{done});\n                    Reply::from(Errno::EINPROGRESS as i32)\n                }}");
            } else {
                let _ = writeln!(out, "                    match self.0.{name}({}) {{\n                        Ok(v) => {},\n                        Err(e) => Reply::from(e as i32),\n                    }}\n                }}", names.trim_end_matches(", "), self.rs_reply(&rk));
            }
        }
        let _ = writeln!(out, "                _ => Reply::from(Errno::ENOSYS as i32),\n            }})\n        }};\n        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))\n    }}\n}}\n");
        if o.resource {
            let _ = writeln!(out, "/// The object over `imp`, for an opener to answer.\npub fn {}(imp: Rc<dyn {t}>) -> Rc<dyn Object> {{\n    Rc::new({t}Object(imp))\n}}\n", rs_ident(&o.name));
        } else {
            let _ = writeln!(out, "/// Serves `imp` under the interface's handle.\npub fn mount(imp: Rc<dyn {t}>) {{\n    link::mount_named(HANDLE, Rc::new({t}Object(imp)));\n}}\n");
        }
    }

    /// The calling side of an object: its methods, by a handle, a
    /// singleton's its interface's named one.
    fn rs_import(&self, o: &Obj, out: &mut String) {
        for f in &o.methods {
            let ps = self.kinds(f);
            let lents: Vec<String> = params(f).filter(|p| is_lent(self.r, &p.ty)).map(|p| rs_ident(&p.name)).collect();
            // An async call's one lent buffer, which its answer hands back.
            let lent = if is_async(f) { lents.first().cloned() } else { None };
            // A sync call lends slices of any plain elements, each its own; an
            // async call lends any buffer that says its span (link `Lend`).
            let lent_ty = |k: usize| if is_async(f) { "L".to_string() } else { format!("&mut [T{k}]") };
            let generic = if lents.is_empty() {
                String::new()
            } else if is_async(f) {
                self.rs_uses.borrow_mut().insert("xpute_link::Lend".into());
                "<L: Lend>".to_string()
            } else {
                let ts: Vec<String> = (0..lents.len()).map(|k| format!("T{k}: Copy + Unpin + 'static")).collect();
                format!("<{}>", ts.join(", "))
            };
            let mut sig: Vec<String> = if o.resource { vec![format!("{}: Handle", rs_ident(&o.name))] } else { vec![] };
            sig.extend(ps.iter().map(|(n, k)| match lents.iter().position(|l| l == n) {
                Some(j) => format!("{n}: {}", lent_ty(j)),
                None => format!("{n}: {}", self.rs_ty(k, false)),
            }));
            let h = if o.resource { rs_ident(&o.name) } else { "HANDLE".into() };
            let rk = self.ret(f);
            let ret = rk.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
            if !lents.is_empty() && !is_async(f) {
                self.rs_uses.borrow_mut().insert("xpute_link::mem".into());
            }
            if is_async(f) {
                self.rs_uses.borrow_mut().insert("core::future::Future".into());
            }
            let lend: String = lents
                .iter()
                .map(|l| {
                    if is_async(f) {
                        format!("let mut {l} = {l}; let ({l}_at, {l}_len) = Lend::span(&mut {l}); ")
                    } else {
                        format!("let {l}_at = mem::off({l}.as_ptr().cast()); let {l}_len = size_of_val(&{l}[..]) as u32; ")
                    }
                })
                .collect();
            let c = screaming(o, f);
            let name = if o.resource { rs_ident(&format!("{}-{}", o.name, method_name(f))) } else { rs_ident(method_name(f)) };
            if is_async(f) {
                let (output, kept, give) = match &lent {
                    Some(l) => (format!("(L, Result<{ret}, Errno>)"), l.clone(), format!("(lent, {})", self.rs_answer(&rk))),
                    None => (format!("Result<{ret}, Errno>"), "()".into(), self.rs_answer(&rk)),
                };
                // The call is sent here, and the future answers it.
                let _ = writeln!(out, "pub fn {name}{generic}({}) -> impl Future<Output = {output}> {{\n    {lend}\n    let sent = wait({h}, {c}, {}, {kept});\n    async move {{\n        let (r, data, lent) = sent.await;\n        {give}\n    }}\n}}\n", sig.join(", "), self.rs_args(&ps, &lents));
            } else {
                let _ = writeln!(out, "pub fn {name}{generic}({}) -> Result<{ret}, Errno> {{\n    {lend}let (r, data) = link::call({h}, {c}, {});\n    {}\n}}\n", sig.join(", "), self.rs_args(&ps, &lents), self.rs_answer(&rk));
            }
        }
    }

    fn rs_iface(&self, objs: &[Obj]) -> String {
        let iname = self.iname();
        let side = self.side;
        let mut items = String::new();
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(items, "/// The interface's handle, named by `{}/{iname}` (frame.json `NAMED`).\npub const HANDLE: Handle = {:#x};\n", self.pkg, self.handle());
        }
        let _ = writeln!(items, "/// The interface's qualified name: its package and its name.\npub const NAME: [&str; 2] = [\"{}\", \"{iname}\"];\n", self.pkg);
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(items, "/// `{}`\npub const {}: Method = {k};", f.name, screaming(o, f));
            }
        }
        items.push('\n');
        self.rs_types(&mut items);
        for o in objs {
            self.rs_export(o, &mut items);
            self.rs_import(o, &mut items);
        }
        let mut body = String::new();
        let _ = writeln!(body, "use std::rc::Rc;\nuse xpute_kit::status::errno::Errno;\nuse xpute_kit::status::error::MarshalError;\nuse xpute_kit::wire::xtp::{{ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader}};");
        // An interface of types alone, as the link's own, takes nothing of
        // the link, which takes its types.
        if !objs.is_empty() {
            let _ = writeln!(body, "use xpute_link::{{Handle, Method, Object, Reply, Return, Tag}};\nuse {side}::link;\nuse {side}::wait::wait;");
        }
        for u in self.rs_uses.borrow().iter() {
            let _ = writeln!(body, "use {u};");
        }
        body.push('\n');
        body.push_str(&items);
        format!("#![allow(unused_imports, unused_variables, dead_code, clippy::all, reason = \"generated from WIT\")]\n\n{body}")
    }

    // ---- TypeScript ----

    fn ts_params(&self, f: &Function) -> Vec<(String, Kind)> {
        params(f).map(|p| (ts_ident(&p.name), kind(self.r, &p.ty))).collect()
    }

    fn ts_ret(&self, f: &Function) -> String {
        self.ret(f).map_or("void".into(), |k| self.ts_ty(&k))
    }

    /// The serving side of an object: its interface, and the object over it.
    fn ts_export(&self, o: &Obj, out: &mut String) {
        let t = pascal(&o.name);
        let _ = writeln!(out, "export interface {t} {{");
        for f in &o.methods {
            let mut sig: Vec<String> = self.ts_params(f).iter().map(|(n, k)| format!("{n}: {}", self.ts_ty(k))).collect();
            let r = self.ts_ret(f);
            // An async method stops on `signal` where its caller cancels it
            // and it can; one that can't leaves it unread.
            if is_async(f) {
                sig.push("signal: AbortSignal".into());
            }
            let r = if is_async(f) { format!("Promise<{r}>") } else { r };
            let _ = writeln!(out, "  {}({}): {r};", ts_ident(method_name(f)), sig.join(", "));
        }
        let fname = if o.resource { ts_ident(&o.name) } else { "object".into() };
        let pick = if o.methods.iter().any(|f| is_async(f)) { "l: Pick<Link, \"task_return\" | \"on_cancel\">, " } else { "" };
        let _ = writeln!(out, "}}\n\n/** The object over `imp`. */\n{}function {fname}({pick}imp: {t}): LinkObject {{\n  return {{\n    call(m, tag, args) {{\n      try {{\n        switch (m) {{", if o.resource { "export " } else { "" });
        for f in &o.methods {
            let mut decs: Vec<String> = self.ts_params(f).iter().enumerate().map(|(k, (_, kd))| self.ts_dec(&format!("args.at({k})"), kd)).collect();
            let takes_args = !decs.is_empty();
            if is_async(f) {
                decs.push("stop.signal".into());
            }
            let call = format!("imp.{}({})", ts_ident(method_name(f)), decs.join(", "));
            let rk = self.ret(f);
            let (result, data) = match &rk {
                None => ("0".to_string(), "[]".to_string()),
                Some(Kind::Bool) => ("v ? 1 : 0".to_string(), "[]".to_string()),
                Some(k) if in_result(k) => ("v".to_string(), "[]".to_string()),
                Some(k) => ("0".to_string(), format!("[{}]", self.ts_enc("v", k))),
            };
            let c = screaming(o, f);
            let _ = writeln!(out, "          case {c}: {{");
            if takes_args {
                let _ = writeln!(out, "            if (args === null) return Errno.EBADMSG;");
            }
            if is_async(f) {
                let bind = if rk.is_none() { "()" } else { "(v)" };
                let _ = writeln!(out, "            const stop = new AbortController();\n            {call}.then(\n              {bind} => l.task_return(tag, {result}, {data}),\n              (e: unknown) => l.task_return(tag, refusal_of(e)),\n            );\n            l.on_cancel(tag, () => stop.abort());\n            return Errno.EINPROGRESS;\n          }}");
            } else if rk.is_none() {
                let _ = writeln!(out, "            {call};\n            return 0;\n          }}");
            } else if data == "[]" {
                let _ = writeln!(out, "            const v = {call};\n            return {result};\n          }}");
            } else {
                let _ = writeln!(out, "            const v = {call};\n            return {{ result: {result}, data: {data} }};\n          }}");
            }
        }
        let _ = writeln!(out, "          default:\n            return Errno.ENOSYS;\n        }}\n      }} catch (e) {{\n        return refusal_of(e);\n      }}\n    }},\n  }};\n}}\n");
        if !o.resource {
            let pick = if pick.is_empty() { "l: Pick<Link, \"mount_named\">" } else { "l: Pick<Link, \"mount_named\" | \"task_return\" | \"on_cancel\">" };
            let pass = if o.methods.iter().any(|f| is_async(f)) { "l, " } else { "" };
            let _ = writeln!(out, "/** Serves `imp` under the interface's handle. */\nexport function mount({pick}, imp: {t}): void {{\n  l.mount_named(HANDLE, object({pass}imp));\n}}\n");
        }
    }

    /// The calling side of an object: its methods as functions, each taking
    /// first the link a singleton is called over, or a resource's reference.
    fn ts_import(&self, o: &Obj, out: &mut String) {
        let (by, to) = if o.resource { ("ref: Ref", "ref.link.send(ref.handle") } else { ("link: Sender", "link.send(HANDLE") };
        for f in &o.methods {
            let ps = self.ts_params(f);
            let mut sig = vec![by.to_string()];
            sig.extend(ps.iter().map(|(n, k)| format!("{n}: {}", self.ts_ty(k))));
            let args: Vec<String> = ps.iter().map(|(n, k)| self.ts_enc(n, k)).collect();
            let rk = self.ret(f);
            let ret = self.ts_ret(f);
            let take = match &rk {
                None => String::new(),
                Some(Kind::Bool) => "\n  return a.result !== 0;".into(),
                Some(k) if in_result(k) => "\n  return a.result;".into(),
                Some(k) => format!(
                    "\n  if (a.data === null) throw new XputeError(Errno.EBADMSG);\n  return {};",
                    self.ts_dec("new TreeReader(a.data).read_branch().at(0)", k)
                ),
            };
            let name = if o.resource { ts_ident(&format!("{}-{}", o.name, method_name(f))) } else { ts_ident(method_name(f)) };
            let _ = writeln!(out, "export async function {name}({}): Promise<{ret}> {{\n  const a = await {to}, {}, [{}]);\n  if (a.result < 0) throw new XputeError(a.result);{take}\n}}\n", sig.join(", "), screaming(o, f), args.join(", "));
        }
    }

    fn ts_iface(&self, objs: &[Obj]) -> String {
        let iname = self.iname();
        let mut out = String::new();
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(out, "/** The interface's handle, named by `{}/{iname}` (frame.json `NAMED`). */\nexport const HANDLE: Handle = {:#x};\n", self.pkg, self.handle());
        }
        let _ = writeln!(out, "/** The interface's qualified name: its package and its name. */\nexport const NAME = uri(\"{}\", \"{iname}\");\n", self.pkg);
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(out, "/** `{}` */\nexport const {}: Method = {k};", f.name, screaming(o, f));
            }
        }
        out.push('\n');
        self.ts_types(&mut out);
        for o in objs {
            self.ts_export(o, &mut out);
            self.ts_import(o, &mut out);
        }
        out
    }

    /// What a module's body takes from the kit, the link and other
    /// interfaces.
    fn ts_uses(&self, body: &str, link: &str) -> String {
        let has = |t: &str| body.contains(t);
        let mut out = String::new();
        let arrays: Vec<&str> = ["U32Array", "U8Array"]
            .into_iter()
            .filter(|a| has(a))
            .collect();
        if !arrays.is_empty() {
            let _ = writeln!(out, "import type {{ {} }} from \"@xpute/kit/abi/array.ts\";", arrays.join(", "));
        }
        if has("Errno.") {
            out.push_str("import { Errno } from \"@xpute/kit/status/errno.spec.ts\";\n");
        }
        let errs: Vec<&str> = ["XputeError"].into_iter().filter(|e| has(&format!("new {e}("))).collect();
        if !errs.is_empty() {
            let _ = writeln!(out, "import {{ {} }} from \"@xpute/kit/status/error.ts\";", errs.join(", "));
        }
        let xtp: BTreeSet<&str> = [("): Arg", "type Arg"), ("new Branch(", "Branch"), ("): Branch", "Branch"), ("c: NodeCursor", "type NodeCursor"), ("new TreeReader(", "TreeReader")]
            .iter()
            .filter(|(t, _)| has(t))
            .map(|(_, n)| *n)
            .collect();
        if !xtp.is_empty() {
            let _ = writeln!(out, "import {{ {} }} from \"@xpute/kit/wire/xtp/mod.ts\";", xtp.into_iter().collect::<Vec<_>>().join(", "));
        }
        let rt: Vec<&str> = [("Pick<Link", "type Link"), ("LinkObject", "type LinkObject"), (": Method", "type Method"), ("refusal_of(", "refusal_of"), ("ref: Ref", "type Ref"), ("Sender", "type Sender"), (": Handle", "type Handle"), ("NAME = uri(", "uri")]
            .iter()
            .filter(|(t, _)| has(t))
            .map(|(_, n)| *n)
            .collect();
        if !rt.is_empty() {
            let _ = writeln!(out, "import {{ {} }} from \"{link}\";", rt.join(", "));
        }
        for (alias, from) in self.uses.borrow().iter() {
            let _ = writeln!(out, "import * as {alias} from \"{from}\";");
        }
        out
    }
}


impl Gen<'_> {
    /// A type's name, and the interface it is reached through where it is
    /// another's.
    fn type_path(&self, t: TypeId) -> (Option<Owner>, String) {
        let td = &self.r.types[t];
        let name = pascal(td.name.as_deref().unwrap_or("anon"));
        let TypeOwner::Interface(i) = td.owner else { return (None, name) };
        if i == self.here {
            return (None, name);
        }
        let iname = self.r.interfaces[i].name.clone().unwrap_or_default();
        let alias = rs_ident(&iname);
        let pkg = package_name(self.r, i);
        let owner = if pkg == package_name(self.r, self.here) {
            Owner { rs: format!("super::{alias}"), ts: format!("./{alias}.ts"), alias }
        } else {
            let (rs, ts) = self.externs.iter().find(|(p, _, _)| *p == pkg).map_or(("", ""), |(_, rs, ts)| (rs.as_str(), ts.as_str()));
            Owner { rs: format!("{rs}::{alias}"), ts: format!("{ts}/{alias}.ts"), alias }
        };
        (Some(owner), name)
    }

    // ---- Rust types ----

    fn rs_ty(&self, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => "bool".into(),
            Kind::Int { rs, .. } | Kind::Float { rs } => (*rs).into(),
            Kind::U64 => "u64".into(),
            Kind::S64 => "i64".into(),
            Kind::Str => if owned { "String" } else { "&str" }.into(),
            Kind::Array { rs, .. } => if owned { format!("Vec<{rs}>") } else { format!("&[{rs}]") },
            Kind::List(e) => {
                let e = self.rs_ty(e, true);
                if owned { format!("Vec<{e}>") } else { format!("&[{e}]") }
            }
            Kind::Record(t) | Kind::Enum(t) => {
                let (m, n) = self.type_path(*t);
                let p = match m {
                    Some(m) if m.rs.starts_with("super::") => format!("{}::{n}", m.rs),
                    Some(m) => {
                        self.rs_uses.borrow_mut().insert(format!("{}::{n}", m.rs));
                        n
                    }
                    None => n,
                };
                if owned || matches!(k, Kind::Enum(_)) { p } else { format!("&{p}") }
            }
            Kind::Option(e) => format!("Option<{}>", self.rs_ty(e, owned)),
            Kind::Handle => "u32".into(),
        }
    }

    /// Writes `e` into `w`. A deref (`*v`) is parenthesized only where a
    /// method is called on it.
    fn rs_write(&self, e: &str, k: &Kind) -> String {
        let place = if e.starts_with('*') { format!("({e})") } else { e.to_string() };
        match k {
            Kind::Bool => format!("w.bool(Some({e}));"),
            Kind::Int { signed: false, rs: "u8" } => format!("w.u8(Some({e}));"),
            Kind::Int { signed: false, .. } | Kind::Handle => format!("w.u32(Some({e} as u32));"),
            Kind::Int { signed: true, .. } => format!("w.i32(Some({e} as i32));"),
            Kind::U64 => format!("w.u64(Some({e}));"),
            Kind::S64 => format!("w.u64(Some({e} as u64));"),
            Kind::Float { .. } => format!("w.f64(Some({e} as f64));"),
            Kind::Str => format!("w.str(Some(&*{e}));"),
            Kind::Array { rs, .. } => format!("w.array::<{rs}>(Some(&*{e}));"),
            Kind::List(el) => format!(
                "w.branch({place}.len() as u32, |w| {{ for x in {place}.iter() {{ {} }} }});",
                self.rs_write("*x", el)
            ),
            Kind::Record(_) => format!("{place}.write(w);"),
            Kind::Enum(_) => format!("w.u32(Some({e} as u32));"),
            Kind::Option(el) => format!(
                "match &{e} {{ Some(v) => {{ {} }} None => {{ w.nil(); }} }}",
                self.rs_write("*v", el)
            ),
        }
    }

    /// Reads the node `c` as `k`, an expression of `Result<_, MarshalError>`.
    fn rs_read(&self, c: &str, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => format!("{c}.get_bool()"),
            Kind::Int { rs, .. } | Kind::Float { rs } => format!("{c}.get_number().map(|v| v as {rs})"),
            Kind::Handle => format!("{c}.get_number().map(|v| v as u32)"),
            Kind::U64 => format!("{c}.get_u64()"),
            Kind::S64 => format!("{c}.get_u64().map(|v| v as i64)"),
            Kind::Str if owned => format!("{c}.get_str_ref().map(str::to_owned)"),
            Kind::Str => format!("{c}.get_str_ref()"),
            Kind::Array { rs, .. } if owned => format!("{c}.get_array::<{rs}>().map(<[{rs}]>::to_vec)"),
            Kind::Array { rs, .. } => format!("{c}.get_array::<{rs}>()"),
            Kind::List(el) => format!(
                "{c}.as_branch().and_then(|b| (0..b.len).map(|i| b.at(i).and_then(|c| {})).collect::<Result<Vec<_>, MarshalError>>())",
                self.rs_read("c", el, true)
            ),
            Kind::Record(_) => format!("{}::read({c})", self.rs_ty(k, true)),
            Kind::Option(el) => format!("{c}.opt().map(|c| {}).transpose()", self.rs_read("c", el, owned)),
            Kind::Enum(_) => format!(
                "{c}.get_number().and_then(|v| {}::of(v as u32).ok_or(MarshalError::new(Errno::EBADMSG)))",
                self.rs_ty(k, true)
            ),
        }
    }

    fn rs_types(&self, out: &mut String) {
        for (name, &t) in &self.r.interfaces[self.here].types {
            let td = &self.r.types[t];
            if !matches!(td.owner, TypeOwner::Interface(i) if i == self.here) {
                continue;
            }
            let n = pascal(name);
            match &td.kind {
                TypeDefKind::Enum(e) => {
                    let _ = writeln!(out, "{}\n", rs_enum(&n, &members(e), "u32", "").join("\n"));
                }
                TypeDefKind::Record(rec) => {
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub struct {n} {{");
                    for f in &rec.fields {
                        let _ = writeln!(out, "    pub {}: {},", rs_ident(&f.name), self.rs_ty(&kind(self.r, &f.ty), true));
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    pub fn write(&self, w: &mut PacketWriter) {{\n        w.branch({}, |w| {{", rec.fields.len());
                    for f in &rec.fields {
                        let _ = writeln!(out, "            {}", self.rs_write(&format!("self.{}", rs_ident(&f.name)), &kind(self.r, &f.ty)));
                    }
                    let _ = writeln!(out, "        }});\n    }}\n\n    pub fn read(c: Cursor<'_>) -> Result<{n}, MarshalError> {{\n        let b = c.as_branch()?;\n        Ok({n} {{");
                    for (k, f) in rec.fields.iter().enumerate() {
                        let _ = writeln!(out, "            {}: b.at({k}).and_then(|c| {})?,", rs_ident(&f.name), self.rs_read("c", &kind(self.r, &f.ty), true));
                    }
                    let _ = writeln!(out, "        }})\n    }}\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub enum {n} {{");
                    for c in &v.cases {
                        let k = kind(self.r, c.ty.as_ref().unwrap_or(&Type::Bool));
                        let _ = writeln!(out, "    {}({}),", pascal(&c.name), self.rs_ty(&k, true));
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    /// Writes its payload alone, whose node's type says its case.\n    pub fn write(&self, w: &mut PacketWriter) {{\n        match self {{");
                    for c in &v.cases {
                        let k = kind(self.r, c.ty.as_ref().unwrap_or(&Type::Bool));
                        let _ = writeln!(out, "            {n}::{}(v) => {{ {} }}", pascal(&c.name), self.rs_write("(*v)", &k));
                    }
                    // A case is told by its node's class, as the TypeScript tells it.
                    let _ = writeln!(out, "        }}\n    }}\n\n    pub fn read(c: Cursor<'_>) -> Result<{n}, MarshalError> {{\n        let t = c.type_();");
                    for c in &v.cases {
                        let k = kind(self.r, c.ty.as_ref().unwrap_or(&Type::Bool));
                        let _ = writeln!(out, "        if {} {{\n            return {}.map({n}::{});\n        }}", rs_class(&k), self.rs_read("c", &k, true), pascal(&c.name));
                    }
                    let _ = writeln!(out, "        Err(MarshalError::new(Errno::EBADMSG))\n    }}\n}}\n");
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(out, "pub type {n} = {};\n", self.rs_ty(&kind(self.r, ty), true));
                }
                _ => {}
            }
        }
    }

    // ---- TypeScript types ----

    /// A type of another interface, by its module, which the file imports.
    fn ts_path(&self, t: TypeId, prefix: &str) -> String {
        let (m, n) = self.type_path(t);
        match m {
            Some(m) => {
                self.uses.borrow_mut().insert(m.alias.clone(), m.ts);
                format!("{}.{prefix}{n}", m.alias)
            }
            None => format!("{prefix}{n}"),
        }
    }

    fn ts_ty(&self, k: &Kind) -> String {
        match k {
            Kind::Bool => "boolean".into(),
            Kind::Int { .. } | Kind::Float { .. } | Kind::Handle => "number".into(),
            Kind::U64 | Kind::S64 => "bigint".into(),
            Kind::Str => "string".into(),
            Kind::Array { ts, .. } => (*ts).into(),
            Kind::List(e) => format!("{}[]", self.ts_ty(e)),
            Kind::Option(e) => format!("({} | null)", self.ts_ty(e)),
            Kind::Record(t) | Kind::Enum(t) => self.ts_path(*t, ""),
        }
    }

    fn ts_enc(&self, e: &str, k: &Kind) -> String {
        match k {
            Kind::S64 => format!("BigInt.asUintN(64, {e})"),
            Kind::List(el) => format!("new Branch({e}.map((x) => {}))", self.ts_enc("x", el)),
            Kind::Record(t) => format!("{}({e})", self.ts_path(*t, "write_")),
            Kind::Option(el) if matches!(**el, Kind::List(_) | Kind::Record(_)) => {
                format!("{e} === null ? null : {}", self.ts_enc(e, el))
            }
            _ => e.to_string(),
        }
    }

    fn ts_dec(&self, c: &str, k: &Kind) -> String {
        match k {
            Kind::Bool => format!("{c}.get_bool()"),
            Kind::Int { .. } | Kind::Float { .. } | Kind::Handle => format!("{c}.get_number()"),
            Kind::U64 => format!("{c}.get_u64()"),
            Kind::S64 => format!("BigInt.asIntN(64, {c}.get_u64())"),
            Kind::Enum(_) => format!("({c}.get_number() as {})", self.ts_ty(k)),
            Kind::Str => format!("{c}.get_str()"),
            Kind::Array { ts, .. } => format!("({c}.get() as {ts})"),
            Kind::List(el) => format!("{c}.as_branch().children().map((c) => {})", self.ts_dec("c", el)),
            Kind::Record(t) => format!("{}({c})", self.ts_path(*t, "read_")),
            Kind::Option(el) => format!("{c}.get_opt((c) => {})", self.ts_dec("c", el)),
        }
    }

    fn ts_types(&self, out: &mut String) {
        for (name, &t) in &self.r.interfaces[self.here].types {
            let td = &self.r.types[t];
            if !matches!(td.owner, TypeOwner::Interface(i) if i == self.here) {
                continue;
            }
            let n = pascal(name);
            match &td.kind {
                TypeDefKind::Enum(e) => {
                    let members: Vec<(String, String)> = members(e).into_iter().map(|(_, name, k)| (name, k)).collect();
                    let _ = writeln!(out, "{}\n", ts_enum(&n, &members).join("\n"));
                }
                TypeDefKind::Record(rec) => {
                    let _ = writeln!(out, "export interface {n} {{");
                    for f in &rec.fields {
                        let _ = writeln!(out, "  {}: {};", ts_ident(&f.name), self.ts_ty(&kind(self.r, &f.ty)));
                    }
                    let fields: Vec<String> = rec
                        .fields
                        .iter()
                        .map(|f| self.ts_enc(&format!("r.{}", ts_ident(&f.name)), &kind(self.r, &f.ty)))
                        .collect();
                    let _ = writeln!(out, "}}\n\nexport function write_{n}(r: {n}): Branch {{\n  return new Branch([{}]);\n}}\n\nexport function read_{n}(c: NodeCursor): {n} {{\n  const b = c.as_branch();\n  return {{", fields.join(", "));
                    for (k, f) in rec.fields.iter().enumerate() {
                        let _ = writeln!(out, "    {}: {},", ts_ident(&f.name), self.ts_dec(&format!("b.at({k})"), &kind(self.r, &f.ty)));
                    }
                    let _ = writeln!(out, "  }};\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let cases: Vec<(String, Kind)> = v
                        .cases
                        .iter()
                        .map(|c| (c.name.clone(), kind(self.r, c.ty.as_ref().unwrap_or(&Type::Bool))))
                        .collect();
                    let alts: Vec<String> = cases.iter().map(|(c, k)| format!("{{ tag: \"{c}\"; val: {} }}", self.ts_ty(k))).collect();
                    let _ = writeln!(out, "export type {n} = {};\n\n/** Its payload alone, whose node's type says its case. */\nexport function write_{n}(v: {n}): Arg {{\n  switch (v.tag) {{", alts.join(" | "));
                    for (c, k) in &cases {
                        let _ = writeln!(out, "    case \"{c}\":\n      return {};", self.ts_enc("v.val", k));
                    }
                    let _ = writeln!(out, "  }}\n}}\n\nexport function read_{n}(c: NodeCursor): {n} {{");
                    let mut ordered: Vec<&(String, Kind)> = cases.iter().filter(|(_, k)| node_class(k) == Some("branch")).collect();
                    ordered.extend(cases.iter().filter(|(_, k)| node_class(k) != Some("branch")));
                    for (c, k) in ordered {
                        let test = match node_class(k) {
                            Some("branch") => "c.is_branch()".to_string(),
                            Some("array") => "ArrayBuffer.isView(c.get())".to_string(),
                            Some("bool") => "typeof c.get() === \"boolean\"".to_string(),
                            Some(t) => format!("typeof c.get() === \"{t}\""),
                            None => "false".to_string(),
                        };
                        let _ = writeln!(out, "  if ({test}) return {{ tag: \"{c}\", val: {} }};", self.ts_dec("c", k));
                    }
                    let _ = writeln!(out, "  throw new XputeError(Errno.EBADMSG);\n}}\n");
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(out, "export type {n} = {};\n", self.ts_ty(&kind(self.r, ty)));
                }
                _ => {}
            }
        }
    }

}

/// The type of node `k` crosses as, which tells a variant's cases apart:
/// none for an option, whose nil any case could be.
fn node_class(k: &Kind) -> Option<&'static str> {
    match k {
        Kind::Bool => Some("bool"),
        Kind::Int { .. } | Kind::Float { .. } | Kind::Enum(_) | Kind::Handle => Some("number"),
        Kind::U64 | Kind::S64 => Some("bigint"),
        Kind::Str => Some("string"),
        Kind::Array { .. } => Some("array"),
        Kind::List(_) | Kind::Record(_) => Some("branch"),
        Kind::Option(_) => None,
    }
}

/// The Rust that tests a node `c`, of type `t`, for the class `k` crosses as
/// (`node_class`).
fn rs_class(k: &Kind) -> String {
    let any = |types: &[&str]| types.iter().map(|ty| format!("t == {ty} as u8")).collect::<Vec<_>>().join(" || ");
    match node_class(k) {
        Some("branch") => "c.is_branch()".into(),
        Some("bool") => any(&["ScalarType::BOOL"]),
        Some("bigint") => any(&["ScalarType::U64", "ScalarType::I64"]),
        Some("string") => any(&["SequenceType::STR"]),
        Some("array") => match k {
            Kind::Array { rs, .. } => format!("t == <{rs} as ArrayElem>::TYPE as u8"),
            _ => "false".into(),
        },
        Some(_) => any(&[
            "ScalarType::U8", "ScalarType::I8", "ScalarType::U16", "ScalarType::I16", "ScalarType::U32",
            "ScalarType::I32", "ScalarType::F32", "ScalarType::F64",
        ]),
        None => "false".into(),
    }
}

/// Whether a function returns `k` in its result, not its packet: a bool, an
/// unsigned integer or a handle, below 2^31.
fn in_result(k: &Kind) -> bool {
    matches!(k, Kind::Bool | Kind::Handle | Kind::Int { signed: false, .. })
}

/// The root module of one side's bindings, `path` from `root`: a module to
/// each of `targets` whose Rust is written in the directory of its name.
pub fn abi_root(root: &Path, path: &str, targets: &[&WitTarget]) -> Result<Output, String> {
    let dir = Path::new(path.trim_end_matches(".rs"));
    let mods: String = targets
        .iter()
        .filter_map(|t| Path::new(t.rs?).strip_prefix(dir).ok()?.file_stem()?.to_str())
        .map(|m| format!("pub mod {m};\n"))
        .collect();
    let text = format!("// {}\n//\n// {}\n\n{mods}", package_path(root, path)?, mark("wit", TOOL));
    Ok(Output::new(path, text))
}

/// The named handle of the interface `name` (`namespace:package/interface`):
/// `named_bit`, frame.json's `NAMED`, and below it the FNV-1a hash of the name
/// folded to the bits under `named_bit`.
fn named(name: &str, named_bit: u32) -> u32 {
    let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    let below = named_bit - 1;
    named_bit | ((hash >> named_bit.trailing_zeros()) ^ hash) & below
}

/// The value of frame.json's `name`, beside xpute:link's WIT, `link`:
/// `NAMED`, the bit of a named handle, or `DROP`, the drop's method, the bound
/// of an object's methods, which none takes.
fn frame_val(link: &Path, name: &str) -> Result<u32, String> {
    let frame = link.join("../spec/frame.json");
    let spec = read(&frame.to_string_lossy())?;
    spec.get(name)
        .and_then(|n| n.val().num())
        .map(|n| n as u32)
        .ok_or(format!("{}: no {name}", frame.display()))
}

/// The path from the directory `from` to `to`.
fn relative(from: &Path, to: &Path) -> String {
    let (f, t): (Vec<_>, Vec<_>) = (from.components().collect(), to.components().collect());
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let up = std::iter::repeat_n("..".to_string(), f.len() - common);
    let down = t[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned());
    let path: Vec<String> = up.chain(down).collect();
    if path.is_empty() { ".".into() } else { path.join("/") }
}

/// How a Rust module in the crate at `here` names the crate of `path` from
/// `root`: by its name, or `crate` for its own.
fn rs_crate(here: &Path, root: &Path, path: &str) -> Result<String, String> {
    let (name, dir) = package(root, path)?;
    Ok(if root.join(dir) == here { "crate".into() } else { name.replace('-', "_") })
}

/// How a Rust module in the crate at `here` names the module `path` from
/// `root`: its crate, then its path under `src/`.
fn rs_module(here: &Path, root: &Path, path: &str) -> Result<String, String> {
    let (_, dir) = package(root, path)?;
    let under = Path::new(path).strip_prefix(dir.join("src")).map_err(|_| format!("{path}: not under src/"))?;
    let mods = under.with_extension("");
    let mods = mods.iter().map(|m| m.to_string_lossy().into_owned());
    Ok(std::iter::once(rs_crate(here, root, path)?).chain(mods).collect::<Vec<_>>().join("::"))
}

/// How a TypeScript module in the directory `here` names the path `path`
/// from `root`: relative to it in its own package, or by the package's name.
fn ts_module(here: &Path, root: &Path, path: &str) -> Result<String, String> {
    let (_, dir) = package(root, path)?;
    if here.starts_with(root.join(dir)) {
        return Ok(relative(here, &root.join(path)));
    }
    package_path(root, path)
}

/// The bindings of `t`'s package. `peers` are the targets of its side, each
/// with its root, xpute:link's first, then each before those that take its
/// types: their WIT is read before its own, the first's names frame.json, and
/// each other package its types take is reached where its peer writes it.
pub fn wit_outputs(root: &Path, t: &WitTarget, peers: &[(&Path, &WitTarget)]) -> Result<Vec<Output>, String> {
    let own = root.join(t.wit);
    let peers: Vec<_> = peers.iter().filter(|(r, p)| r.join(p.wit) != own).collect();
    let mut r = Resolve::default();
    let mut ids = Vec::new();
    for (pr, p) in &peers {
        let (id, _) = r.push_dir(pr.join(p.wit)).map_err(|e| format!("{}: {e:?}", p.wit))?;
        ids.push(id);
    }
    let (pkg, _) = r.push_dir(&own).map_err(|e| format!("{}: {e:?}", t.wit))?;
    let p = &r.packages[pkg];
    let pname = format!("{}:{}", p.name.namespace, p.name.name);
    let link = peers.first().map_or(own.clone(), |(pr, p)| pr.join(p.wit));
    let named_bit = frame_val(&link, "NAMED")?;
    let drop = frame_val(&link, "DROP")?;
    let crate_dir = t.rs.map(|rs| package(root, rs).map(|(_, dir)| root.join(dir))).transpose()?;
    let ts_dir = t.ts.map(|d| root.join(d));
    let mut externs = Vec::new();
    for (&id, (pr, p)) in ids.iter().zip(&peers) {
        let name = &r.packages[id].name;
        let rs = match (&crate_dir, p.rs) {
            (Some(here), Some(rs)) => rs_module(here, pr, rs)?,
            _ => String::new(),
        };
        let ts = match (&ts_dir, p.ts) {
            (Some(here), Some(ts)) => ts_module(here, pr, ts)?,
            _ => String::new(),
        };
        externs.push((format!("{}:{}", name.namespace, name.name), rs, ts));
    }
    let side = match &crate_dir {
        Some(here) => rs_crate(here, root, &format!("{}/Cargo.toml", t.side))?,
        None => String::new(),
    };
    // The link's TypeScript, `link.ts` beside xpute:link's bindings.
    let link_ts = match (&ts_dir, peers.first().map_or(t.ts, |(_, p)| p.ts)) {
        (Some(here), Some(ts)) => {
            let pr = peers.first().map_or(root, |(pr, _)| pr);
            let at = Path::new(ts).parent().unwrap_or(Path::new("")).join("link.ts");
            ts_module(here, pr, &at.to_string_lossy())?
        }
        _ => String::new(),
    };
    // No two interfaces the target reaches share a named handle.
    let mut handles: BTreeMap<u32, String> = BTreeMap::new();
    for (i, iface) in r.interfaces.iter() {
        let Some(n) = &iface.name else { continue };
        let name = format!("{}/{n}", package_name(&r, i));
        if let Some(other) = handles.insert(named(&name, named_bit), name.clone()) {
            return Err(format!("{name} and {other} fold to one named handle"));
        }
    }
    for &i in p.interfaces.values() {
        for &ty in r.interfaces[i].types.values() {
            let TypeDefKind::Type(Type::Id(o)) = r.types[ty].kind else { continue };
            let TypeOwner::Interface(oi) = r.types[o].owner else { continue };
            let other = package_name(&r, oi);
            if other != pname && !externs.iter().any(|(e, _, _)| *e == other) {
                return Err(format!("{pname} takes types of {other}, which no peer writes"));
            }
        }
    }
    let src = t.wit;
    let mut rs = String::new();
    let mut outs = Vec::new();
    let rs_dir = t.rs.map(|path| path.trim_end_matches(".rs"));
    for (iname, &i) in &p.interfaces {
        let at = format!("{pname}/{iname}");
        let objs = objs(&r, i, &at, drop)?;
        for o in &objs {
            for f in &o.methods {
                check_func(&r, f, &format!("{at}.{}", f.name))?;
            }
        }
        for &ty in r.interfaces[i].types.values() {
            check_type(&r, &Type::Id(ty), &at)?;
        }
        let g = Gen {
            r: &r,
            here: i,
            side: &side,
            pkg: pname.clone(),
            named_bit,
            uses: RefCell::default(),
            rs_uses: RefCell::default(),
            externs: &externs,
        };
        let m = rs_ident(iname);
        if let Some(dir) = rs_dir {
            let path = format!("{dir}/{m}.rs");
            let text = format!("// {}\n//\n// {}\n\n{}", package_path(root, &path)?, mark(src, TOOL), g.rs_iface(&objs));
            outs.push(Output::new(&path, text));
            let _ = writeln!(rs, "pub mod {m};");
        }
        if let Some(dir) = t.ts {
            let path = format!("{dir}/{m}.ts");
            let body = g.ts_iface(&objs);
            let imports = g.ts_uses(&body, &link_ts);
            let text = format!("// {}\n//\n// {}\n\n{imports}\n{body}", package_path(root, &path)?, mark(src, TOOL));
            outs.push(Output::new(&path, text));
        }
    }
    if let Some(path) = t.rs {
        let text = format!("// {}\n//\n// {}\n\n{rs}", package_path(root, path)?, mark(src, TOOL));
        outs.push(Output::new(path, text));
    }
    Ok(outs)
}
