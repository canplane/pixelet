// xpute-link-gen/wit.rs

//! This writes the link's bindings from WIT (xpute/link/wit/types.wit), in Rust,
//! TypeScript and C++. For each interface it writes its methods' numbers, its
//! types, the side that serves it and the side that calls it: a singleton
//! interface's functions are one object's methods, under its named handle, the
//! FNV-1a hash of `package/interface` folded below frame.json5's `NAMED`, and
//! each resource's methods another's. A method's number is its place among its
//! object's, in declaration order. No two interfaces a target reaches share a
//! named handle. Each language is written by its own module, from the one
//! reading of the WIT here: Rust (rs.rs), TypeScript (ts.rs), and in C++ the
//! guest's side alone (cpp.rs).
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

use wit_parser::{
    Function, FunctionKind, IndexMap, InterfaceId, Param, Resolve, Type, TypeDefKind, TypeId,
    TypeOwner, WorldItem, WorldKey,
};

use xpute_gen::spec::{
    int_of, is_float, mark, package, package_path, pascal, read, rs_crate, rs_ident, rs_module,
    screaming, snake, ts_module, Member, Output,
};

mod cpp;
mod rs;
mod ts;

/// One package's bindings on one side, and where they go.
pub struct WitTarget<'a> {
    /// The package's WIT directory.
    pub wit: &'a str,
    /// The Rust module file, which declares a module per interface in the
    /// directory of its name, or is itself the module of a package's one
    /// interface of its name (`folded`).
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
        if matches!(r.types[t].kind, TypeDefKind::Resource)
            && r.types[t].owner == TypeOwner::Interface(i)
        {
            order.push(t);
            let name = r.types[t].name.clone().unwrap_or_default();
            res.insert(
                t,
                Obj {
                    name,
                    resource: true,
                    methods: Vec::new(),
                },
            );
        }
    }
    for f in interface.functions.values() {
        match f.kind {
            FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => single.methods.push(f),
            FunctionKind::Method(t) | FunctionKind::AsyncMethod(t) => {
                res.get_mut(&t)
                    .ok_or(format!("{at}.{}: a method of no resource here", f.name))?
                    .methods
                    .push(f);
            }
            _ => {
                return Err(format!(
                    "{at}.{}: the link carries no constructor, static, getter or setter",
                    f.name
                ))
            }
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
                return Err(format!(
                    "{at}.{}: its name, joined, is another function's",
                    f.name
                ));
            }
        }
    }
    Ok(out)
}

fn is_async(f: &Function) -> bool {
    matches!(
        f.kind,
        FunctionKind::AsyncFreestanding | FunctionKind::AsyncMethod(_)
    )
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
    if o.resource {
        format!("{}-{}", o.name, method_name(f))
    } else {
        method_name(f).to_string()
    }
}

/// The trait or interface that serves an object, by the side that serves
/// it (`Guest`, `Host`): a singleton's is the side's name, and a resource's
/// the side's and the resource's, as wit-bindgen's and wasmtime's are.
fn serving(o: &Obj, side: &str) -> String {
    if o.resource {
        format!("{side}{}", pascal(&o.name))
    } else {
        side.into()
    }
}

/// The names the Rust bindings give beside WIT names, those the runtime
/// module `_rt` holds, and the prelude's they name bare. A WIT name that
/// meets one takes `_`, as wit-bindgen's `Guest_` does.
const RS_GIVEN: [&str; 33] = [
    "HANDLE",
    "IMPORTS",
    "LATER",
    "mount",
    "task_return",
    "Guest",
    "Host",
    "L",
    "Errno",
    "Future",
    "Handle",
    "Lend",
    "Lift",
    "Lower",
    "Method",
    "Object",
    "Rc",
    "Reply",
    "Return",
    "Tag",
    "Box",
    "Copy",
    "Err",
    "Fn",
    "None",
    "Ok",
    "Option",
    "Result",
    "Some",
    "String",
    "Unpin",
    "Vec",
    "size_of_val",
];

