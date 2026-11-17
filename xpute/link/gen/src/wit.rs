// xpute-link-gen/wit.rs

//! This writes the link's bindings from WIT (xpute/link/wit/types.wit), in Rust,
//! TypeScript and C++. For each interface it writes its methods' numbers, its
//! types, the side that implements it and the side that calls it: a singleton
//! interface's functions are one object's methods, under its named handle, the
//! FNV-1a hash of `package/interface` folded below frame.json5's `NAMED`, and
//! each resource's methods another's. A method's number is its place among its
//! object's, in declaration order. No two interfaces a target reaches share a
//! named handle. Each language is written by its own module, from the one
//! reading of the WIT here: Rust for either side (rs.rs), TypeScript for a
//! host alone (ts.rs) and C++ for the guest alone (cpp.rs). A program's
//! package is read over xpute's own (`TARGETS`), which it may take, and
//! whose WIT the generator carries (`FILES`), and over the program's own
//! packages it takes, which the program names.
//!
//! A packet lays its values out as the Component Model's Canonical ABI does
//! (xpute-link cabi.rs): a call's params as one tuple, an answer's value
//! alone. It reads the subset the link's WIT names. Anything else is an
//! error: a resource's constructor or static, an option of an option, more
//! than 255 methods to an object.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use wit_parser::{
    Function, FunctionKind, IndexMap, InterfaceId, Param, Resolve, SourceMap, Type, TypeDefKind,
    TypeId, TypeOwner, UnresolvedPackageGroup, WorldItem, WorldKey,
};

use xpute_gen::spec::{
    Member, Output, Packages, int_of, is_float, mark, package, package_path, pascal, read_str,
    rs_crate, rs_ident, rs_module, screaming, snake, ts_module,
};

mod cpp;
mod rs;
mod ts;

/// One package's bindings on one side, and where they go, each path from the
/// root of the program they are written for.
pub struct WitTarget<'a> {
    /// The package's WIT directory.
    pub wit: &'a str,
    /// The Rust module file, which declares a module per interface in the
    /// directory of its name, or is itself the module of a package's one
    /// interface of its name (`folded`).
    pub rs: Option<&'a str>,
    /// The TypeScript directory, a module per interface, which a host
    /// alone takes.
    pub ts: Option<&'a str>,
    /// The C++ directory, a header per interface, which the guest alone
    /// takes. A program's headers include xpute's library as the
    /// amalgamation's one header, `xpute.hpp`; xpute's own lie in the
    /// library, under its root, which holds `compile_flags.txt`.
    pub cpp: Option<&'a str>,
    /// The side the bindings are for, or none for a package of types alone,
    /// which takes nothing of a side. The Rust is written per side, as two
    /// Rust sides share one process natively; the TypeScript is a host's,
    /// taking the link it sends on as an argument, from `link.ts` beside
    /// xpute:link's bindings.
    pub side: Option<Side>,
}

/// A side of the link. A package's world is its guest's, which implements
/// what the world exports and calls what it imports; every other side, the
/// TypeScript too, implements the imports and calls the exports. A package
/// whose interfaces have functions declares its world.
#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Guest,
    Host,
}

impl Side {
    /// The directory of the side's crate in xpute, whose `src/abi.rs` is the
    /// root module of xpute's bindings on the side: the guest's, whose `link`
    /// opens, calls and sends and whose `wait` waits, which its bindings
    /// reach, and the host's, whose bindings take the side they are given;
    /// the link's own items come from xpute-link.
    fn dir(self) -> &'static str {
        match self {
            Side::Guest => "guest/rust",
            Side::Host => "host/rust",
        }
    }
}

/// The files of xpute's own packages that the generator reads, each `(path,
/// text)`, its path from xpute's root: their WIT and the link's frame.json5,
/// embedded as this crate is built (build.rs), so that a program's bindings
/// are written with no xpute checkout.
const FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/xpute.rs"));

/// The link's frame.json5, among `FILES`.
const FRAME: &str = "link/spec/frame.json5";

/// xpute's packages that bindings reach, each its directory from xpute's
/// root and its name, as a program depends on it.
const PACKAGES: [(&str, &str); 5] = [
    ("link/rust", "xpute-link"),
    ("link/ts", "@xpute/link"),
    ("guest/rust", "xpute-guest"),
    ("host/rust", "xpute-host"),
    ("host/ts", "@xpute/host"),
];

