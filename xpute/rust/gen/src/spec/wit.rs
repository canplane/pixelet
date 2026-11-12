// xpute-gen/spec/wit.rs

//! This writes the link's bindings from WIT (xpute/wit/link.wit), in Rust and
//! in TypeScript. For each interface it writes its methods' numbers, its
//! types, the side that serves it and the side that calls it: a singleton
//! interface's functions are one object's methods, named by the URI
//! (package, interface), and each resource's methods another's. A method's
//! number is its place among its object's, in declaration order.
//!
//! It reads the subset link.wit names. Anything else is an error: a
//! resource's constructor or static, a variant case with no payload or two
//! that cross as one type, more than 255 methods to an object.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use wit_parser::{Function, FunctionKind, InterfaceId, PackageId, Param, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use crate::spec::Output;

/// One package's bindings and where they go.
pub struct WitTarget<'a> {
    /// The packages' directories, dependencies first; the last is the one
    /// written.
    pub dirs: &'a [&'a str],
    /// The Rust file, and its header's name.
    pub rs: Option<(&'a str, &'a str)>,
    /// The TypeScript directory, a module per interface, and its header's
    /// name.
    pub ts: Option<(&'a str, &'a str)>,
    /// How the Rust names xpute-core, and the module the TypeScript takes the
    /// link from.
    pub core: &'a str,
    pub runtime_ts: &'a str,
    /// Where another package's bindings are: its `namespace:package`, its
    /// Rust module and its TypeScript directory.
    pub externs: &'a [(&'a str, &'a str, &'a str)],
}

/// The release's number, which no object's method takes (link.wit).
const RELEASE: usize = 0xff;

/// One kind of object an interface defines: its singleton, of the
/// interface's functions, or a resource, of its methods.
struct Obj<'a> {
    /// The interface's name, or the resource's.
    name: String,
    resource: bool,
    methods: Vec<&'a Function>,
}

fn objs<'a>(r: &'a Resolve, i: InterfaceId, at: &str) -> Result<Vec<Obj<'a>>, String> {
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
    // The names the generated code's own locals take.
    for f in iface.functions.values() {
        if let Some(p) = f.params.iter().find(|p| ["ref", "a", "b", "w", "m", "tag", "args", "link", "handle"].contains(&p.name.as_str())) {
            return Err(format!("{at}.{}: `{}`, a name the bindings keep for their own", f.name, p.name));
        }
    }
    let mut out = Vec::new();
    if !single.methods.is_empty() {
        out.push(single);
    }
    out.extend(order.into_iter().filter_map(|t| res.remove(&t)));
    for o in &out {
        if o.methods.len() > RELEASE {
            return Err(format!("{at}/{}: more than {RELEASE} methods", o.name));
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
        Type::Bool | Type::U8 | Type::U16 | Type::U32 | Type::S8 | Type::S16 | Type::S32 => Ok(()),
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
    Float { rs: &'static str },
    Str,
    Bytes,
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
        Type::S8 => Kind::Int { rs: "i8", signed: true },
        Type::S16 => Kind::Int { rs: "i16", signed: true },
        Type::S32 => Kind::Int { rs: "i32", signed: true },
        Type::F32 => Kind::Float { rs: "f32" },
        Type::F64 => Kind::Float { rs: "f64" },
        Type::String => Kind::Str,
        Type::Id(t) => match &r.types[*t].kind {
            TypeDefKind::Record(_) | TypeDefKind::Variant(_) => Kind::Record(*t),
            TypeDefKind::Enum(_) => Kind::Enum(*t),
            TypeDefKind::List(Type::U8) => Kind::Bytes,
            TypeDefKind::List(e) => Kind::List(Box::new(kind(r, e))),
            TypeDefKind::Option(e) => Kind::Option(Box::new(kind(r, e))),
            TypeDefKind::Type(e) => kind(r, e),
            _ => Kind::Handle,
        },
        _ => Kind::Handle,
    }
}

const RS_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "yield", "try", "box", "do", "final", "macro", "override", "priv", "typeof",
];