/// The names the TypeScript bindings give beside WIT names, those they
/// import from the kit and the link, and the globals they name. A WIT name
/// that meets one takes `_`, as jco's names keep clear of its
/// `exclude_globals`.
const TS_GIVEN: [&str; 33] = [
    "HANDLE",
    "mount",
    "object",
    "Host",
    "ERRNO_EBADMSG",
    "ERRNO_EINPROGRESS",
    "ERRNO_ENOSYS",
    "ComponentError",
    "Handle",
    "Lift",
    "LinkObject",
    "Lower",
    "Method",
    "Ref",
    "Sender",
    "Side",
    "XputeError",
    "refusal_of",
    "AbortController",
    "AbortSignal",
    "ArrayBuffer",
    "BigInt64Array",
    "BigUint64Array",
    "Float32Array",
    "Float64Array",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "Pick",
    "Promise",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
];

/// The modules the TypeScript bindings give beside interfaces' modules in a
/// package with a world: its tables of the interfaces it imports
/// (imports.ts) and of methods answered later (later.ts). An interface's
/// module that meets one takes `_`, as a name in `TS_GIVEN` does.
const TS_GIVEN_MODULES: [&str; 2] = ["imports", "later"];

/// An interface's TypeScript module, escaped where its package has a world
/// (`TS_GIVEN_MODULES`).
fn interface_ts_module(r: &Resolve, i: InterfaceId) -> String {
    let m = rs_name(r.interfaces[i].name.as_deref().unwrap_or_default());
    let world = r.interfaces[i]
        .package
        .is_some_and(|p| !r.packages[p].worlds.is_empty());
    if world {
        escaped(m, |n| TS_GIVEN_MODULES.contains(&n))
    } else {
        m
    }
}

/// Whether `n` is a resource's serving trait or interface's form, a side's
/// name and then a word (`GuestDevice`, `HostDevice`), which any module may
/// name.
fn is_serving(n: &str) -> bool {
    ["Guest", "Host"].iter().any(|s| {
        n.strip_prefix(s)
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_uppercase()))
    })
}

/// Whether the Rust bindings give `n`: one of `RS_GIVEN`, a lent buffer's
/// generic (`T0`, …), or a resource's serving trait's form.
fn rs_given(n: &str) -> bool {
    RS_GIVEN.contains(&n)
        || n.strip_prefix('T')
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
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
    if given(&n) {
        n + "_"
    } else {
        n
    }
}

/// A WIT name as a Rust identifier.
fn rs_name(n: &str) -> String {
    escaped(rs_ident(&snake(n)), rs_given)
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
    let TypeOwner::Interface(i) = td.owner else {
        return false;
    };
    td.name.as_deref() == Some("lent")
        && r.interfaces[i].name.as_deref() == Some("types")
        && package_name(r, i) == "xpute:link"
}

/// A number the link carries, at its WIT width: an integer of xpute-gen's
/// `INTS`, which gives its spellings and its range, or a float.
pub(crate) struct Number {
    pub(crate) wit: Type,
    /// Its Rust type, the name of its writer and reader in Rust and TypeScript.
    pub(crate) rs: &'static str,
    /// Its size, which is its alignment (Canonical ABI).
    pub(crate) sz: usize,
    /// Its list's TypeScript typed array, as type and as constructor.
    pub(crate) arr: &'static str,
}

impl Number {
    /// Its C++ type: a float's, `float` of 4 bytes and `double` of 8, or its
    /// integer's.
    pub(crate) fn cpp(&self) -> &'static str {
        match self.rs {
            r if is_float(r) => {
                if self.sz == 4 {
                    "float"
                } else {
                    "double"
                }
            }
            r => int_of(r).map_or("", |i| i.cpp),
        }
    }
}

