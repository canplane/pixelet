// xpute-link-gen/wit.rs

//! This writes the link's bindings from WIT (xpute/link/wit/types.wit), in Rust
//! and in TypeScript. For each interface it writes its methods' numbers, its
//! types, the side that serves it and the side that calls it: a singleton
//! interface's functions are one object's methods, under its named handle, the
//! FNV-1a hash of `package/interface` folded below frame.json's `NAMED`, and
//! each resource's methods another's. A method's number is its place among its
//! object's, in declaration order. No two interfaces a target reaches share a
//! named handle. In C++ it writes the numbers and the types alone (cpp.rs).
//!
//! A packet lays its values out as the Component Model's Canonical ABI does
//! (xpute-link cabi.rs): a call's params as one tuple, an answer's value
//! alone. It reads the subset the link's WIT names. Anything else is an
//! error: a resource's constructor or static, an option of an option, more
//! than 255 methods to an object.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use wit_parser::{Function, FunctionKind, IndexMap, InterfaceId, Param, Resolve, Type, TypeDefKind, TypeId, TypeOwner, WorldItem, WorldKey};

use xpute_gen::spec::{mark, of_ident, package, package_path, pascal, read, rs_enum, rs_ident, screaming, snake, ts_enum, ts_ident, Member, Output};

mod cpp;

/// One package's bindings on one side, and where they go.
pub struct WitTarget<'a> {
    /// The package's WIT directory.
    pub wit: &'a str,
    /// The Rust module file, which declares a module per interface in the
    /// directory of its name.
    pub rs: Option<&'a str>,
    /// The TypeScript directory, a module per interface.
    pub ts: Option<&'a str>,
    /// The C++ directory, a header per interface, under the library whose
    /// root holds `compile_flags.txt`.
    pub cpp: Option<&'a str>,
    /// The side the Rust runs on, or none for a package of types alone,
    /// which takes nothing of a side. The Rust is written per side, as two
    /// Rust sides share one process on a device; the TypeScript is
    /// side-agnostic, taking the link it sends on as an argument, from
    /// `link.ts` beside xpute:link's bindings.
    pub side: Option<Side<'a>>,
}