/// The text of xpute's file `path`, among `FILES`.
fn xpute_file(path: &str) -> Result<&'static str, String> {
    FILES
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, text)| *text)
        .ok_or(format!("{path}: no file of xpute's"))
}

/// xpute's package in its directory `dir`, of the `*.wit` files there among
/// `FILES`, unresolved.
fn xpute_wit(dir: &str) -> Result<UnresolvedPackageGroup, String> {
    let mut map = SourceMap::default();
    for (path, text) in FILES {
        if Path::new(path).parent() == Some(Path::new(dir)) && path.ends_with(".wit") {
            map.push_str(path, *text);
        }
    }
    map.parse()
        .map_err(|(map, e)| format!("{dir}: {}", e.render(&map)))
}

/// xpute's package that holds `path`, a path from xpute's root: its name and
/// its directory (`Packages`).
fn xpute_package(path: &str) -> Result<(String, PathBuf), String> {
    PACKAGES
        .iter()
        .find(|(dir, _)| Path::new(path).starts_with(dir))
        .map(|&(dir, name)| (name.into(), dir.into()))
        .ok_or(format!("{path}: in no package of xpute's"))
}

// xpute's packages by name, after the link's, as `TARGETS` orders them.
macro_rules! targets {
    ($($p:ident),*) => {
        &[
            WitTarget {
                wit: "link/wit",
                rs: Some("link/rust/src/abi.rs"),
                ts: Some("link/ts/src/abi"),
                cpp: Some("cpp/xpute/link/abi"),
                side: None,
            },
            $(WitTarget {
                wit: concat!("wit/", stringify!($p)),
                rs: Some(concat!("guest/rust/src/abi/", stringify!($p), ".rs")),
                ts: None,
                cpp: Some(concat!("cpp/xpute/guest/abi/", stringify!($p))),
                side: Some(Side::Guest),
            },)*
            $(WitTarget {
                wit: concat!("wit/", stringify!($p)),
                rs: Some(concat!("host/rust/src/abi/", stringify!($p), ".rs")),
                ts: Some(concat!("host/ts/src/abi/", stringify!($p))),
                cpp: None,
                side: Some(Side::Host),
            },)*
        ]
    };
}

/// The bindings of xpute's packages, each path from xpute's root, in the
/// order they are read: xpute:link's own types first, which every side reads,
/// then xpute's packages, each `xpute:<name>` in `wit/<name>`, each before
/// those that take its types. A program's package is read after all of its
/// side's. On the guest's side the Rust is in the guest's crate and the C++
/// in the C++ guest's `abi/`; on a host's the Rust is in the Rust host,
/// and the TypeScript in the TypeScript host, on no platform's head.
pub const TARGETS: &[WitTarget<'static>] = targets!(clocks, fs, life, log, stop);

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
                ));
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

/// The trait or interface an object's implementation implements, named by the
/// side that implements it (`Guest`, `Host`): a singleton's is the side's
/// name, and a resource's the side's and the resource's, as wit-bindgen's and
/// wasmtime's are.
fn side_trait(o: &Obj, side: &str) -> String {
    if o.resource {
        format!("{side}{}", pascal(&o.name))
    } else {
        side.into()
    }
}

/// The names the Rust bindings give beside WIT names, those the runtime
/// module `_rt` holds, and the prelude's they name bare. A WIT name that
/// meets one takes `_`, as wit-bindgen's `Guest_` does.
const RS_GIVEN: [&str; 32] = [
    "HANDLE",
    "IMPORTS",
    "CALLS",
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
    "Callee",
    "ComponentError",
    "Handle",
    "Lift",
    "LinkObject",
    "Lower",
    "Method",
    "Ref",
    "Sender",
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
/// (imports.ts) and of methods answered at once (calls.ts). An interface's
/// module that meets one takes `_`, as a name in `TS_GIVEN` does.
const TS_GIVEN_MODULES: [&str; 2] = ["imports", "calls"];

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

/// Whether `n` is a resource's side trait or interface's form, a side's
/// name and then a word (`GuestDevice`, `HostDevice`), which any module may
/// name.
fn is_side_trait(n: &str) -> bool {
    ["Guest", "Host"].iter().any(|s| {
        n.strip_prefix(s)
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_uppercase()))
    })
}

/// Whether the Rust bindings give `n`: one of `RS_GIVEN`, a lent buffer's
/// generic (`T0`, …), or a resource's side trait's form.
fn rs_given(n: &str) -> bool {
    RS_GIVEN.contains(&n)
        || n.strip_prefix('T')
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        || is_side_trait(n)
}