pub(crate) static NUMBERS: [Number; 10] = [
    Number {
        wit: Type::U8,
        rs: "u8",
        sz: 1,
        arr: "Uint8Array",
    },
    Number {
        wit: Type::S8,
        rs: "i8",
        sz: 1,
        arr: "Int8Array",
    },
    Number {
        wit: Type::U16,
        rs: "u16",
        sz: 2,
        arr: "Uint16Array",
    },
    Number {
        wit: Type::S16,
        rs: "i16",
        sz: 2,
        arr: "Int16Array",
    },
    Number {
        wit: Type::U32,
        rs: "u32",
        sz: 4,
        arr: "Uint32Array",
    },
    Number {
        wit: Type::S32,
        rs: "i32",
        sz: 4,
        arr: "Int32Array",
    },
    Number {
        wit: Type::U64,
        rs: "u64",
        sz: 8,
        arr: "BigUint64Array",
    },
    Number {
        wit: Type::S64,
        rs: "i64",
        sz: 8,
        arr: "BigInt64Array",
    },
    Number {
        wit: Type::F32,
        rs: "f32",
        sz: 4,
        arr: "Float32Array",
    },
    Number {
        wit: Type::F64,
        rs: "f64",
        sz: 8,
        arr: "Float64Array",
    },
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
            Kind::Option(_) => {
                return Err(format!("{at}: the link carries no option of an option"))
            }
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
fn cases(
    r: &Resolve,
    v: &wit_parser::Variant,
    at: &str,
) -> Result<Vec<(String, Option<Kind>)>, String> {
    v.cases
        .iter()
        .map(|c| {
            Ok((
                c.name.clone(),
                c.ty.as_ref().map(|t| kind(r, t, at)).transpose()?,
            ))
        })
        .collect()
}

/// The integer a discriminant of `n` cases is held in: the narrowest
/// unsigned one of 4 bytes or fewer that numbers them (Canonical ABI). More
/// cases than a u32 numbers are refused.
pub(crate) fn discriminant(n: usize) -> Result<&'static Number, String> {
    let last = n.max(1) as f64 - 1.0;
    NUMBERS
        .iter()
        .find(|x| x.sz <= 4 && int_of(x.rs).is_some_and(|i| i.lo == 0.0 && i.holds(last)))
        .ok_or(format!("{n} cases are more than a discriminant numbers"))
}

/// Whether the interface `i` is its package's module `module` itself, not a
/// module within it: the package's one interface, of the module's name. A
/// Rust module and a C++ namespace fold so (cpp.rs `is_folded`).
fn folded(r: &Resolve, i: InterfaceId, module: &str) -> bool {
    let iface = &r.interfaces[i];
    let one = iface
        .package
        .is_some_and(|p| r.packages[p].interfaces.len() == 1);
    one && iface.name.as_deref().is_some_and(|n| rs_name(n) == module)
}

/// The offset `off` past `at`, as code writes it: `at` itself at 0, and the
/// sum where `at` is a number.
fn plus(at: &str, off: usize) -> String {
    match at.parse::<usize>() {
        Ok(n) => (n + off).to_string(),
        Err(_) if off == 0 => at.into(),
        Err(_) => format!("{at} + {off}"),
    }
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
pub(crate) fn variant_layout(
    payloads: &[Option<(usize, usize)>],
) -> Result<(usize, usize, usize), String> {
    let d = discriminant(payloads.len())?.sz;
    let align = payloads
        .iter()
        .flatten()
        .map(|&(_, a)| a)
        .max()
        .unwrap_or(1)
        .max(d);
    let off = d.next_multiple_of(
        payloads
            .iter()
            .flatten()
            .map(|&(_, a)| a)
            .max()
            .unwrap_or(1),
    );
    let sz = payloads
        .iter()
        .flatten()
        .map(|&(s, _)| s)
        .max()
        .unwrap_or(0);
    Ok((off, (off + sz).next_multiple_of(align), align))
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
            doc: c
                .docs
                .contents
                .as_deref()
                .map_or(Vec::new(), |d| d.lines().map(str::to_string).collect()),
        })
        .collect()
}

/// Where an interface's bindings are reached from another module, to name a
/// type or a constant of its: its Rust module, and its TypeScript module's
/// path with no extension.
struct Owner {
    rs: String,
    ts: String,
}

/// Where the interface `i`'s bindings are reached from a module of the
/// package `own`: its Rust module, `rs_up` and its name for one of `own`'s,
/// and its TypeScript module, without its extension; another package's at
/// its peer's (`externs`).
fn owner(
    r: &Resolve,
    externs: &[(String, String, String)],
    own: &str,
    rs_up: &str,
    i: InterfaceId,
) -> Owner {
    let alias = rs_name(r.interfaces[i].name.as_deref().unwrap_or_default());
    let ts_alias = interface_ts_module(r, i);
    let pkg = package_name(r, i);
    if pkg == own {
        return Owner {
            rs: format!("{rs_up}{alias}"),
            ts: format!("./{ts_alias}"),
        };
    }
    let (rs, ts) = externs
        .iter()
        .find(|(p, _, _)| *p == pkg)
        .map_or(("", ""), |(_, rs, ts)| (rs.as_str(), ts.as_str()));
    let module = rs.rsplit("::").next().unwrap_or_default();
    Owner {
        rs: if folded(r, i, module) {
            rs.into()
        } else {
            format!("{rs}::{alias}")
        },
        ts: format!("{ts}/{ts_alias}"),
    }
}