/// A side of the link, by the directory of its crate, whose `link` opens,
/// calls and sends and whose `wait` waits; the link's own items come from
/// xpute-link. A package's world is its guest's, which serves what the world
/// exports and calls what it imports; every other side, the TypeScript too,
/// serves the imports and calls the exports. A package whose interfaces have
/// functions declares its world.
pub enum Side<'a> {
    Guest(&'a str),
    Host(&'a str),
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
    let interface = &r.interfaces[i];
    let mut single = Obj {
        name: interface.name.clone().unwrap_or_default(),
        resource: false,
        methods: Vec::new(),
    };
    let mut res: BTreeMap<TypeId, Obj> = BTreeMap::new();
    let mut order = Vec::new();
    for &t in interface.types.values() {
        if matches!(r.types[t].kind, TypeDefKind::Resource) && r.types[t].owner == TypeOwner::Interface(i) {
            order.push(t);
            let name = r.types[t].name.clone().unwrap_or_default();
            res.insert(t, Obj { name, resource: true, methods: Vec::new() });
        }
    }
    for f in interface.functions.values() {
        match f.kind {
            FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => single.methods.push(f),
            FunctionKind::Method(t) | FunctionKind::AsyncMethod(t) => {
                res.get_mut(&t).ok_or(format!("{at}.{}: a method of no resource here", f.name))?.methods.push(f);
            }
            _ => return Err(format!("{at}.{}: the link carries no constructor, static, getter or setter", f.name)),
        }
    }
    // An async call lends one buffer, which its answer hands back.
    for f in interface.functions.values() {
        let lents = f.params.iter().filter(|p| is_lent(r, &p.ty)).count();
        if lents > 1 && is_async(f) {
            return Err(format!("{at}.{}: an async call lends one buffer", f.name));
        }
    }
    let mut out = Vec::new();
    if !single.methods.is_empty() {
        out.push(single);
    }
    out.extend(order.into_iter().filter_map(|t| res.remove(&t)));
    // A resource's method is named by joining its resource's name and its
    // own, which no free function's or other method's may also give.
    let mut joined = BTreeSet::new();
    for o in &out {
        if o.methods.len() > drop as usize {
            return Err(format!("{at}/{}: more than {drop} methods", o.name));
        }
        for f in &o.methods {
            if !joined.insert(snake(&object_method(o, f))) {
                return Err(format!("{at}.{}: its name, joined, is another function's", f.name));
            }
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
fn method_const(o: &Obj, f: &Function, given: fn(&str) -> bool) -> String {
    escaped(screaming(&object_method(o, f)), given)
}

/// A method's name among its interface's: a resource's prefixed by it.
fn object_method(o: &Obj, f: &Function) -> String {
    if o.resource { format!("{}-{}", o.name, method_name(f)) } else { method_name(f).to_string() }
}

/// The trait or interface that serves an object, by the side that serves
/// it (`Guest`, `Host`): a singleton's is the side's name, and a resource's
/// the side's and the resource's, as wit-bindgen's and wasmtime's are.
fn serving(o: &Obj, side: &str) -> String {
    if o.resource { format!("{side}{}", pascal(&o.name)) } else { side.into() }
}

/// The object over a served object's implementation, a name of the
/// generator's alone, which its leading `_` keeps from every WIT name.
fn object_struct(o: &Obj) -> String {
    if o.resource { format!("_{}Object", pascal(&o.name)) } else { "_Object".into() }
}

/// The names the Rust bindings give beside WIT names, those the runtime
/// module `_rt` holds, and the prelude's they name bare. A WIT name that
/// meets one takes `_`, as wit-bindgen's `Guest_` does.
const RS_GIVEN: [&str; 31] = [
    "HANDLE", "mount", "task_return", "Guest", "Host", "L", "Errno", "Future", "Handle", "Lend", "Lift", "Lower", "Method",
    "Object", "Rc", "Reply", "Return", "Tag", "Box", "Copy", "Err", "Fn", "None", "Ok", "Option", "Result", "Some",
    "String", "Unpin", "Vec", "size_of_val",
];

/// The names the TypeScript bindings give beside WIT names, those they
/// import from the kit and the link, and the globals they name. A WIT name
/// that meets one takes `_`, as jco's names keep clear of its
/// `exclude_globals`.
const TS_GIVEN: [&str; 34] = [
    "HANDLE", "mount", "object", "Host", "ERRNO_EBADMSG", "ERRNO_EINPROGRESS", "ERRNO_ENOSYS", "ComponentError",
    "Handle", "Lift", "Link", "LinkObject", "Lower", "MarshalError", "Method", "Ref", "Sender", "XputeError",
    "refusal_of", "AbortController", "AbortSignal",
    "ArrayBuffer", "BigInt64Array", "BigUint64Array", "Float32Array", "Float64Array", "Int16Array", "Int32Array",
    "Int8Array", "Pick", "Promise", "Uint16Array", "Uint32Array", "Uint8Array",
];

/// Whether `n` is a resource's serving trait or interface's form, a side's
/// name and then a word (`GuestDevice`, `HostDevice`), which any module may
/// name.
fn is_serving(n: &str) -> bool {
    ["Guest", "Host"].iter().any(|s| n.strip_prefix(s).is_some_and(|r| r.starts_with(|c: char| c.is_ascii_uppercase())))
}

/// Whether the Rust bindings give `n`: one of `RS_GIVEN`, a lent buffer's
/// generic (`T0`, …), or a resource's serving trait's form.
fn rs_given(n: &str) -> bool {
    RS_GIVEN.contains(&n)
        || n.strip_prefix('T').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        || is_serving(n)
}

/// Whether the TypeScript bindings give `n`: one of `TS_GIVEN`, or a
/// resource's serving interface's form.
fn ts_given(n: &str) -> bool {
    TS_GIVEN.contains(&n) || is_serving(n)
}

/// A WIT type's name, in a language whose bindings give the names `given`,
/// with `_` where it meets one.
fn type_name(r: &Resolve, t: TypeId, given: fn(&str) -> bool) -> String {
    escaped(pascal(r.types[t].name.as_deref().unwrap_or("anon")), given)
}

/// `n`, a name made of a WIT name, with `_` where the bindings give it.
fn escaped(n: String, given: fn(&str) -> bool) -> String {
    if given(&n) { n + "_" } else { n }
}

/// A WIT name as a Rust identifier.
fn rs_name(n: &str) -> String {
    escaped(rs_ident(&snake(n)), rs_given)
}

/// A WIT name as a TypeScript identifier.
fn ts_name(n: &str) -> String {
    escaped(ts_ident(&snake(n)), ts_given)
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

/// A number the link carries, at its WIT width.
pub(crate) struct Number {
    pub(crate) wit: Type,
    /// Its Rust type, the name of its writer and reader in both languages.
    pub(crate) rs: &'static str,
    /// Its size, which is its alignment (Canonical ABI).
    pub(crate) sz: usize,
    /// Whether it is a TypeScript `number`, and not a `bigint`: none of 64
    /// bits but the f64.
    pub(crate) in_f64: bool,
    /// Its list's TypeScript typed array, as type and as constructor.
    pub(crate) arr: &'static str,
    /// Its C++ type.
    pub(crate) cpp: &'static str,
}

pub(crate) static NUMBERS: [Number; 10] = [
    Number { wit: Type::U8, rs: "u8", sz: 1, in_f64: true, arr: "Uint8Array", cpp: "std::uint8_t" },
    Number { wit: Type::S8, rs: "i8", sz: 1, in_f64: true, arr: "Int8Array", cpp: "std::int8_t" },
    Number { wit: Type::U16, rs: "u16", sz: 2, in_f64: true, arr: "Uint16Array", cpp: "std::uint16_t" },
    Number { wit: Type::S16, rs: "i16", sz: 2, in_f64: true, arr: "Int16Array", cpp: "std::int16_t" },
    Number { wit: Type::U32, rs: "u32", sz: 4, in_f64: true, arr: "Uint32Array", cpp: "std::uint32_t" },
    Number { wit: Type::S32, rs: "i32", sz: 4, in_f64: true, arr: "Int32Array", cpp: "std::int32_t" },
    Number { wit: Type::U64, rs: "u64", sz: 8, in_f64: false, arr: "BigUint64Array", cpp: "std::uint64_t" },
    Number { wit: Type::S64, rs: "i64", sz: 8, in_f64: false, arr: "BigInt64Array", cpp: "std::int64_t" },
    Number { wit: Type::F32, rs: "f32", sz: 4, in_f64: true, arr: "Float32Array", cpp: "float" },
    Number { wit: Type::F64, rs: "f64", sz: 8, in_f64: true, arr: "Float64Array", cpp: "double" },
];

pub(crate) fn number(ty: &Type) -> Option<&'static Number> {
    NUMBERS.iter().find(|n| n.wit == *ty)
}

/// What a type is, as the link carries it.
#[derive(Clone)]
pub(crate) enum Kind {
    Bool,
    Number(&'static Number),
    Str,
    /// A list of numbers, copied out as its TypeScript typed array.
    Array(&'static Number),
    List(Box<Kind>),
    Option(Box<Kind>),
    /// WIT's `result`, its ok and its error cases, either of which may carry
    /// nothing.
    Result(Option<Box<Kind>>, Option<Box<Kind>>),
    Record(TypeId),
    Variant(TypeId),
    Enum(TypeId),
    Handle,
}

/// What `ty` is as the link carries it, or what it carries no such type as.
pub(crate) fn kind(r: &Resolve, ty: &Type, at: &str) -> Result<Kind, String> {
    if let Some(n) = number(ty) {
        return Ok(Kind::Number(n));
    }
    let t = match ty {
        Type::Bool => return Ok(Kind::Bool),
        Type::String => return Ok(Kind::Str),
        Type::Id(t) => *t,
        t => return Err(format!("{at}: the link carries no {t:?}")),
    };
    let boxed = |t: &Option<Type>| t.as_ref().map(|t| kind(r, t, at).map(Box::new)).transpose();
    Ok(match &r.types[t].kind {
        TypeDefKind::Record(_) => Kind::Record(t),
        TypeDefKind::Variant(_) => Kind::Variant(t),
        TypeDefKind::Enum(_) => Kind::Enum(t),
        TypeDefKind::List(e) => match kind(r, e, at)? {
            Kind::Number(n) => Kind::Array(n),
            k => Kind::List(Box::new(k)),
        },
        // TypeScript's absent is one null, which an option inside it would share.
        TypeDefKind::Option(e) => match kind(r, e, at)? {
            Kind::Option(_) => return Err(format!("{at}: the link carries no option of an option")),
            k => Kind::Option(Box::new(k)),
        },
        TypeDefKind::Result(res) => Kind::Result(boxed(&res.ok)?, boxed(&res.err)?),
        TypeDefKind::Type(e) => kind(r, e, at)?,
        TypeDefKind::Resource | TypeDefKind::Handle(_) => Kind::Handle,
        k => return Err(format!("{at}: the link carries no {}", k.as_str())),
    })
}

/// A variant's cases, each its name and its payload's kind, if it carries
/// one.
fn cases(r: &Resolve, v: &wit_parser::Variant, at: &str) -> Result<Vec<(String, Option<Kind>)>, String> {
    v.cases.iter().map(|c| Ok((c.name.clone(), c.ty.as_ref().map(|t| kind(r, t, at)).transpose()?))).collect()
}

/// The size of a discriminant of `n` cases, which is its alignment.
pub(crate) fn discriminant_sz(n: usize) -> usize {
    match n {
        0..=256 => 1,
        257..=65536 => 2,
        _ => 4,
    }
}

/// The Rust and TypeScript type of a discriminant of `n` cases.
pub(crate) fn discriminant_type(n: usize) -> &'static str {
    ["u8", "u16", "u32"][discriminant_sz(n).trailing_zeros() as usize]
}

/// Where fields of `layouts`, each (size, align), lie one after another, and
/// the size and alignment of them all: a record's, or a tuple's, which the
/// Canonical ABI lays out as a record.
pub(crate) fn rec_layout(layouts: &[(usize, usize)]) -> (Vec<usize>, usize, usize) {
    let mut at: usize = 0;
    let mut align = 1;
    let mut offs = Vec::new();
    for &(sz, a) in layouts {
        at = at.next_multiple_of(a);
        offs.push(at);
        at += sz;
        align = align.max(a);
    }
    (offs, at.next_multiple_of(align), align)
}

/// A variant's layout from its cases' payloads' layouts: the payload's
/// offset, and the variant's size and alignment.
pub(crate) fn variant_layout(payloads: &[Option<(usize, usize)>]) -> (usize, usize, usize) {
    let d = discriminant_sz(payloads.len());
    let align = payloads.iter().flatten().map(|&(_, a)| a).max().unwrap_or(1).max(d);
    let off = d.next_multiple_of(payloads.iter().flatten().map(|&(_, a)| a).max().unwrap_or(1));
    let sz = payloads.iter().flatten().map(|&(s, _)| s).max().unwrap_or(0);
    (off, (off + sz).next_multiple_of(align), align)
}

/// An enum's members as the enum writers take them (xpute-gen `rs_enum`,
/// `ts_enum`, `cpp_enum`): each one's identifier, its name and its number,
/// its place. Its name is its identifier.
fn members(e: &wit_parser::Enum) -> Vec<Member> {
    e.cases
        .iter()
        .enumerate()
        .map(|(k, c)| Member {
            ident: pascal(&c.name),
            name: pascal(&c.name),
            number: k.to_string(),
            doc: c.docs.contents.as_deref().map_or(Vec::new(), |d| d.lines().map(str::to_string).collect()),
        })
        .collect()
}

/// The interface a type of another one is reached through: its Rust module,
/// and its TypeScript module's path with no extension.
struct Owner {
    rs: String,
    ts: String,
}

fn package_name(r: &Resolve, i: InterfaceId) -> String {
    r.interfaces[i]
        .package
        .map(|p| format!("{}:{}", r.packages[p].name.namespace, r.packages[p].name.name))
        .unwrap_or_default()
}

/// What a side does with an interface.
#[derive(Clone, Copy)]
enum Role {
    Serve,
    Call,
}

/// The Rust modules the bindings take items from, beside the link's crate.
const RS_ERRNO: &str = "xpute_kit::status::errno";

/// The TypeScript modules the bindings take items from, beside the link's.
const TS_ERRNO: &str = "@xpute/kit/status/errno.spec.ts";
const TS_ERROR: &str = "@xpute/kit/status/error.ts";

pub(crate) struct Gen<'a> {
    r: &'a Resolve,
    here: InterfaceId,
    /// The package's `namespace:package`.
    pkg: String,
    /// The bit that marks a named handle (frame.json `NAMED`).
    named_bit: u32,
    /// The link's TypeScript modules, `link.ts` and `cabi.ts`, and its Rust
    /// crate.
    link_ts: &'a str,
    cabi_ts: &'a str,
    link_rs: &'a str,
    /// What a Rust module takes from other modules, by module, which its
    /// runtime module `_rt` re-exports.
    rs_uses: RefCell<BTreeMap<String, BTreeSet<String>>>,
    /// How deep in lists the TypeScript or the C++ being written is, which
    /// names its lists' locals.
    depth: std::cell::Cell<u32>,
    /// What a TypeScript module imports, by module: each name, and whether
    /// it is named as a type alone.
    ts_uses: RefCell<BTreeMap<String, BTreeMap<String, bool>>>,
    /// Where each other package's bindings are, by its `namespace:package`:
    /// its Rust module and its TypeScript directory.
    externs: &'a [(String, String, String)],
}

impl Gen<'_> {
    fn iname(&self) -> String {
        self.r.interfaces[self.here].name.clone().unwrap_or_default()
    }

    /// The interface's module's name, in both languages.
    fn module(&self) -> String {
        rs_name(&self.iname())
    }

    fn at(&self) -> String {
        format!("{}/{}", self.pkg, self.iname())
    }

    /// The interface's named handle (`named`).
    fn handle(&self) -> u32 {
        named(&self.at(), self.named_bit)
    }

    fn kinds(&self, f: &Function) -> Result<Vec<(String, Kind)>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        params(f).map(|p| Ok((rs_name(&p.name), kind(self.r, &p.ty, &at)?))).collect()
    }

    fn ret(&self, f: &Function) -> Result<Option<Kind>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        f.result.as_ref().map(|t| kind(self.r, t, &at)).transpose()
    }

    /// `name` of the Rust module `module`, reached through the module's
    /// runtime module `_rt`, which re-exports it, as wit-bindgen's bindings
    /// reach theirs.
    fn rs_use(&self, module: &str, name: &str) -> String {
        self.rs_uses.borrow_mut().entry(module.into()).or_default().insert(name.into());
        format!("_rt::{name}")
    }

    /// `name` of the TypeScript module `module`, which the module imports by
    /// its name, as a type alone where `is_type`.
    fn ts_use(&self, module: &str, name: &str, is_type: bool) -> String {
        let mut uses = self.ts_uses.borrow_mut();
        let t = uses.entry(module.into()).or_default().entry(name.into()).or_insert(is_type);
        *t &= is_type;
        name.into()
    }

    /// A type's name, and the interface it is reached through where it is
    /// another's.
    fn type_path(&self, t: TypeId, given: fn(&str) -> bool) -> (Option<Owner>, String) {
        let td = &self.r.types[t];
        let name = type_name(self.r, t, given);
        let TypeOwner::Interface(i) = td.owner else { return (None, name) };
        if i == self.here {
            return (None, name);
        }
        let iname = self.r.interfaces[i].name.clone().unwrap_or_default();
        let alias = rs_name(&iname);
        let pkg = package_name(self.r, i);
        let owner = if pkg == package_name(self.r, self.here) {
            Owner { rs: format!("super::{alias}"), ts: format!("./{alias}") }
        } else {
            let (rs, ts) = self.externs.iter().find(|(p, _, _)| *p == pkg).map_or(("", ""), |(_, rs, ts)| (rs.as_str(), ts.as_str()));
            Owner { rs: format!("{rs}::{alias}"), ts: format!("{ts}/{alias}") }
        };
        (Some(owner), name)
    }

    // ---- Layout ----

    /// The size and alignment of a value of `k` (Canonical ABI).
    pub(crate) fn layout(&self, k: &Kind) -> Result<(usize, usize), String> {
        Ok(match k {
            Kind::Bool => (1, 1),
            Kind::Number(n) => (n.sz, n.sz),
            Kind::Str | Kind::Array(_) | Kind::List(_) => (8, 4),
            Kind::Handle => (4, 4),
            Kind::Enum(t) => {
                let d = discriminant_sz(self.case_cnt(*t));
                (d, d)
            }
            Kind::Option(e) => {
                let (_, sz, align) = variant_layout(&[None, Some(self.layout(e)?)]);
                (sz, align)
            }
            Kind::Result(ok, err) => {
                let (_, sz, align) = self.result_layout(ok, err)?;
                (sz, align)
            }
            Kind::Record(t) => {
                let (_, sz, align) = rec_layout(&self.field_layouts(*t)?);
                (sz, align)
            }
            Kind::Variant(t) => {
                let (_, sz, align) = variant_layout(&self.payload_layouts(*t)?);
                (sz, align)
            }
        })
    }

    pub(crate) fn case_cnt(&self, t: TypeId) -> usize {
        match &self.r.types[t].kind {
            TypeDefKind::Enum(e) => e.cases.len(),
            _ => 0,
        }
    }

    /// A record's fields: each its name and its kind.
    pub(crate) fn fields(&self, t: TypeId) -> Result<Vec<(String, Kind)>, String> {
        let TypeDefKind::Record(rec) = &self.r.types[t].kind else { return Ok(Vec::new()) };
        rec.fields.iter().map(|f| Ok((f.name.clone(), kind(self.r, &f.ty, &self.at())?))).collect()
    }

    pub(crate) fn field_layouts(&self, t: TypeId) -> Result<Vec<(usize, usize)>, String> {
        self.fields(t)?.iter().map(|(_, k)| self.layout(k)).collect()
    }

    /// A variant's cases' payloads' layouts, none for a case that carries
    /// nothing.
    pub(crate) fn payload_layouts(&self, t: TypeId) -> Result<Vec<Option<(usize, usize)>>, String> {
        let TypeDefKind::Variant(v) = &self.r.types[t].kind else { return Ok(Vec::new()) };
        cases(self.r, v, &self.at())?.iter().map(|(_, k)| k.as_ref().map(|k| self.layout(k)).transpose()).collect()
    }

    pub(crate) fn result_layout(&self, ok: &Option<Box<Kind>>, err: &Option<Box<Kind>>) -> Result<(usize, usize, usize), String> {
        let payload = |k: &Option<Box<Kind>>| k.as_ref().map(|k| self.layout(k)).transpose();
        Ok(variant_layout(&[payload(ok)?, payload(err)?]))
    }

    /// Where the values `ks` lie as one tuple, and its size: a call's params.
    pub(crate) fn tuple_layout(&self, ks: &[&Kind]) -> Result<(Vec<usize>, usize), String> {
        let layouts = ks.iter().map(|k| self.layout(k)).collect::<Result<Vec<_>, String>>()?;
        let (offs, sz, _) = rec_layout(&layouts);
        Ok((offs, sz))
    }

    // ---- Rust ----

    /// Stores `e`, a reference to a value of `k`, at `at` with the writer
    /// `w`.
    fn rs_store(&self, w: &str, at: &str, e: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool => format!("{w}.bool({at}, *{e});"),
            Kind::Number(n) => format!("{w}.{}({at}, *{e});", n.rs),
            Kind::Handle => format!("{w}.u32({at}, *{e});"),
            Kind::Enum(t) => {
                let d = discriminant_type(self.case_cnt(*t));
                format!("{w}.{d}({at}, *{e} as {d});")
            }
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(_) => format!("{w}.arr({at}, {e});"),
            Kind::List(el) => {
                let (sz, align) = self.layout(el)?;
                format!(
                    "if let Some(b) = {w}.list({at}, ({e}).len(), {sz}, {align}) {{ for (i, x) in ({e}).iter().enumerate() {{ {} }} }}",
                    self.rs_store(w, &format!("b + i * {sz}"), "x", el)?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => format!("({e}).store({w}, {at});"),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)]);
                format!("if let Some(v) = {e} {{ {w}.u8({at}, 1); {} }}", self.rs_store(w, &format!("{at} + {off}"), "v", el)?)
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>| match k {
                    Some(k) => self.rs_store(w, &format!("{at} + {off}"), "v", k),
                    None => Ok(String::new()),
                };
                format!("match {e} {{ Ok(v) => {{ {} }} Err(v) => {{ {w}.u8({at}, 1); {} }} }}", case(ok)?, case(err)?)
            }
        })
    }

    /// Loads a value of `k` at `at` with the reader `r`, an expression of
    /// `Result<_, Errno>`.
    fn rs_load(&self, r: &str, at: &str, k: &Kind) -> Result<String, String> {
        let errno = self.rs_use(RS_ERRNO, "Errno");
        Ok(match k {
            Kind::Bool => format!("{r}.bool({at})"),
            Kind::Number(n) => format!("{r}.{}({at})", n.rs),
            Kind::Handle => format!("{r}.u32({at})"),
            Kind::Enum(t) => format!(
                "{r}.case({at}, {}).and_then(|v| {}::of(v).ok_or({errno}::EBADMSG))",
                self.case_cnt(*t),
                self.rs_ty(k, true)
            ),
            Kind::Str => format!("{r}.str({at}).map(str::to_owned)"),
            Kind::Array(n) => format!("{r}.arr::<{}>({at})", n.rs),
            Kind::List(el) => {
                let (sz, align) = self.layout(el)?;
                format!(
                    "{r}.list({at}, {sz}, {align}).and_then(|(b, n)| (0..n).map(|i| {}).collect::<Result<Vec<_>, {errno}>>())",
                    self.rs_load(r, &format!("b + i * {sz}"), el)?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => format!("{}::load({r}, {at})", self.rs_ty(k, true)),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)]);
                format!(
                    "{r}.case({at}, 2).and_then(|d| if d == 0 {{ Ok(None) }} else {{ {}.map(Some) }})",
                    self.rs_load(r, &format!("{at} + {off}"), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>| match k {
                    Some(k) => self.rs_load(r, &format!("{at} + {off}"), k),
                    None => Ok("Ok(())".into()),
                };
                format!(
                    "{r}.case({at}, 2).and_then(|d| if d == 0 {{ {}.map(Ok) }} else {{ {}.map(Err) }})",
                    case(ok)?,
                    case(err)?
                )
            }
        })
    }

    /// The Rust of the args `ps`: none, or their tuple's size and what writes
    /// it, each of `lents` lending memory, written as the lent record of its
    /// span (`{lent}_at_`, `{lent}_len_`), names no WIT name escapes to.
    fn rs_args(&self, ps: &[(String, Kind)], lents: &[String]) -> Result<String, String> {
        if ps.is_empty() {
            return Ok("None".into());
        }
        let (offs, sz) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
        let mut writes = Vec::new();
        for ((n, k), at) in ps.iter().zip(&offs) {
            writes.push(if lents.contains(n) {
                format!("{} {{ at: {n}_at_, len: {n}_len_ }}.store(w_, {at});", self.rs_ty(k, true))
            } else {
                self.rs_store("w_", &at.to_string(), &format!("&{n}"), k)?
            });
        }
        let w = format!("|w_: &mut {}| {{ {} }}", self.rs_use(self.link_rs, "Lower"), writes.join(" "));
        Ok(format!("Some(({sz}, &{w}))"))
    }

    /// The Rust that answers `v`, a method's value.
    fn rs_reply(&self, rk: &Option<Kind>) -> Result<String, String> {
        let reply = self.rs_use(self.link_rs, "Reply");
        Ok(match rk {
            None => format!("{reply}::from(0)"),
            Some(k) => format!(
                "{reply} {{ result: 0, data: Some(({}, Box::new(move |w: &mut {}| {{ {} }}))) }}",
                self.layout(k)?.0,
                self.rs_use(self.link_rs, "Lower"),
                self.rs_store("w", "0", "&v", k)?
            ),
        })
    }

    /// The Rust of the answer's decoding, from the result `r` and the return
    /// data `data`.
    fn rs_answer(&self, k: &Option<Kind>) -> Result<String, String> {
        let answer = self.rs_use(self.link_rs, "answer");
        Ok(match k {
            None => format!("{answer}(r)"),
            Some(k) => format!(
                "{answer}(r).and_then(|_| {{ let r_ = {}::new(data.as_deref().ok_or({}::EBADMSG)?); {} }})",
                self.rs_use(self.link_rs, "Lift"),
                self.rs_use(RS_ERRNO, "Errno"),
                self.rs_load("r_", "0", k)?
            ),
        })
    }

    /// The serving side of an object, on the side of the crate `side`: its
    /// trait, and the object over it.
    fn rs_export(&self, o: &Obj, side: &str, serves: &str, out: &mut String) -> Result<(), String> {
        let (t, obj) = (serving(o, serves), object_struct(o));
        let errno = self.rs_use(RS_ERRNO, "Errno");
        let reply = self.rs_use(self.link_rs, "Reply");
        let rc = self.rs_use("std::rc", "Rc");
        let _ = writeln!(out, "pub trait {t} {{");
        for f in &o.methods {
            let ps: Vec<String> = self.kinds(f)?.iter().map(|(n, k)| format!("{n}: {}", self.rs_ty(k, true))).collect();
            let ret = self.ret(f)?.map_or("()".into(), |k| self.rs_ty(&k, true));
            let name = rs_name(method_name(f));
            if is_async(f) {
                let _ = writeln!(out, "    fn {name}(&self, {}task_return: {}<{ret}>);", ps.iter().map(|p| format!("{p}, ")).collect::<String>(), self.rs_use(self.link_rs, "Return"));
            } else {
                let _ = writeln!(out, "    fn {name}(&self, {}) -> Result<{ret}, {errno}>;", ps.join(", "));
            }
        }
        let _ = writeln!(
            out,
            "}}\n\nstruct {obj}({rc}<dyn {t}>);\n\nimpl {} for {obj} {{\n    fn call(&self, m_: {}, tag_: {}, args_: Option<&[u8]>) -> {reply} {{\n        let run = || -> Result<{reply}, {errno}> {{\n            Ok(match m_ {{",
            self.rs_use(self.link_rs, "Object"),
            self.rs_use(self.link_rs, "Method"),
            self.rs_use(self.link_rs, "Tag")
        );
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let _ = writeln!(out, "                {} => {{", method_const(o, f, rs_given));
            if !ps.is_empty() {
                let _ = writeln!(out, "                    let r_ = {}::new(args_.ok_or({errno}::EBADMSG)?);", self.rs_use(self.link_rs, "Lift"));
            }
            let (offs, _) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
            for ((n, kd), at) in ps.iter().zip(&offs) {
                let _ = writeln!(out, "                    let {n} = {}?;", self.rs_load("r_", &at.to_string(), kd)?);
            }
            let names: String = ps.iter().map(|(n, _)| format!("{n}, ")).collect();
            let name = rs_name(method_name(f));
            let rk = self.ret(f)?;
            if is_async(f) {
                let link = self.rs_use(side, "link");
                let done = format!(
                    "{}::new(tag_, {link}::on_cancel, Box::new(move |got| {{ let reply = match got {{ Ok(v) => {}, Err(e) => {reply}::from(e as i32) }}; {link}::task_return(tag_, reply.result, reply.data.as_ref().map(|(n, w)| (*n, &**w as &dyn Fn(&mut {})))) }}))",
                    self.rs_use(self.link_rs, "Return"),
                    self.rs_reply(&rk)?,
                    self.rs_use(self.link_rs, "Lower")
                );
                let _ = writeln!(out, "                    self.0.{name}({names}{done});\n                    {reply}::from({errno}::EINPROGRESS as i32)\n                }}");
            } else {
                let _ = writeln!(out, "                    match self.0.{name}({}) {{\n                        Ok(v) => {},\n                        Err(e) => {reply}::from(e as i32),\n                    }}\n                }}", names.trim_end_matches(", "), self.rs_reply(&rk)?);
            }
        }
        let _ = writeln!(out, "                _ => {reply}::from({errno}::ENOSYS as i32),\n            }})\n        }};\n        run().unwrap_or_else(|e| {reply}::from(e as i32))\n    }}\n}}\n");
        if o.resource {
            let _ = writeln!(out, "/// The object over `imp`, for an opener to answer.\npub fn {}(imp: {rc}<dyn {t}>) -> {rc}<dyn {}> {{\n    {rc}::new({obj}(imp))\n}}\n", rs_name(&o.name), self.rs_use(self.link_rs, "Object"));
        } else {
            let _ = writeln!(out, "/// Serves `imp` under the interface's handle.\npub fn mount(imp: {rc}<dyn {t}>) {{\n    {}::mount_named(HANDLE, {rc}::new({obj}(imp)));\n}}\n", self.rs_use(side, "link"));
        }
        Ok(())
    }

    /// The calling side of an object, on the side of the crate `side`: its
    /// methods, by a handle, a singleton's its interface's named one.
    fn rs_import(&self, o: &Obj, side: &str, out: &mut String) -> Result<(), String> {
        let errno = self.rs_use(RS_ERRNO, "Errno");
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let lents: Vec<String> = params(f).filter(|p| is_lent(self.r, &p.ty)).map(|p| rs_name(&p.name)).collect();
            // An async call's one lent buffer, which its answer hands back.
            let lent = if is_async(f) { lents.first().cloned() } else { None };
            // A sync call lends slices of any plain elements, each its own; an
            // async call lends any buffer that says its span (link `Lend`).
            let lent_ty = |k: usize| if is_async(f) { "L".to_string() } else { format!("&mut [T{k}]") };
            let generic = if lents.is_empty() {
                String::new()
            } else if is_async(f) {
                format!("<L: {}>", self.rs_use(self.link_rs, "Lend"))
            } else {
                let ts: Vec<String> = (0..lents.len()).map(|k| format!("T{k}: Copy + Unpin + 'static")).collect();
                format!("<{}>", ts.join(", "))
            };
            let mut sig: Vec<String> = if o.resource { vec![format!("{}_: {}", rs_name(&o.name), self.rs_use(self.link_rs, "Handle"))] } else { vec![] };
            sig.extend(ps.iter().map(|(n, k)| match lents.iter().position(|l| l == n) {
                Some(j) => format!("{n}: {}", lent_ty(j)),
                None => format!("{n}: {}", self.rs_ty(k, false)),
            }));
            let h = if o.resource { format!("{}_", rs_name(&o.name)) } else { "HANDLE".into() };
            let rk = self.ret(f)?;
            let ret = rk.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
            let lend: String = lents
                .iter()
                .map(|l| {
                    if is_async(f) {
                        format!("let mut {l} = {l}; let ({l}_at_, {l}_len_) = {}::span(&mut {l}); ", self.rs_use(self.link_rs, "Lend"))
                    } else {
                        format!("let {l}_at_ = {}::off({l}.as_ptr().cast()); let {l}_len_ = size_of_val(&{l}[..]) as u32; ", self.rs_use(self.link_rs, "mem"))
                    }
                })
                .collect();
            let c = method_const(o, f, rs_given);
            let name = rs_name(&object_method(o, f));
            if is_async(f) {
                let (output, kept, give) = match &lent {
                    Some(l) => (format!("(L, Result<{ret}, {errno}>)"), l.clone(), format!("(lent, {})", self.rs_answer(&rk)?)),
                    None => (format!("Result<{ret}, {errno}>"), "()".into(), self.rs_answer(&rk)?),
                };
                // The call is sent here, and the future answers it.
                let _ = writeln!(out, "pub fn {name}{generic}({}) -> impl {}<Output = {output}> {{\n    {lend}\n    let sent = {}::wait({h}, {c}, {}, {kept});\n    async move {{\n        let (r, data, lent) = sent.await;\n        {give}\n    }}\n}}\n", sig.join(", "), self.rs_use("core::future", "Future"), self.rs_use(side, "wait"), self.rs_args(&ps, &lents)?);
            } else {
                let _ = writeln!(out, "pub fn {name}{generic}({}) -> Result<{ret}, {errno}> {{\n    {lend}let (r, data) = {}::call({h}, {c}, {});\n    {}\n}}\n", sig.join(", "), self.rs_use(side, "link"), self.rs_args(&ps, &lents)?, self.rs_answer(&rk)?);
            }
        }
        Ok(())
    }

    /// The interface's Rust module, by `role` on the side of its crate, none
    /// for an interface of types alone.
    fn rs_interface(&self, objs: &[Obj], role: Option<(Role, &str, &str)>) -> Result<String, String> {
        let mut items = String::new();
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(items, "/// The interface's handle, named by `{}` (frame.json `NAMED`).\npub const HANDLE: {} = {:#x};\n", self.at(), self.rs_use(self.link_rs, "Handle"), self.handle());
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(items, "/// `{}`\npub const {}: {} = {k};", f.name, method_const(o, f, rs_given), self.rs_use(self.link_rs, "Method"));
            }
        }
        items.push('\n');
        self.rs_types(&mut items)?;
        if let Some((role, side, serves)) = role {
            for o in objs {
                match role {
                    Role::Serve => self.rs_export(o, side, serves, &mut items)?,
                    Role::Call => self.rs_import(o, side, &mut items)?,
                }
            }
        }
        // What the module takes from other modules, which its runtime module
        // holds alone (wit-bindgen's `_rt`).
        let mut body = items;
        if !self.rs_uses.borrow().is_empty() {
            body.push_str("mod _rt {\n");
            for (m, names) in self.rs_uses.borrow().iter() {
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                let _ = writeln!(body, "    pub use {m}::{{{}}};", names.join(", "));
            }
            body.push_str("}\n");
        }
        Ok(format!("#![allow(unused_imports, unused_variables, dead_code, clippy::all, reason = \"generated from WIT\")]\n\n{body}"))
    }

    // ---- TypeScript ----

    fn ts_params(&self, f: &Function) -> Result<Vec<(String, Kind)>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        params(f).map(|p| Ok((ts_name(&p.name), kind(self.r, &p.ty, &at)?))).collect()
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
    fn ts_tagged(&self, ok: &Option<Box<Kind>>, err: &Option<Box<Kind>>, x: &str, e: &str) -> (String, String) {
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
            Kind::Enum(t) => format!("{w}.{}({at}, {e});", discriminant_type(self.case_cnt(*t))),
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(_) => format!("{w}.arr({at}, {e});"),
            Kind::List(el) => {
                let (sz, align) = self.layout(el)?;
                // A list inside a list takes names of its own depth.
                let d = self.depth.get();
                self.depth.set(d + 1);
                let inner = self.ts_store(w, &format!("b{d} + i{d} * {sz}"), &format!("x{d}"), el);
                self.depth.set(d);
                format!("{{ const b{d} = {w}.list({at}, {e}.length, {sz}, {align}); if (b{d} !== null) {e}.forEach((x{d}, i{d}) => {{ {} }}); }}", inner?)
            }
            Kind::Record(t) | Kind::Variant(t) => format!("{}({w}, {at}, {e});", self.ts_path(*t, "store_")),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)]);
                format!("if ({e} !== null) {{ {w}.u8({at}, 1); {} }}", self.ts_store(w, &format!("{at} + {off}"), e, el)?)
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>| match k {
                    Some(k) => self.ts_store(w, &format!("{at} + {off}"), &format!("{e}.val"), k),
                    None => Ok(String::new()),
                };
                format!("if ({e}.tag === \"ok\") {{ {} }} else {{ {w}.u8({at}, 1); {} }}", case(ok)?, case(err)?)
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
                let (sz, align) = self.layout(el)?;
                format!("{r}.map({at}, {sz}, {align}, (at) => {})", self.ts_load(r, "at", el)?)
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => format!("{}({r}, {at})", self.ts_path(*t, "load_")),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)]);
                format!("({r}.case({at}, 2) === 0 ? null : {})", self.ts_load(r, &format!("{at} + {off}"), el)?)
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |tag: &str, k: &Option<Box<Kind>>| -> Result<String, String> {
                    Ok(match k {
                        Some(k) => format!("{{ tag: \"{tag}\" as const, val: {} }}", self.ts_load(r, &format!("{at} + {off}"), k)?),
                        None => format!("{{ tag: \"{tag}\" as const }}"),
                    })
                };
                format!("({r}.case({at}, 2) === 0 ? {} : {})", case("ok", ok)?, case("err", err)?)
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
        let writes = ps.iter().zip(&offs).map(|((n, k), at)| self.ts_store("w_", &at.to_string(), n, k)).collect::<Result<Vec<_>, String>>()?;
        Ok(format!("[{sz}, (w_) => {{ {} }}]", writes.join(" ")))
    }

    /// The serving side of an object: its interface, and the object over it.
    fn ts_export(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        let link = self.link_ts;
        let t = serving(o, "Host");
        let _ = writeln!(out, "export interface {t} {{");
        for f in &o.methods {
            let mut sig: Vec<String> = self.ts_params(f)?.iter().map(|(n, k)| format!("{n}: {}", self.ts_ty(k))).collect();
            let r = self.ts_ret(f)?;
            // An async method stops on `signal_` where its caller cancels it
            // and it can; one that can't leaves it unread.
            if is_async(f) {
                sig.push("signal_: AbortSignal".into());
            }
            let r = if is_async(f) { format!("Promise<{r}>") } else { r };
            let _ = writeln!(out, "  {}({}): {r};", ts_name(method_name(f)), sig.join(", "));
        }
        let fname = if o.resource { ts_name(&o.name) } else { "object".into() };
        let pick = if o.methods.iter().any(|f| is_async(f)) { format!("l: Pick<{}, \"task_return\" | \"on_cancel\">, ", self.ts_use(link, "Link", true)) } else { String::new() };
        let refusal_of = self.ts_use(link, "refusal_of", false);
        let _ = writeln!(out, "}}\n\n/** The object over `imp`. */\n{}function {fname}({pick}imp: {t}): {} {{\n  return {{\n    call(m, tag, args) {{\n      try {{\n        switch (m) {{", if o.resource { "export " } else { "" }, self.ts_use(link, "LinkObject", true));
        for f in &o.methods {
            let ps = self.ts_params(f)?;
            let (offs, _) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
            let mut loads = ps.iter().zip(&offs).map(|((_, kd), at)| self.ts_load("r_", &at.to_string(), kd)).collect::<Result<Vec<_>, String>>()?;
            let takes_args = !loads.is_empty();
            if is_async(f) {
                loads.push("stop.signal".into());
            }
            let call = format!("imp.{}({})", ts_name(method_name(f)), loads.join(", "));
            let rk = self.ret(f)?;
            let data = match &rk {
                None => "null".to_string(),
                Some(k) => format!("[{}, (w) => {{ {} }}]", self.layout(k)?.0, self.ts_store("w", "0", "v", k)?),
            };
            let c = method_const(o, f, ts_given);
            let _ = writeln!(out, "          case {c}: {{");
            if takes_args {
                let _ = writeln!(out, "            if (args === null) return {};\n            const r_ = new {}(args);", self.ts_use(TS_ERRNO, "ERRNO_EBADMSG", false), self.ts_use(self.cabi_ts, "Lift", false));
            }
            if let Some(k @ Kind::Result(ok, err)) = &rk {
                let ce = self.ts_use(link, "ComponentError", false);
                let ty = self.ts_ty(k);
                let (tag_ok, tag_err) = self.ts_tagged(ok, err, "x", "e");
                let bind = if ok.is_some() { "(x)" } else { "()" };
                if is_async(f) {
                    let _ = writeln!(out, "            const stop = new AbortController();\n            {call}.then(\n              {bind} => {{\n                const v = {tag_ok} as {ty};\n                l.task_return(tag, 0, {data});\n              }},\n              (e: unknown) => {{\n                if (!(e instanceof {ce})) return l.task_return(tag, {refusal_of}(e));\n                const v = {tag_err} as {ty};\n                l.task_return(tag, 0, {data});\n              }},\n            );\n            l.on_cancel(tag, () => stop.abort());\n            return {};\n          }}", self.ts_use(TS_ERRNO, "ERRNO_EINPROGRESS", false));
                } else {
                    let set = if ok.is_some() { format!("const x = {call};\n              v = {tag_ok};") } else { format!("{call};\n              v = {tag_ok};") };
                    let _ = writeln!(out, "            let v: {ty};\n            try {{\n              {set}\n            }} catch (e) {{\n              if (!(e instanceof {ce})) throw e;\n              v = {tag_err};\n            }}\n            return {{ result: 0, data: {data} }};\n          }}");
                }
            } else if is_async(f) {
                let bind = if rk.is_none() { "()" } else { "(v)" };
                let _ = writeln!(out, "            const stop = new AbortController();\n            {call}.then(\n              {bind} => l.task_return(tag, 0, {data}),\n              (e: unknown) => l.task_return(tag, {refusal_of}(e)),\n            );\n            l.on_cancel(tag, () => stop.abort());\n            return {};\n          }}", self.ts_use(TS_ERRNO, "ERRNO_EINPROGRESS", false));
            } else if rk.is_none() {
                let _ = writeln!(out, "            {call};\n            return 0;\n          }}");
            } else {
                let _ = writeln!(out, "            const v = {call};\n            return {{ result: 0, data: {data} }};\n          }}");
            }
        }
        let _ = writeln!(out, "          default:\n            return {};\n        }}\n      }} catch (e) {{\n        return {refusal_of}(e);\n      }}\n    }},\n  }};\n}}\n", self.ts_use(TS_ERRNO, "ERRNO_ENOSYS", false));
        if !o.resource {
            let ln = self.ts_use(link, "Link", true);
            let pick = if pick.is_empty() { format!("l: Pick<{ln}, \"mount_named\">") } else { format!("l: Pick<{ln}, \"mount_named\" | \"task_return\" | \"on_cancel\">") };
            let pass = if o.methods.iter().any(|f| is_async(f)) { "l, " } else { "" };
            let _ = writeln!(out, "/** Serves `imp` under the interface's handle. */\nexport function mount({pick}, imp: {t}): void {{\n  l.mount_named(HANDLE, object({pass}imp));\n}}\n");
        }
        Ok(())
    }

    /// The calling side of an object: its methods as functions, each taking
    /// first the link a singleton is called over, or a resource's reference.
    fn ts_import(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        let link = self.link_ts;
        let (by, to) = if o.resource {
            (format!("ref_: {}", self.ts_use(link, "Ref", true)), "ref_.link.send(ref_.handle")
        } else {
            (format!("link_: {}", self.ts_use(link, "Sender", true)), "link_.send(HANDLE")
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
                    self.ts_use(TS_ERROR, "MarshalError", false),
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
            let _ = writeln!(out, "export async function {name}({}): Promise<{ret}> {{\n  const a_ = await {to}, {}, {});\n  if (a_.result !== 0) throw new {err}(a_.result);{take}\n}}\n", sig.join(", "), method_const(o, f, ts_given), self.ts_args(&ps)?);
        }
        Ok(())
    }

    /// The interface's TypeScript modules, by `role`, none for an interface
    /// of types alone: its functions' module, and its enums' module, empty
    /// where it has none.
    fn ts_interface(&self, objs: &[Obj], role: Option<Role>) -> Result<(String, String), String> {
        let link = self.link_ts;
        let mut out = String::new();
        let mut enums = String::new();
        if objs.iter().any(|o| !o.resource) {
            let _ = writeln!(out, "/** The interface's handle, named by `{}` (frame.json `NAMED`). */\nexport const HANDLE: {} = {:#x};\n", self.at(), self.ts_use(link, "Handle", true), self.handle());
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(out, "/** `{}` */\nexport const {}: {} = {k};", f.name, method_const(o, f, ts_given), self.ts_use(link, "Method", true));
            }
        }
        out.push('\n');
        self.ts_types(&mut out, &mut enums)?;
        if let Some(role) = role {
            for o in objs {
                match role {
                    Role::Serve => self.ts_export(o, &mut out)?,
                    Role::Call => self.ts_import(o, &mut out)?,
                }
            }
        }
        Ok((out, enums))
    }

    /// The imports of a TypeScript module.
    fn ts_imports(&self) -> String {
        let mut out = String::new();
        for (from, names) in self.ts_uses.borrow().iter() {
            if names.values().all(|t| *t) {
                let _ = writeln!(out, "import type {{ {} }} from \"{from}\";", names.keys().cloned().collect::<Vec<_>>().join(", "));
            } else {
                let names: Vec<String> = names.iter().map(|(n, t)| if *t { format!("type {n}") } else { n.clone() }).collect();
                let _ = writeln!(out, "import {{ {} }} from \"{from}\";", names.join(", "));
            }
        }
        out
    }

    // ---- Rust types ----

    fn rs_ty(&self, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => "bool".into(),
            Kind::Number(n) => n.rs.into(),
            Kind::Str => if owned { "String" } else { "&str" }.into(),
            Kind::Array(n) => if owned { format!("Vec<{}>", n.rs) } else { format!("&[{}]", n.rs) },
            Kind::List(e) => {
                let e = self.rs_ty(e, true);
                if owned { format!("Vec<{e}>") } else { format!("&[{e}]") }
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => {
                let (m, n) = self.type_path(*t, rs_given);
                let p = match m {
                    Some(m) if m.rs.starts_with("super::") => format!("{}::{n}", m.rs),
                    Some(m) => self.rs_use(&m.rs, &n),
                    None => n,
                };
                if owned || matches!(k, Kind::Enum(_)) { p } else { format!("&{p}") }
            }
            Kind::Option(e) => format!("Option<{}>", self.rs_ty(e, owned)),
            Kind::Result(ok, err) => {
                let case = |k: &Option<Box<Kind>>| k.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
                format!("Result<{}, {}>", case(ok), case(err))
            }
            Kind::Handle => "u32".into(),
        }
    }

    fn rs_types(&self, out: &mut String) -> Result<(), String> {
        let at = self.at();
        let lower = || self.rs_use(self.link_rs, "Lower");
        let lift = || self.rs_use(self.link_rs, "Lift");
        let errno = || self.rs_use(RS_ERRNO, "Errno");
        for &t in self.r.interfaces[self.here].types.values() {
            let td = &self.r.types[t];
            if !matches!(td.owner, TypeOwner::Interface(i) if i == self.here) {
                continue;
            }
            let n = type_name(self.r, t, rs_given);
            match &td.kind {
                TypeDefKind::Enum(e) => {
                    let _ = writeln!(out, "{}\n", rs_enum(&n, &members(e), "u32", "").join("\n"));
                }
                TypeDefKind::Record(_) => {
                    let fields = self.fields(t)?;
                    let (offs, sz, align) = rec_layout(&self.field_layouts(t)?);
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub struct {n} {{");
                    for (f, k) in &fields {
                        let _ = writeln!(out, "    pub {}: {},", rs_name(f), self.rs_ty(k, true));
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    pub const SZ: usize = {sz};\n    pub const ALIGN: usize = {align};\n\n    pub fn store(&self, w: &mut {}, at: usize) {{", lower());
                    for ((f, k), off) in fields.iter().zip(&offs) {
                        let _ = writeln!(out, "        {}", self.rs_store("w", &format!("at + {off}"), &format!("&self.{}", rs_name(f)), k)?);
                    }
                    let _ = writeln!(out, "    }}\n\n    pub fn load(r: {}<'_>, at: usize) -> Result<{n}, {}> {{\n        Ok({n} {{", lift(), errno());
                    for ((f, k), off) in fields.iter().zip(&offs) {
                        let _ = writeln!(out, "            {}: {}?,", rs_name(f), self.rs_load("r", &format!("at + {off}"), k)?);
                    }
                    let _ = writeln!(out, "        }})\n    }}\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let cases = cases(self.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&self.payload_layouts(t)?);
                    let d = discriminant_type(cases.len());
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub enum {n} {{");
                    for (c, k) in &cases {
                        match k {
                            Some(k) => {
                                let _ = writeln!(out, "    {}({}),", pascal(c), self.rs_ty(k, true));
                            }
                            None => {
                                let _ = writeln!(out, "    {},", pascal(c));
                            }
                        }
                    }
                    let _ = writeln!(out, "}}\n\nimpl {n} {{\n    pub const SZ: usize = {sz};\n    pub const ALIGN: usize = {align};\n\n    pub fn store(&self, w: &mut {}, at: usize) {{\n        match self {{", lower());
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(out, "            {n}::{}(v) => {{ w.{d}(at, {k}); {} }}", pascal(c), self.rs_store("w", &format!("at + {off}"), "v", kd)?);
                            }
                            None => {
                                let _ = writeln!(out, "            {n}::{} => w.{d}(at, {k}),", pascal(c));
                            }
                        }
                    }
                    let _ = writeln!(out, "        }}\n    }}\n\n    pub fn load(r: {}<'_>, at: usize) -> Result<{n}, {}> {{\n        match r.case(at, {})? {{", lift(), errno(), cases.len());
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(out, "            {k} => {}.map({n}::{}),", self.rs_load("r", &format!("at + {off}"), kd)?, pascal(c));
                            }
                            None => {
                                let _ = writeln!(out, "            {k} => Ok({n}::{}),", pascal(c));
                            }
                        }
                    }
                    let _ = writeln!(out, "            _ => Err({}::EBADMSG),\n        }}\n    }}\n}}\n", errno());
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(out, "pub type {n} = {};\n", self.rs_ty(&kind(self.r, ty, &at)?, true));
                }
                _ => {
                    kind(self.r, &Type::Id(t), &at)?;
                }
            }
        }
        Ok(())
    }

    // ---- TypeScript types ----

    /// A type's name, or one of its functions' (`load_`, `store_`): its own
    /// module's, or imported from its interface's. An enum's type is its
    /// enums' module's.
    fn ts_path(&self, t: TypeId, prefix: &str) -> String {
        let (m, n) = self.type_path(t, ts_given);
        let name = format!("{prefix}{n}");
        let is_type = prefix.is_empty();
        let base = m.map(|m| m.ts);
        if is_type && matches!(self.r.types[t].kind, TypeDefKind::Enum(_)) {
            let base = base.unwrap_or(format!("./{}", self.module()));
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
            Kind::Number(n) if n.in_f64 => "number".into(),
            Kind::Number(_) => "bigint".into(),
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
                    let own = format!("./{}.enum.ts", self.module());
                    let _ = writeln!(
                        out,
                        "export function load_{n}(r: {}, at: number): {} {{\n  const v = {}(r.case(at, {}));\n  if (v === null) throw new {}({});\n  return v;\n}}\n",
                        self.ts_use(self.cabi_ts, "Lift", true),
                        self.ts_path(t, ""),
                        self.ts_use(&own, &of_ident(&n), false),
                        e.cases.len(),
                        self.ts_use(TS_ERROR, "MarshalError", false),
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
                    let writes = fields.iter().zip(&offs).map(|((f, k), off)| self.ts_store("w", &format!("at + {off}"), &format!("v.{}", ts_name(f)), k)).collect::<Result<Vec<_>, String>>()?;
                    let _ = writeln!(out, "}}\n\nexport const {up}_SZ = {sz};\nexport const {up}_ALIGN = {align};\n\nexport function store_{n}(w: {}, at: number, v: {n}): void {{\n  {}\n}}\n\nexport function load_{n}(r: {}, at: number): {n} {{\n  return {{", self.ts_use(self.cabi_ts, "Lower", true), writes.join("\n  "), self.ts_use(self.cabi_ts, "Lift", true));
                    for ((f, k), off) in fields.iter().zip(&offs) {
                        let _ = writeln!(out, "    {}: {},", ts_name(f), self.ts_load("r", &format!("at + {off}"), k)?);
                    }
                    let _ = writeln!(out, "  }};\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let cases = cases(self.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&self.payload_layouts(t)?);
                    let d = discriminant_type(cases.len());
                    let alts: Vec<String> = cases
                        .iter()
                        .map(|(c, k)| match k {
                            Some(k) => format!("{{ tag: \"{c}\"; val: {} }}", self.ts_ty(k)),
                            None => format!("{{ tag: \"{c}\" }}"),
                        })
                        .collect();
                    let _ = writeln!(out, "export type {n} = {};\n\nexport const {up}_SZ = {sz};\nexport const {up}_ALIGN = {align};\n\nexport function store_{n}(w: {}, at: number, v: {n}): void {{\n  switch (v.tag) {{", alts.join(" | "), self.ts_use(self.cabi_ts, "Lower", true));
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        let payload = match kd {
                            Some(kd) => self.ts_store("w", &format!("at + {off}"), "v.val", kd)?,
                            None => String::new(),
                        };
                        let _ = writeln!(out, "    case \"{c}\":\n      w.{d}(at, {k});\n      {payload}\n      return;");
                    }
                    let _ = writeln!(out, "  }}\n}}\n\nexport function load_{n}(r: {}, at: number): {n} {{\n  switch (r.case(at, {})) {{", self.ts_use(self.cabi_ts, "Lift", true), cases.len());
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(out, "    case {k}:\n      return {{ tag: \"{c}\", val: {} }};", self.ts_load("r", &format!("at + {off}"), kd)?);
                            }
                            None => {
                                let _ = writeln!(out, "    case {k}:\n      return {{ tag: \"{c}\" }};");
                            }
                        }
                    }
                    let _ = writeln!(out, "  }}\n  throw new {}({});\n}}\n", self.ts_use(TS_ERROR, "MarshalError", false), self.ts_use(TS_ERRNO, "ERRNO_EBADMSG", false));
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(out, "export type {n} = {};\n", self.ts_ty(&kind(self.r, ty, &at)?));
                }
                _ => {
                    kind(self.r, &Type::Id(t), &at)?;
                }
            }
        }
        Ok(())
    }
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
    Ok(if root.join(dir) == here { "crate".into() } else { snake(&name) })
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
    let cpp_externs = t.cpp.map(|_| cpp::externs(&r, &ids, &peers)).transpose()?.unwrap_or_default();
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
    // The side's crate, as the Rust names it, and whether it is the guest.
    let side = match (&crate_dir, &t.side) {
        (Some(here), Some(Side::Guest(dir))) => Some((rs_crate(here, root, &format!("{dir}/Cargo.toml"))?, true)),
        (Some(here), Some(Side::Host(dir))) => Some((rs_crate(here, root, &format!("{dir}/Cargo.toml"))?, false)),
        _ => None,
    };
    // The link's TypeScript module `file`, beside xpute:link's bindings.
    let link_module = |file: &str| match (&ts_dir, peers.first().map_or(t.ts, |(_, p)| p.ts)) {
        (Some(here), Some(ts)) => {
            let pr = peers.first().map_or(root, |(pr, _)| pr);
            let at = Path::new(ts).parent().unwrap_or(Path::new("")).join(file);
            ts_module(here, pr, &at.to_string_lossy())
        }
        _ => Ok(String::new()),
    };
    let link_ts = link_module("link.ts")?;
    let cabi_ts = link_module("cabi.ts")?;
    // The link's crate, xpute:link's bindings' own, as the Rust names it.
    let link_rs = match (&crate_dir, peers.first().map_or(t.rs, |(_, p)| p.rs)) {
        (Some(here), Some(rs)) => rs_crate(here, peers.first().map_or(root, |(pr, _)| pr), rs)?,
        _ => String::new(),
    };
    // No two interfaces the target reaches share a named handle.
    let mut handles: BTreeMap<u32, String> = BTreeMap::new();
    for (i, interface) in r.interfaces.iter() {
        let Some(n) = &interface.name else { continue };
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
    // What the package's world exports and imports, by interface: whether
    // the guest serves it.
    let world = match p.worlds.values().collect::<Vec<_>>()[..] {
        [] => None,
        [&w] => Some(&r.worlds[w]),
        _ => return Err(format!("{pname}: more than one world")),
    };
    let has = |m: &IndexMap<WorldKey, WorldItem>, i: InterfaceId| {
        m.values().any(|w| matches!(w, WorldItem::Interface { id, .. } if *id == i))
    };
    let src = t.wit;
    let mut rs = String::new();
    let mut outs = Vec::new();
    let rs_dir = t.rs.map(|path| path.trim_end_matches(".rs"));
    for (iname, &i) in &p.interfaces {
        let at = format!("{pname}/{iname}");
        let objs = objs(&r, i, &at, drop)?;
        let g = Gen {
            r: &r,
            here: i,
            pkg: pname.clone(),
            named_bit,
            link_ts: &link_ts,
            cabi_ts: &cabi_ts,
            link_rs: &link_rs,
            rs_uses: RefCell::default(),
            ts_uses: RefCell::default(),
            depth: std::cell::Cell::default(),
            externs: &externs,
        };
        // Whether the guest serves the interface, as its world exports it, or
        // calls it, as its world imports it; none for one of types alone.
        let guest_serves = match world {
            _ if objs.is_empty() => None,
            Some(w) if has(&w.exports, i) => Some(true),
            Some(w) if has(&w.imports, i) => Some(false),
            Some(_) => return Err(format!("{at}: in no world of its package")),
            None => return Err(format!("{at}: its package has no world")),
        };
        let role = |serves: bool| if serves { Role::Serve } else { Role::Call };
        let m = rs_name(iname);
        if let Some(dir) = rs_dir {
            let rs_role = match (guest_serves, &side) {
                (None, _) => None,
                (Some(g), Some((side, guest))) => Some((role(g == *guest), side.as_str(), if *guest { "Guest" } else { "Host" })),
                (Some(_), None) => return Err(format!("{at}: its target names no side")),
            };
            let path = format!("{dir}/{m}.rs");
            let text = format!("// {}\n//\n// {}\n\n{}", package_path(root, &path)?, mark(src, TOOL), g.rs_interface(&objs, rs_role)?);
            outs.push(Output::new(&path, text));
            let _ = writeln!(rs, "pub mod {m};");
        }
        if let Some(dir) = t.ts {
            let (body, enums) = g.ts_interface(&objs, guest_serves.map(|g| role(!g)))?;
            if !enums.is_empty() {
                let path = format!("{dir}/{m}.enum.ts");
                let text = format!("// {}\n//\n// {}\n{enums}", package_path(root, &path)?, mark(src, TOOL));
                outs.push(Output::new(&path, text));
            }
            let path = format!("{dir}/{m}.ts");
            let text = format!("// {}\n//\n// {}\n\n{}\n{body}", package_path(root, &path)?, mark(src, TOOL), g.ts_imports());
            outs.push(Output::new(&path, text));
        }
        if let Some(dir) = t.cpp {
            let path = format!("{dir}/{m}.hpp");
            outs.push(Output::new(&path, cpp::hdr(&g, &objs, &path, src, &cpp_externs)?));
        }
    }
    if let Some(path) = t.rs {
        let text = format!("// {}\n//\n// {}\n\n{rs}", package_path(root, path)?, mark(src, TOOL));
        outs.push(Output::new(path, text));
    }
    Ok(outs)
}