/// Whether the TypeScript bindings give `n`: one of `TS_GIVEN`, or a
/// resource's side interface's form.
fn ts_given(n: &str) -> bool {
    TS_GIVEN.contains(&n) || is_side_trait(n)
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
                return Err(format!("{at}: the link carries no option of an option"));
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
    Implement,
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

/// The root module of xpute's bindings on `side`, its crate's `src/abi.rs`
/// in xpute's tree at `root`: a module to each of `TARGETS` on the side whose
/// Rust is written in the directory of its name.
pub fn abi_root(root: &Path, side: Side) -> Result<Output, String> {
    let path = format!("{}/src/abi.rs", side.dir());
    let dir = Path::new(path.trim_end_matches(".rs"));
    let mods: String = TARGETS
        .iter()
        .filter(|t| t.side == Some(side))
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
        package_path(&|p| package(root, p), &path)?,
        mark("wit", TOOL)
    );
    Ok(Output::new(&path, text))
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

/// The value of the link's frame.json5's `name`: `NAMED`, the bit of a named
/// handle, or `DROP`, the drop's method, the bound of an object's methods,
/// which none takes.
fn frame_val(name: &str) -> Result<u32, String> {
    let spec = read_str(FRAME, xpute_file(FRAME)?)?;
    spec.get(name)
        .and_then(|n| n.val().num())
        .map(|n| n as u32)
        .ok_or(format!("{FRAME}: no {name}"))
}

/// The bindings of `t`'s package, its paths from `root`. Its peers are the
/// packages of `TARGETS` on its side that are read before it, from `FILES`,
/// then those of `own` on its side, the program's own packages it takes,
/// from `root`, each named by the manifest above its files: their WIT is
/// read before its own, and each package its types take is reached where
/// its peer writes it. A package that is one of `TARGETS` is xpute's own,
/// and `root` is then xpute's tree, where its peers lie.
pub fn wit_outputs(root: &Path, t: &WitTarget, own: &[WitTarget]) -> Result<Vec<Output>, String> {
    let main = UnresolvedPackageGroup::parse_dir(root.join(t.wit))
        .map_err(|e| format!("{}: {e:?}", t.wit))?;
    let on_side = |p: &&WitTarget| p.side.is_none() || p.side == t.side;
    let mut r = Resolve::default();
    let mut ids = Vec::new();
    let mut peers = Vec::new();
    let mut inside = None;
    for p in TARGETS.iter().filter(on_side) {
        let group = xpute_wit(p.wit)?;
        if group.main.name == main.main.name {
            inside = Some(root);
            break;
        }
        ids.push(
            r.push_group(group)
                .map_err(|e| format!("{}: {e:?}", p.wit))?,
        );
        peers.push(p);
    }
    let mut own_ids = Vec::new();
    for p in own.iter().filter(on_side) {
        let group = UnresolvedPackageGroup::parse_dir(root.join(p.wit))
            .map_err(|e| format!("{}: {e:?}", p.wit))?;
        own_ids.push((
            r.push_group(group)
                .map_err(|e| format!("{}: {e:?}", p.wit))?,
            p,
        ));
    }
    let pkg = r
        .push_group(main)
        .map_err(|e| format!("{}: {e:?}", t.wit))?;
    let p = &r.packages[pkg];
    let pname = format!("{}:{}", p.name.namespace, p.name.name);
    // xpute:link, which every side reads first.
    let link = &TARGETS[0];
    let named_bit = frame_val("NAMED")?;
    let drop = frame_val("DROP")?;
    let crate_dir =
        t.rs.map(|rs| package(root, rs).map(|(_, dir)| root.join(dir)))
            .transpose()?;
    let ts_dir = t.ts.map(|d| root.join(d));
    // xpute's own packages' C++ is inside its library, which a program's is
    // over.
    let cpp_library = match (t.cpp, inside) {
        (Some(_), Some(x)) => cpp::Library::Inside(cpp::externs(&r, &ids, x, &peers)?),
        _ => cpp::Library::Over,
    };
    // Each peer with the root its paths are from where the package lies in
    // that tree, and how a path there maps to its package: xpute's by
    // `PACKAGES`, the program's by its manifests.
    let tree = |path: &str| package(root, path);
    let xpute: Packages = &xpute_package;
    let reached = ids
        .iter()
        .zip(&peers)
        .map(|(&id, &p)| (id, p, inside, xpute))
        .chain(
            own_ids
                .iter()
                .map(|&(id, p)| (id, p, Some(root), &tree as Packages)),
        );
    let mut externs = Vec::new();
    for (id, p, from, pkg) in reached {
        let name = &r.packages[id].name;
        let rs = match (&crate_dir, p.rs) {
            (Some(here), Some(rs)) => rs_module(here, from, pkg, rs)?,
            _ => String::new(),
        };
        let ts = match (&ts_dir, p.ts) {
            (Some(here), Some(ts)) => ts_module(here, from, pkg, ts)?,
            _ => String::new(),
        };
        externs.push((format!("{}:{}", name.namespace, name.name), rs, ts));
    }
    // The guest's crate, as the Rust names it, whose `link` its bindings
    // reach, or none for a host's, which take the side they are given; and
    // whether it is the guest.
    let side = match (&crate_dir, t.side) {
        (Some(here), Some(Side::Guest)) => Some((
            Some(rs_crate(
                here,
                inside,
                &xpute_package,
                &format!("{}/Cargo.toml", Side::Guest.dir()),
            )?),
            true,
        )),
        (Some(_), Some(Side::Host)) => Some((None, false)),
        _ => None,
    };
    // The link's TypeScript module `file`, beside xpute:link's bindings.
    let link_module = |file: &str| match (&ts_dir, link.ts) {
        (Some(here), Some(ts)) => {
            let at = Path::new(ts).parent().unwrap_or(Path::new("")).join(file);
            ts_module(here, inside, &xpute_package, &at.to_string_lossy())
        }
        _ => Ok(String::new()),
    };
    let link_ts = link_module("link.ts")?;
    let cabi_ts = link_module("cabi.ts")?;
    // The link's crate, xpute:link's bindings' own, as the Rust names it.
    let link_rs = match (&crate_dir, link.rs) {
        (Some(here), Some(rs)) => rs_crate(here, inside, &xpute_package, rs)?,
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
    // the guest implements it.
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
    // The methods of the interfaces its world imports that answer at once, a
    // function without WIT's `async`, xpute's and a plugin's as well as the
    // package's own, each its interface's named handle and its method's
    // number, as IMPORTS holds the handles.
    let mut calls: Vec<(u32, usize)> = Vec::new();
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
        // Whether the guest implements the interface, as its world exports it,
        // or calls it, as its world imports it; none for one of types alone.
        let guest_implements = match world {
            _ if objs.is_empty() => None,
            Some(w) if has(&w.exports, i) => Some(true),
            Some(w) if has(&w.imports, i) => Some(false),
            Some(_) => return Err(format!("{at}: in no world of its package")),
            None => return Err(format!("{at}: its package has no world")),
        };
        let role = |implements: bool| {
            if implements {
                Role::Implement
            } else {
                Role::Call
            }
        };
        let m = rs_name(iname);
        if let Some(dir) = rs_dir {
            let rs_role = match (guest_implements, &side) {
                (None, _) => None,
                (Some(g), Some((side, guest))) => Some((
                    role(g == *guest),
                    side.as_deref(),
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
                    package_path(&|p| package(root, p), &path)?,
                    mark(src, TOOL),
                    g.rs_interface(&objs, rs_role)?
                );
                outs.push(Output::new(&path, text));
                let _ = writeln!(rs, "pub mod {m};");
            }
        }
        if let Some(dir) = t.ts {
            let ts_role = match (guest_implements, t.side) {
                (None, _) => None,
                (Some(g), Some(Side::Host)) => Some(role(!g)),
                (Some(_), _) => {
                    return Err(format!("{at}: TypeScript is written for a host alone"));
                }
            };
            let (body, enums) = g.ts_interface(&objs, ts_role)?;
            let m = g.module_ts();
            if !enums.is_empty() {
                let path = format!("{dir}/{m}.enum.ts");
                let text = format!(
                    "// {}\n//\n// {}\n{enums}",
                    package_path(&|p| package(root, p), &path)?,
                    mark(src, TOOL)
                );
                outs.push(Output::new(&path, text));
            }
            let path = format!("{dir}/{m}.ts");
            let text = format!(
                "// {}\n//\n// {}\n\n{}\n{body}",
                package_path(&|p| package(root, p), &path)?,
                mark(src, TOOL),
                g.ts_imports()
            );
            outs.push(Output::new(&path, text));
        }
        if let Some(dir) = t.cpp {
            let cpp_role = match (guest_implements, t.side) {
                (None, _) => None,
                (Some(g), Some(Side::Guest)) => Some(role(g)),
                (Some(_), _) => return Err(format!("{at}: C++ is written for the guest alone")),
            };
            let path = if cpp::is_folded(&r, i) {
                format!("{dir}.hpp")
            } else {
                format!("{dir}/{m}.hpp")
            };
            outs.push(Output::new(
                &path,
                cpp::hdr(&g, &objs, cpp_role, &path, src, &cpp_library)?,
            ));
        }
    }
    // The interfaces the world imports, in the world's order, each its name
    // and its named handle; one of types alone has none.
    let mut imports: Vec<(String, u32)> = Vec::new();
    for item in world.into_iter().flat_map(|w| w.imports.values()) {
        let WorldItem::Interface { id, .. } = *item else {
            continue;
        };
        let Some(n) = &r.interfaces[id].name else {
            continue;
        };
        let name = format!("{}/{n}", package_name(&r, id));
        let objs = objs(&r, id, &name, drop)?;
        if objs.is_empty() {
            continue;
        }
        let handle = named(&name, named_bit);
        for o in &objs {
            for (k, f) in o.methods.iter().enumerate() {
                if !is_async(f) {
                    calls.push((handle, k));
                }
            }
        }
        imports.push((name, handle));
    }
    if let (Some((_, false)), Some(_)) = (&side, world) {
        let pairs: Vec<_> = imports
            .iter()
            .map(|(n, h)| format!("(\"{n}\", {h:#x})"))
            .collect();
        let _ = writeln!(
            rs,
            "\n/// The interfaces the world imports, each `(name, handle)`, which are\n/// mounted before the guest boots: one with functions of its own under its\n/// handle, one of resources alone by an opener at a prefix.\npub const IMPORTS: &[(&str, {link_rs}::Handle)] = &[{}];",
            pairs.join(", ")
        );
        let pairs: Vec<_> = calls
            .iter()
            .map(|(h, k)| format!("({h:#x}, {k})"))
            .collect();
        let _ = writeln!(
            rs,
            "\n/// The methods of the interfaces the world imports that answer at once,\n/// each `(handle, method)`, a function without WIT's `async`, which a\n/// runtime refuses to carry to a UI on another thread.\npub const CALLS: &[({link_rs}::Handle, {link_rs}::Method)] = &[{}];",
            pairs.join(", ")
        );
    }
    if let (Some(dir), Some(_)) = (t.ts, world) {
        let pairs: Vec<_> = imports
            .iter()
            .map(|(n, h)| format!("[\"{n}\", {h:#x}]"))
            .collect();
        let path = format!("{dir}/imports.ts");
        let text = format!(
            "// {}\n//\n// {}\n\nimport type {{ Handle }} from \"{link_ts}\";\n\n/** The interfaces the world imports, each `[name, handle]`, which are\n * mounted before the guest boots: one with functions of its own under its\n * handle, one of resources alone by an opener at a prefix. */\nexport const IMPORTS: readonly (readonly [string, Handle])[] = [{}];\n",
            package_path(&|p| package(root, p), &path)?,
            mark(src, TOOL),
            pairs.join(", ")
        );
        outs.push(Output::new(&path, text));
        let pairs: Vec<_> = calls
            .iter()
            .map(|(h, k)| format!("[{h:#x}, {k}]"))
            .collect();
        let path = format!("{dir}/calls.ts");
        let text = format!(
            "// {}\n//\n// {}\n\nimport type {{ Handle, Method }} from \"{link_ts}\";\n\n/** The methods of the interfaces the world imports that answer at once,\n * each `[handle, method]`, a function without WIT's `async`, which a\n * runtime refuses to carry to a UI on another thread. */\nexport const CALLS: readonly (readonly [Handle, Method])[] = [{}];\n",
            package_path(&|p| package(root, p), &path)?,
            mark(src, TOOL),
            pairs.join(", ")
        );
        outs.push(Output::new(&path, text));
    }
    if let Some(path) = t.rs {
        let text = format!(
            "// {}\n//\n// {}\n\n{rs}",
            package_path(&|p| package(root, p), path)?,
            mark(src, TOOL)
        );
        outs.push(Output::new(path, text));
    }
    Ok(outs)
}