/// The TypeScript imports of the interfaces' modules a table names, each
/// module as its own name, one line a module; no two of the modules share a
/// name (the world's imports are refused where they would).
fn ts_namespaces<'a>(reaches: impl Iterator<Item = &'a (String, String, String)>) -> String {
    let modules: BTreeMap<_, _> = reaches
        .map(|(_, ts, m)| (m.as_str(), ts.as_str()))
        .collect();
    modules
        .iter()
        .map(|(m, ts)| format!("import * as {m} from \"{ts}.ts\";\n"))
        .collect()
}

fn package_name(r: &Resolve, i: InterfaceId) -> String {
    r.interfaces[i]
        .package
        .map(|p| {
            format!(
                "{}:{}",
                r.packages[p].name.namespace, r.packages[p].name.name
            )
        })
        .unwrap_or_default()
}

/// What a side does with an interface.
#[derive(Clone, Copy)]
enum Role {
    Serve,
    Call,
}

pub(crate) struct Gen<'a> {
    r: &'a Resolve,
    here: InterfaceId,
    /// The package's `namespace:package`.
    pkg: String,
    /// The bit that marks a named handle (frame.json5 `NAMED`).
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
        self.r.interfaces[self.here]
            .name
            .clone()
            .unwrap_or_default()
    }

    /// The interface's TypeScript module's name (`interface_ts_module`).
    fn module_ts(&self) -> String {
        interface_ts_module(self.r, self.here)
    }

    fn at(&self) -> String {
        format!("{}/{}", self.pkg, self.iname())
    }

    /// The interface's named handle (`named`).
    fn handle(&self) -> u32 {
        named(&self.at(), self.named_bit)
    }

    fn ret(&self, f: &Function) -> Result<Option<Kind>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        f.result.as_ref().map(|t| kind(self.r, t, &at)).transpose()
    }

    /// A type's name, and the interface it is reached through where it is
    /// another's.
    fn type_path(&self, t: TypeId, given: fn(&str) -> bool) -> (Option<Owner>, String) {
        let td = &self.r.types[t];
        let name = type_name(self.r, t, given);
        let TypeOwner::Interface(i) = td.owner else {
            return (None, name);
        };
        if i == self.here {
            return (None, name);
        }
        let own = package_name(self.r, self.here);
        (Some(owner(self.r, self.externs, &own, "super::", i)), name)
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
                let d = discriminant(self.case_cnt(*t))?.sz;
                (d, d)
            }
            Kind::Option(e) => {
                let (_, sz, align) = variant_layout(&[None, Some(self.layout(e)?)])?;
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
                let (_, sz, align) = variant_layout(&self.payload_layouts(*t)?)?;
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
        let TypeDefKind::Record(rec) = &self.r.types[t].kind else {
            return Ok(Vec::new());
        };
        rec.fields
            .iter()
            .map(|f| Ok((f.name.clone(), kind(self.r, &f.ty, &self.at())?)))
            .collect()
    }

    pub(crate) fn field_layouts(&self, t: TypeId) -> Result<Vec<(usize, usize)>, String> {
        self.fields(t)?
            .iter()
            .map(|(_, k)| self.layout(k))
            .collect()
    }

    /// A variant's cases' payloads' layouts, none for a case that carries
    /// nothing.
    pub(crate) fn payload_layouts(&self, t: TypeId) -> Result<Vec<Option<(usize, usize)>>, String> {
        let TypeDefKind::Variant(v) = &self.r.types[t].kind else {
            return Ok(Vec::new());
        };
        cases(self.r, v, &self.at())?
            .iter()
            .map(|(_, k)| k.as_ref().map(|k| self.layout(k)).transpose())
            .collect()
    }

    pub(crate) fn result_layout(
        &self,
        ok: &Option<Box<Kind>>,
        err: &Option<Box<Kind>>,
    ) -> Result<(usize, usize, usize), String> {
        let payload = |k: &Option<Box<Kind>>| k.as_ref().map(|k| self.layout(k)).transpose();
        variant_layout(&[payload(ok)?, payload(err)?])
    }

    /// Where the values `ks` lie as one tuple, and its size: a call's params.
    pub(crate) fn tuple_layout(&self, ks: &[&Kind]) -> Result<(Vec<usize>, usize), String> {
        let layouts = ks
            .iter()
            .map(|k| self.layout(k))
            .collect::<Result<Vec<_>, String>>()?;
        let (offs, sz, _) = rec_layout(&layouts);
        Ok((offs, sz))
    }
}