fn snake(s: &str) -> String {
    let n = s.replace('-', "_");
    if RS_KEYWORDS.contains(&n.as_str()) {
        format!("r#{n}")
    } else {
        n
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

const TS_RESERVED: &[&str] = &["in", "function", "new", "delete", "default", "class", "var", "this"];

fn ts_name(s: &str) -> String {
    let n = s.replace('-', "_");
    if TS_RESERVED.contains(&n.as_str()) {
        format!("{n}_")
    } else {
        n
    }
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
    core: &'a str,
    /// The package's `namespace:package`.
    pkg: String,
    /// The other interfaces a TypeScript module takes types from.
    uses: RefCell<BTreeMap<String, String>>,
    externs: &'a [(&'a str, &'a str, &'a str)],
}

impl Gen<'_> {
    fn iname(&self) -> String {
        self.r.interfaces[self.here].name.clone().unwrap_or_default()
    }

    fn kinds(&self, f: &Function) -> Vec<(String, Kind)> {
        params(f).map(|p| (snake(&p.name), kind(self.r, &p.ty))).collect()
    }

    fn ret(&self, f: &Function) -> Option<Kind> {
        f.result.as_ref().map(|t| kind(self.r, t))
    }

    // ---- Rust ----

    /// The Rust of the args `ps`: none, or how many and what writes them.
    fn rs_args(&self, ps: &[(String, Kind)], boxed: bool) -> String {
        if ps.is_empty() {
            return "None".into();
        }
        let writes: Vec<String> = ps.iter().map(|(n, k)| self.rs_write(n, k)).collect();
        let w = format!("|w: &mut PacketWriter| {{ {} }}", writes.join(" "));
        if boxed {
            format!("Some(({}, Box::new(move {w})))", ps.len())
        } else {
            format!("Some(({}, &{w}))", ps.len())
        }
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
            None => "link::answer(r).map(|_| ())".into(),
            Some(Kind::Bool) => "link::answer(r).map(|v| v != 0)".into(),
            Some(k) if in_result(k) => format!("link::answer(r).map(|v| v as {})", self.rs_ty(k, true)),
            Some(k) => format!(
                "link::answer(r).and_then(|_| {{ let p = data.as_deref().ok_or(Errno::EBADMSG)?; TreeReader::new(p).and_then(|t| t.read_branch()).and_then(|b| b.at(0)).and_then(|c| {}).map_err(|e| e.errnum) }})",
                self.rs_read("c", k, true)
            ),
        }
    }

    /// The serving side of an object: its trait, and the object over it.
    fn rs_served(&self, o: &Obj, out: &mut String) {
        let t = pascal(&o.name);
        let _ = writeln!(out, "pub trait {t} {{");
        for f in &o.methods {
            let ps: Vec<String> = self.kinds(f).iter().map(|(n, k)| format!("{n}: {}", self.rs_ty(k, true))).collect();
            let ret = self.ret(f).map_or("()".into(), |k| self.rs_ty(&k, true));
            let name = snake(method_name(f));
            if is_async(f) {
                let _ = writeln!(out, "    fn {name}(&self, {}done: Done<{ret}>);", ps.iter().map(|p| format!("{p}, ")).collect::<String>());
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
            let name = snake(method_name(f));
            let rk = self.ret(f);
            if is_async(f) {
                let done = format!(
                    "Box::new(move |got| {{ let reply = match got {{ Ok(v) => {}, Err(e) => Reply::from(e as i32) }}; link::complete(tag, reply.result, reply.data.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut PacketWriter)))) }})",
                    self.rs_reply(&rk)
                );
                let _ = writeln!(out, "                    self.0.{name}({names}{done});\n                    Reply::from(Errno::EINPROGRESS as i32)\n                }}");
            } else {
                let _ = writeln!(out, "                    match self.0.{name}({}) {{\n                        Ok(v) => {},\n                        Err(e) => Reply::from(e as i32),\n                    }}\n                }}", names.trim_end_matches(", "), self.rs_reply(&rk));
            }
        }
        let _ = writeln!(out, "                _ => Reply::from(Errno::ENOSYS as i32),\n            }})\n        }};\n        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))\n    }}\n}}\n");
        if o.resource {
            let _ = writeln!(out, "/// The object over `imp`, for an opener to answer.\npub fn {}(imp: Rc<dyn {t}>) -> Rc<dyn Object> {{\n    Rc::new({t}Object(imp))\n}}\n", snake(&o.name));
        } else {
            let _ = writeln!(out, "/// Mounts `imp` at the URI, one object for every open.\npub fn mount(imp: Rc<dyn {t}>) {{\n    let o: Rc<dyn Object> = Rc::new({t}Object(imp));\n    link::mount(link::uri(&URI), move |_| Ok(o.clone()));\n}}\n");
        }
    }

    /// The calling side of an object: its methods, by a handle, a
    /// singleton's the one opened at boot.
    fn rs_caller(&self, o: &Obj, out: &mut String) {
        if !o.resource {
            let _ = writeln!(out, "thread_local! {{\n    static HANDLE: Cell<Option<Handle>> = const {{ Cell::new(None) }};\n}}\n\n/// Opens the singleton on the other side, once, at boot.\npub fn open() -> Result<(), Errno> {{\n    HANDLE.set(Some(link::open(&link::uri(&URI))?));\n    Ok(())\n}}\n\nfn handle() -> Handle {{\n    HANDLE.get().or_bug(Errno::EBADF)\n}}\n");
        }
        for f in &o.methods {
            let ps = self.kinds(f);
            let mut sig: Vec<String> = if o.resource { vec![format!("{}: Handle", snake(&o.name))] } else { vec![] };
            sig.extend(ps.iter().map(|(n, k)| format!("{n}: {}", self.rs_ty(k, false))));
            let h = if o.resource { snake(&o.name) } else { "handle()".into() };
            let rk = self.ret(f);
            let ret = rk.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
            let c = screaming(o, f);
            let name = if o.resource { snake(&format!("{}-{}", o.name, method_name(f))) } else { snake(method_name(f)) };
            if is_async(f) {
                let owns: String = ps
                    .iter()
                    .map(|(n, k)| match k {
                        Kind::Str | Kind::Bytes | Kind::List(_) => format!("let {n} = {n}.to_owned(); "),
                        Kind::Record(_) => format!("let {n} = {n}.clone(); "),
                        Kind::Option(e) => match **e {
                            Kind::Str | Kind::Bytes | Kind::List(_) => format!("let {n} = {n}.map(ToOwned::to_owned); "),
                            Kind::Record(_) => format!("let {n} = {n}.cloned(); "),
                            _ => String::new(),
                        },
                        _ => String::new(),
                    })
                    .collect();
                let _ = writeln!(out, "pub fn {name}({}) -> impl core::future::Future<Output = Result<{ret}, Errno>> {{\n    {owns}\n    let sent = wait({h}, {c}, {}, ());\n    async move {{\n        let (r, data) = sent.await;\n        {}\n    }}\n}}\n", sig.join(", "), self.rs_args(&ps, true), self.rs_answer(&rk));
            } else {
                let _ = writeln!(out, "pub fn {name}({}) -> Result<{ret}, Errno> {{\n    let (r, data) = link::call({h}, {c}, {});\n    {}\n}}\n", sig.join(", "), self.rs_args(&ps, false), self.rs_answer(&rk));
            }
        }
    }

    fn rs_iface(&self, objs: &[Obj]) -> String {
        let iname = self.iname();
        let core = self.core;
        let mut body = String::new();
        let _ = writeln!(body, "use std::cell::Cell;\nuse std::rc::Rc;\nuse xpute_kit::status::bug::OrBug;\nuse xpute_kit::status::errno::Errno;\nuse xpute_kit::status::error::MarshalError;\nuse xpute_kit::wire::xtp::{{BranchCursor, Cursor, PacketWriter, TreeReader}};\nuse {core}::link::{{self, Done, Handle, Method, Object, Reply, Tag}};\nuse {core}::wait::wait;\n");
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(body, "/// The singleton's URI.\npub const URI: [&str; 2] = [\"{}\", \"{iname}\"];\n", self.pkg);
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(body, "/// `{}`\npub const {}: Method = {k};", f.name, screaming(o, f));
            }
        }
        body.push('\n');
        self.rs_types(&mut body);
        for o in objs {
            self.rs_served(o, &mut body);
            self.rs_caller(o, &mut body);
        }
        let mut out = format!("pub mod {} {{\n    #![allow(unused_imports, unused_variables, dead_code, clippy::all, reason = \"generated from WIT\")]\n", snake(&iname));
        for l in body.lines() {
            if l.is_empty() {
                out.push('\n');
            } else {
                let _ = writeln!(out, "    {l}");
            }
        }
        out.push_str("}\n");
        out
    }

    // ---- TypeScript ----

    fn ts_params(&self, f: &Function) -> Vec<(String, Kind)> {
        params(f).map(|p| (ts_name(&p.name), kind(self.r, &p.ty))).collect()
    }

    fn ts_ret(&self, f: &Function) -> String {
        self.ret(f).map_or("void".into(), |k| self.ts_ty(&k))
    }

    /// The serving side of an object: its interface, and the object over it.
    fn ts_served(&self, o: &Obj, out: &mut String) {
        let t = pascal(&o.name);
        let _ = writeln!(out, "export interface {t} {{");
        for f in &o.methods {
            let sig: Vec<String> = self.ts_params(f).iter().map(|(n, k)| format!("{n}: {}", self.ts_ty(k))).collect();
            let r = self.ts_ret(f);
            let r = if is_async(f) { format!("Promise<{r}>") } else { r };
            let _ = writeln!(out, "  {}({}): {r};", ts_name(method_name(f)), sig.join(", "));
        }
        let fname = if o.resource { ts_name(&o.name) } else { "object".into() };
        let host = if o.methods.iter().any(|f| is_async(f)) { "link: Pick<Link, \"complete\">, " } else { "" };
        let _ = writeln!(out, "}}\n\n/** The object over `imp`. */\n{}function {fname}({host}imp: {t}): LinkObject {{\n  return {{\n    call(m, tag, args) {{\n      try {{\n        switch (m) {{", if o.resource { "export " } else { "" });
        for f in &o.methods {
            let decs: Vec<String> = self.ts_params(f).iter().enumerate().map(|(k, (_, kd))| self.ts_dec(&format!("args.at({k})"), kd)).collect();
            let call = format!("imp.{}({})", ts_name(method_name(f)), decs.join(", "));
            let rk = self.ret(f);
            let (result, data) = match &rk {
                None => ("0".to_string(), "[]".to_string()),
                Some(Kind::Bool) => ("v ? 1 : 0".to_string(), "[]".to_string()),
                Some(k) if in_result(k) => ("v".to_string(), "[]".to_string()),
                Some(k) => ("0".to_string(), format!("[{}]", self.ts_enc("v", k))),
            };
            let c = screaming(o, f);
            let _ = writeln!(out, "          case {c}: {{");
            if !decs.is_empty() {
                let _ = writeln!(out, "            if (args === null) return Errno.EBADMSG;");
            }
            if is_async(f) {
                let bind = if rk.is_none() { "()" } else { "(v)" };
                let _ = writeln!(out, "            {call}.then(\n              {bind} => link.complete(tag, {result}, {data}),\n              (e: unknown) => link.complete(tag, refusal_of(e)),\n            );\n            return Errno.EINPROGRESS;\n          }}");
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
            let host = if host.is_empty() { "link: Pick<Link, \"mount\">" } else { "link: Pick<Link, \"mount\" | \"complete\">" };
            let pass = if o.methods.iter().any(|f| is_async(f)) { "link, " } else { "" };
            let _ = writeln!(out, "/** Mounts `imp` at the URI, one object for every open. */\nexport function mount({host}, imp: {t}): void {{\n  const o = object({pass}imp);\n  link.mount(URI, () => o);\n}}\n");
        }
    }

    /// The calling side of an object: its methods as functions, each taking
    /// first the reference a singleton's `open` answers, or a resource's.
    fn ts_caller(&self, o: &Obj, out: &mut String) {
        if !o.resource {
            let _ = writeln!(out, "/** Opens the singleton on the other side, once, at boot. */\nexport async function open(link: Sender): Promise<Ref> {{\n  return {{ link, handle: await open_uri(link, URI) }};\n}}\n");
        }
        for f in &o.methods {
            let ps = self.ts_params(f);
            let mut sig = vec!["ref: Ref".to_string()];
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
            let name = if o.resource { ts_name(&format!("{}-{}", o.name, method_name(f))) } else { ts_name(method_name(f)) };
            let _ = writeln!(out, "export async function {name}({}): Promise<{ret}> {{\n  const a = await ref.link.send(ref.handle, {}, [{}]);\n  if (a.result < 0) throw new XputeError(a.result);{take}\n}}\n", sig.join(", "), screaming(o, f), args.join(", "));
        }
    }

    fn ts_iface(&self, objs: &[Obj]) -> String {
        let iname = self.iname();
        let mut out = String::new();
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(out, "/** The singleton's URI. */\nexport const URI = uri(\"{}\", \"{iname}\");\n", self.pkg);
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(out, "/** `{}` */\nexport const {}: Method = {k};", f.name, screaming(o, f));
            }
        }
        out.push('\n');
        self.ts_types(&mut out);
        for o in objs {
            self.ts_served(o, &mut out);
            self.ts_caller(o, &mut out);
        }
        out
    }

    /// What a module's body takes from the kit, the runtime and other
    /// interfaces.
    fn ts_imports(&self, body: &str, runtime: &str) -> String {
        let has = |t: &str| body.contains(t);
        let mut out = String::new();
        if has("U8Array") {
            out.push_str("import type { U8Array } from \"@xpute/kit/abi/array.ts\";\n");
        }
        if has("Errno.") {
            out.push_str("import * as Errno from \"@xpute/kit/status/errno.spec.ts\";\n");
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
        let rt: Vec<&str> = [("Pick<Link", "type Link"), ("LinkObject", "type LinkObject"), (": Method", "type Method"), ("open_uri(", "open as open_uri"), ("refusal_of(", "refusal_of"), ("ref: Ref", "type Ref"), ("Sender", "type Sender"), (" = uri(", "uri")]
            .iter()
            .filter(|(t, _)| has(t))
            .map(|(_, n)| *n)
            .collect();
        if !rt.is_empty() {
            let _ = writeln!(out, "import {{ {} }} from \"{runtime}\";", rt.join(", "));
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
        let alias = snake(&iname);
        let pkg = package_name(self.r, i);
        let owner = if pkg == package_name(self.r, self.here) {
            Owner { rs: format!("super::{alias}"), ts: format!("./{iname}.ts"), alias }
        } else {
            let (_, rs, ts) = self.externs.iter().find(|(p, _, _)| *p == pkg).copied().unwrap_or(("", "", ""));
            Owner { rs: format!("{rs}::{alias}"), ts: format!("{ts}/{iname}.ts"), alias }
        };
        (Some(owner), name)
    }

    // ---- Rust types ----

    fn rs_ty(&self, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => "bool".into(),
            Kind::Int { rs, .. } | Kind::Float { rs } => (*rs).into(),
            Kind::Str => if owned { "String" } else { "&str" }.into(),
            Kind::Bytes => if owned { "Vec<u8>" } else { "&[u8]" }.into(),
            Kind::List(e) => {
                let e = self.rs_ty(e, true);
                if owned { format!("Vec<{e}>") } else { format!("&[{e}]") }
            }
            Kind::Record(t) | Kind::Enum(t) => {
                let (m, n) = self.type_path(*t);
                let p = m.map_or(n.clone(), |m| format!("{}::{n}", m.rs));
                if owned || matches!(k, Kind::Enum(_)) { p } else { format!("&{p}") }
            }
            Kind::Option(e) => format!("Option<{}>", self.rs_ty(e, owned)),
            Kind::Handle => "u32".into(),
        }
    }

    /// Writes `e` into `w`.
    fn rs_write(&self, e: &str, k: &Kind) -> String {
        match k {
            Kind::Bool => format!("w.bool(Some({e}));"),
            Kind::Int { signed: false, rs: "u8" } => format!("w.u8(Some({e}));"),
            Kind::Int { signed: false, .. } | Kind::Handle => format!("w.u32(Some({e} as u32));"),
            Kind::Int { signed: true, .. } => format!("w.i32(Some({e} as i32));"),
            Kind::Float { .. } => format!("w.f64(Some({e} as f64));"),
            Kind::Str => format!("w.str(Some(&*{e}));"),
            Kind::Bytes => format!("w.array::<u8>(Some(&*{e}));"),
            Kind::List(el) => format!(
                "w.branch({e}.len() as u32, |w| {{ for x in {e}.iter() {{ {} }} }});",
                self.rs_write("(*x)", el)
            ),
            Kind::Record(_) => format!("{e}.write(w);"),
            Kind::Enum(_) => format!("w.u32(Some({e} as u32));"),
            Kind::Option(el) => format!(
                "match &{e} {{ Some(v) => {{ {} }} None => {{ w.nil(); }} }}",
                self.rs_write("(*v)", el)
            ),
        }
    }

    /// Reads the node `c` as `k`, an expression of `Result<_, MarshalError>`.
    fn rs_read(&self, c: &str, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => format!("{c}.get_bool()"),
            Kind::Int { rs, .. } | Kind::Float { rs } => format!("{c}.get_number().map(|v| v as {rs})"),
            Kind::Handle => format!("{c}.get_number().map(|v| v as u32)"),
            Kind::Str if owned => format!("{c}.get_str_ref().map(str::to_owned)"),
            Kind::Str => format!("{c}.get_str_ref()"),
            Kind::Bytes if owned => format!("{c}.get_array::<u8>().map(<[u8]>::to_vec)"),
            Kind::Bytes => format!("{c}.get_array::<u8>()"),
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
                    let _ = writeln!(out, "#[repr(u32)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum {n} {{");
                    for (k, c) in e.cases.iter().enumerate() {
                        let _ = writeln!(out, "    {} = {k},", pascal(&c.name));
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    pub const ALL: [{n}; {}] = [{}];\n\n    pub fn of(v: u32) -> Option<{n}> {{\n        {n}::ALL.get(v as usize).copied()\n    }}\n}}\n",
                        e.cases.len(),
                        e.cases.iter().map(|c| format!("{n}::{}", pascal(&c.name))).collect::<Vec<_>>().join(", "));
                }
                TypeDefKind::Record(rec) => {
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub struct {n} {{");
                    for f in &rec.fields {
                        let _ = writeln!(out, "    pub {}: {},", snake(&f.name), self.rs_ty(&kind(self.r, &f.ty), true));
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    pub fn write(&self, w: &mut PacketWriter) {{\n        w.branch({}, |w| {{", rec.fields.len());
                    for f in &rec.fields {
                        let _ = writeln!(out, "            {}", self.rs_write(&format!("self.{}", snake(&f.name)), &kind(self.r, &f.ty)));
                    }
                    let _ = writeln!(out, "        }});\n    }}\n\n    pub fn read(c: Cursor<'_>) -> Result<{n}, MarshalError> {{\n        let b = c.as_branch()?;\n        Ok({n} {{");
                    for (k, f) in rec.fields.iter().enumerate() {
                        let _ = writeln!(out, "            {}: b.at({k}).and_then(|c| {})?,", snake(&f.name), self.rs_read("c", &kind(self.r, &f.ty), true));
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
                    let _ = writeln!(out, "        }}\n    }}\n\n    pub fn read(c: Cursor<'_>) -> Result<{n}, MarshalError> {{");
                    for c in &v.cases {
                        let k = kind(self.r, c.ty.as_ref().unwrap_or(&Type::Bool));
                        let _ = writeln!(out, "        if let Ok(v) = {} {{\n            return Ok({n}::{}(v));\n        }}", self.rs_read("c", &k, true), pascal(&c.name));
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
            Kind::Str => "string".into(),
            Kind::Bytes => "U8Array".into(),
            Kind::List(e) => format!("{}[]", self.ts_ty(e)),
            Kind::Option(e) => format!("({} | null)", self.ts_ty(e)),
            Kind::Record(t) | Kind::Enum(t) => self.ts_path(*t, ""),
        }
    }

    fn ts_enc(&self, e: &str, k: &Kind) -> String {
        match k {
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
            Kind::Enum(_) => format!("({c}.get_number() as {})", self.ts_ty(k)),
            Kind::Str => format!("{c}.get_str()"),
            Kind::Bytes => format!("({c}.get() as U8Array)"),
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
                    let _ = writeln!(out, "export const {n} = {{");
                    for (k, c) in e.cases.iter().enumerate() {
                        let _ = writeln!(out, "  {}: {k},", pascal(&c.name));
                    }
                    let _ = writeln!(out, "}} as const;\nexport type {n} = (typeof {n})[keyof typeof {n}];\n");
                }
                TypeDefKind::Record(rec) => {
                    let _ = writeln!(out, "export interface {n} {{");
                    for f in &rec.fields {
                        let _ = writeln!(out, "  {}: {};", ts_name(&f.name), self.ts_ty(&kind(self.r, &f.ty)));
                    }
                    let fields: Vec<String> = rec
                        .fields
                        .iter()
                        .map(|f| self.ts_enc(&format!("r.{}", ts_name(&f.name)), &kind(self.r, &f.ty)))
                        .collect();
                    let _ = writeln!(out, "}}\n\nexport function write_{n}(r: {n}): Branch {{\n  return new Branch([{}]);\n}}\n\nexport function read_{n}(c: NodeCursor): {n} {{\n  const b = c.as_branch();\n  return {{", fields.join(", "));
                    for (k, f) in rec.fields.iter().enumerate() {
                        let _ = writeln!(out, "    {}: {},", ts_name(&f.name), self.ts_dec(&format!("b.at({k})"), &kind(self.r, &f.ty)));
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
                            Some("bytes") => "c.get() instanceof Uint8Array".to_string(),
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
        Kind::Str => Some("string"),
        Kind::Bytes => Some("bytes"),
        Kind::List(_) | Kind::Record(_) => Some("branch"),
        Kind::Option(_) => None,
    }
}

/// Whether a function returns `k` in its result, not its packet: a bool, an
/// unsigned integer or a handle, below 2^31.
fn in_result(k: &Kind) -> bool {
    matches!(k, Kind::Bool | Kind::Handle | Kind::Int { signed: false, .. })
}

/// The bindings of the last package of `t.dirs`.
pub fn wit_outputs(root: &Path, t: &WitTarget) -> Result<Vec<Output>, String> {
    let mut r = Resolve::default();
    let mut pkg: Option<PackageId> = None;
    for d in t.dirs {
        let (id, _) = r.push_dir(root.join(d)).map_err(|e| format!("{d}: {e:?}"))?;
        pkg = Some(id);
    }
    let pkg = pkg.ok_or("no WIT package")?;
    let p = &r.packages[pkg];
    let pname = format!("{}:{}", p.name.namespace, p.name.name);
    for &i in p.interfaces.values() {
        for &ty in r.interfaces[i].types.values() {
            let TypeDefKind::Type(Type::Id(o)) = r.types[ty].kind else { continue };
            let TypeOwner::Interface(oi) = r.types[o].owner else { continue };
            let other = package_name(&r, oi);
            if other != pname && !t.externs.iter().any(|(e, _, _)| *e == other) {
                return Err(format!("{pname} takes types of {other}, whose bindings no extern names"));
            }
        }
    }
    let src = t.dirs.last().copied().unwrap_or("");
    let mut rs = String::new();
    let mut outs = Vec::new();
    for (iname, &i) in &p.interfaces {
        let at = format!("{pname}/{iname}");
        let objs = objs(&r, i, &at)?;
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
            core: t.core,
            pkg: pname.clone(),
            uses: RefCell::default(),
            externs: t.externs,
        };
        rs.push_str(&g.rs_iface(&objs));
        rs.push('\n');
        if let Some((dir, logical)) = t.ts {
            let body = g.ts_iface(&objs);
            let imports = g.ts_imports(&body, t.runtime_ts);
            let text = format!("// {logical}/{iname}.ts\n//\n// GENERATED from {src} (xpute-gen spec/wit.rs)\n\n{imports}\n{body}");
            outs.push(Output::new(&format!("{dir}/{iname}.ts"), text));
        }
    }
    if let Some((path, logical)) = t.rs {
        let text = format!("// {logical}\n//\n// GENERATED from {src} (xpute-gen spec/wit.rs)\n\n{rs}");
        outs.push(Output::new(path, text));
    }
    Ok(outs)
}