/// The root module of one side's bindings, `path` from `root`: a module to
/// each of `targets` whose Rust is written in the directory of its name.
pub fn abi_root(root: &Path, path: &str, targets: &[&WitTarget]) -> Result<Output, String> {
    let dir = Path::new(path.trim_end_matches(".rs"));
    let mods: String = targets
        .iter()
        .filter_map(|t| {
            Path::new(t.rs?)
                .strip_prefix(dir)
                .ok()?
                .file_stem()?
                .to_str()
        })
        .map(|m| format!("pub mod {m};\n"))
        .collect();
    let text = format!(
        "// {}\n//\n// {}\n\n{mods}",
        package_path(root, path)?,
        mark("wit", TOOL)
    );
    Ok(Output::new(path, text))
}

/// The named handle of the interface `name` (`namespace:package/interface`):
/// `named_bit`, frame.json5's `NAMED`, and below it the FNV-1a hash of the name
/// folded to the bits under `named_bit`.
fn named(name: &str, named_bit: u32) -> u32 {
    let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    });
    let below = named_bit - 1;
    named_bit | ((hash >> named_bit.trailing_zeros()) ^ hash) & below
}

/// The value of frame.json5's `name`, beside xpute:link's WIT, `link`:
/// `NAMED`, the bit of a named handle, or `DROP`, the drop's method, the bound
/// of an object's methods, which none takes.
fn frame_val(link: &Path, name: &str) -> Result<u32, String> {
    let frame = link.join("../spec/frame.json5");
    let spec = read(&frame.to_string_lossy())?;
    spec.get(name)
        .and_then(|n| n.val().num())
        .map(|n| n as u32)
        .ok_or(format!("{}: no {name}", frame.display()))
}

/// The bindings of `t`'s package. `peers` are the targets of its side, each
/// with its root, xpute:link's first, then each before those that take its
/// types: their WIT is read before its own, the first's names frame.json5, and
/// each other package its types take is reached where its peer writes it.
pub fn wit_outputs(
    root: &Path,
    t: &WitTarget,
    peers: &[(&Path, &WitTarget)],
) -> Result<Vec<Output>, String> {
    let own = root.join(t.wit);
    let peers: Vec<_> = peers.iter().filter(|(r, p)| r.join(p.wit) != own).collect();
    let mut r = Resolve::default();
    let mut ids = Vec::new();
    for (pr, p) in &peers {
        let (id, _) = r
            .push_dir(pr.join(p.wit))
            .map_err(|e| format!("{}: {e:?}", p.wit))?;
        ids.push(id);
    }
    let (pkg, _) = r.push_dir(&own).map_err(|e| format!("{}: {e:?}", t.wit))?;
    let p = &r.packages[pkg];
    let pname = format!("{}:{}", p.name.namespace, p.name.name);
    let link = peers.first().map_or(own.clone(), |(pr, p)| pr.join(p.wit));
    let named_bit = frame_val(&link, "NAMED")?;
    let drop = frame_val(&link, "DROP")?;
    let crate_dir =
        t.rs.map(|rs| package(root, rs).map(|(_, dir)| root.join(dir)))
            .transpose()?;
    let ts_dir = t.ts.map(|d| root.join(d));
    let cpp_externs = t
        .cpp
        .map(|_| cpp::externs(&r, &ids, &peers))
        .transpose()?
        .unwrap_or_default();
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
        (Some(here), Some(Side::Guest(dir))) => {
            Some((rs_crate(here, root, &format!("{dir}/Cargo.toml"))?, true))
        }
        (Some(here), Some(Side::Host(dir))) => {
            Some((rs_crate(here, root, &format!("{dir}/Cargo.toml"))?, false))
        }
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
            let TypeDefKind::Type(Type::Id(o)) = r.types[ty].kind else {
                continue;
            };
            let TypeOwner::Interface(oi) = r.types[o].owner else {
                continue;
            };
            let other = package_name(&r, oi);
            if other != pname && !externs.iter().any(|(e, _, _)| *e == other) {
                return Err(format!(
                    "{pname} takes types of {other}, which no peer writes"
                ));
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
        m.values()
            .any(|w| matches!(w, WorldItem::Interface { id, .. } if *id == i))
    };
    let src = t.wit;
    let mut rs = String::new();
    let mut outs = Vec::new();
    let rs_dir = t.rs.map(|path| path.trim_end_matches(".rs"));
    let package_rs =
        t.rs.and_then(|path| Path::new(path).file_stem()?.to_str())
            .unwrap_or_default();
    // The methods of the named interfaces the host serves that answer later,
    // from WIT's `async func`, each its interface's reach (`reach`) and its
    // method's Rust and TypeScript constants.
    let mut later = Vec::new();
    // Where the package's module, or a module beside it, reaches the
    // interface `i`: its Rust module's prefix, none where `i` is folded into
    // the package's module, and its TypeScript module and that module's name.
    let reach = |i: InterfaceId| {
        let o = owner(&r, &externs, &pname, "", i);
        let own_folded = package_name(&r, i) == pname && folded(&r, i, package_rs);
        let rs = if own_folded {
            String::new()
        } else {
            format!("{}::", o.rs)
        };
        (rs, o.ts, interface_ts_module(&r, i))
    };
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
        if guest_serves == Some(false) {
            for o in objs.iter().filter(|o| !o.resource) {
                for f in o.methods.iter().filter(|f| is_async(f)) {
                    later.push((
                        reach(i),
                        method_const(o, f, rs_given),
                        method_const(o, f, ts_given),
                    ));
                }
            }
        }
        if let Some(dir) = rs_dir {
            let rs_role = match (guest_serves, &side) {
                (None, _) => None,
                (Some(g), Some((side, guest))) => Some((
                    role(g == *guest),
                    side.as_str(),
                    if *guest { "Guest" } else { "Host" },
                )),
                (Some(_), None) => return Err(format!("{at}: its target names no side")),
            };
            if folded(&r, i, package_rs) {
                rs.push_str(&g.rs_interface(&objs, rs_role)?);
            } else {
                let path = format!("{dir}/{m}.rs");
                let text = format!(
                    "// {}\n//\n// {}\n\n{}",
                    package_path(root, &path)?,
                    mark(src, TOOL),
                    g.rs_interface(&objs, rs_role)?
                );
                outs.push(Output::new(&path, text));
                let _ = writeln!(rs, "pub mod {m};");
            }
        }
        if let Some(dir) = t.ts {
            let (body, enums) = g.ts_interface(&objs, guest_serves.map(|g| role(!g)))?;
            let m = g.module_ts();
            if !enums.is_empty() {
                let path = format!("{dir}/{m}.enum.ts");
                let text = format!(
                    "// {}\n//\n// {}\n{enums}",
                    package_path(root, &path)?,
                    mark(src, TOOL)
                );
                outs.push(Output::new(&path, text));
            }
            let path = format!("{dir}/{m}.ts");
            let text = format!(
                "// {}\n//\n// {}\n\n{}\n{body}",
                package_path(root, &path)?,
                mark(src, TOOL),
                g.ts_imports()
            );
            outs.push(Output::new(&path, text));
        }
        if let Some(dir) = t.cpp {
            let cpp_role = match (guest_serves, &t.side) {
                (None, _) => None,
                (Some(g), Some(Side::Guest(_))) => Some(role(g)),
                (Some(_), _) => return Err(format!("{at}: C++ is written for the guest alone")),
            };
            let path = if cpp::is_folded(&r, i) {
                format!("{dir}.hpp")
            } else {
                format!("{dir}/{m}.hpp")
            };
            outs.push(Output::new(
                &path,
                cpp::hdr(&g, &objs, cpp_role, &path, src, &cpp_externs)?,
            ));
        }
    }
    // The interfaces the world imports, in the world's order, each its name
    // and its reach (`reach`); one of types alone has none. The bindings name
    // each by its module, in Rust beside the package's own modules and in
    // TypeScript by the same name, so no other import, nor for one of
    // another package any interface of this one, has its module's name.
    let mut imports: Vec<(String, (String, String, String))> = Vec::new();
    for item in world.into_iter().flat_map(|w| w.imports.values()) {
        let WorldItem::Interface { id, .. } = *item else {
            continue;
        };
        let Some(n) = &r.interfaces[id].name else {
            continue;
        };
        let name = format!("{}/{n}", package_name(&r, id));
        if objs(&r, id, &name, drop)?.is_empty() {
            continue;
        }
        let module = rs_name(n);
        let outside = package_name(&r, id) != pname;
        let mut imported = imports.iter().map(|(o, _)| o.clone());
        let mut own_names = p
            .interfaces
            .keys()
            .filter(|_| outside)
            .map(|i| format!("{pname}/{i}"));
        let same_module = |o: &String| {
            o.rsplit_once('/')
                .is_some_and(|(_, i)| rs_name(i) == module)
        };
        if let Some(other) = imported
            .find(same_module)
            .or_else(|| own_names.find(same_module))
        {
            return Err(format!("{pname}: its world imports {name}, whose module {module} is {other}'s too, and the bindings name each import by its module"));
        }
        imports.push((name, reach(id)));
    }
    if let (Some((_, false)), Some(_)) = (&side, world) {
        // An interface of another package is brought in by a `use` of its
        // module and named by that module (`imports`).
        let mut uses = Vec::new();
        let mut pairs = Vec::new();
        for (n, (rs, ..)) in &imports {
            let Some((_, module)) = rs.trim_end_matches("::").rsplit_once("::") else {
                pairs.push(format!("(\"{n}\", {rs}HANDLE)"));
                continue;
            };
            uses.push(format!("use {};", rs.trim_end_matches("::")));
            pairs.push(format!("(\"{n}\", {module}::HANDLE)"));
        }
        if !uses.is_empty() {
            let _ = write!(rs, "\n{}\n", uses.join("\n"));
        }
        let _ = writeln!(
            rs,
            "\n/// The interfaces the world imports, each `(name, handle)`, which are\n/// mounted before the guest boots: one with functions of its own under its\n/// handle, one of resources alone by an opener at a prefix.\npub const IMPORTS: &[(&str, {link_rs}::Handle)] = &[{}];",
            pairs.join(", ")
        );
        let pairs: Vec<_> = later
            .iter()
            .map(|((rs, ..), c, _)| format!("({rs}HANDLE, {rs}{c})"))
            .collect();
        let _ = writeln!(
            rs,
            "\n/// The methods of the named interfaces the host serves that answer\n/// later, each `(handle, method)`, as WIT's `async func` marks them.\npub const LATER: &[({link_rs}::Handle, {link_rs}::Method)] = &[{}];",
            pairs.join(", ")
        );
    }
    if let (Some(dir), Some(_)) = (t.ts, world) {
        let uses = ts_namespaces(imports.iter().map(|(_, reach)| reach));
        let pairs: Vec<_> = imports
            .iter()
            .map(|(n, (_, _, m))| format!("[\"{n}\", {m}.HANDLE]"))
            .collect();
        let path = format!("{dir}/imports.ts");
        let text = format!(
            "// {}\n//\n// {}\n\nimport type {{ Handle }} from \"{link_ts}\";\n{uses}\n/** The interfaces the world imports, each `[name, handle]`, which are\n * mounted before the guest boots: one with functions of its own under its\n * handle, one of resources alone by an opener at a prefix. */\nexport const IMPORTS: readonly (readonly [string, Handle])[] = [{}];\n",
            package_path(root, &path)?,
            mark(src, TOOL),
            pairs.join(", ")
        );
        outs.push(Output::new(&path, text));
        let uses = ts_namespaces(later.iter().map(|(reach, ..)| reach));
        let pairs: Vec<_> = later
            .iter()
            .map(|((_, _, m), _, c)| format!("[{m}.HANDLE, {m}.{c}]"))
            .collect();
        let path = format!("{dir}/later.ts");
        let text = format!(
            "// {}\n//\n// {}\n\nimport type {{ Handle, Method }} from \"{link_ts}\";\n{uses}\n/** The methods of the named interfaces the host serves that answer\n * later, each `[handle, method]`, as WIT's `async func` marks them. */\nexport const LATER: readonly (readonly [Handle, Method])[] = [{}];\n",
            package_path(root, &path)?,
            mark(src, TOOL),
            pairs.join(", ")
        );
        outs.push(Output::new(&path, text));
    }
    if let Some(path) = t.rs {
        let text = format!(
            "// {}\n//\n// {}\n\n{rs}",
            package_path(root, path)?,
            mark(src, TOOL)
        );
        outs.push(Output::new(path, text));
    }
    Ok(outs)
}

#[cfg(test)]
#[path = "wit.test.rs"]
mod test;
