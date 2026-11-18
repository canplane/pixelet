// xpute-link-gen/wit/cpp.rs

//! The link's bindings in C++, a header per interface, in the shape the Rust
//! takes (rs.rs) and in C++'s own forms: the interface's handle and its
//! methods' numbers, its types, each storing itself into a packet and loading
//! itself from one (cpp/xpute/link/cabi.hpp) at the offsets the Rust and the
//! TypeScript take, from the one layout of each type (`Gen::layout`), and the
//! guest's side of its objects, the only side C++ is written for: a function
//! per method it calls, and a class per object it implements, with the object
//! over it and what mounts or opens it. A record is a struct, an enum
//! xpute-gen's (`cpp_enum`), a variant a struct over a `std::variant` of one
//! struct per case, an option a `std::optional`, a result a `std::variant` of
//! its ok and its error, a string a `std::string` and a list a `std::vector`.
//! A call answers a `Result`, and an async one a `Task` of it (xpute/link
//! wait.hpp), sent as it is made, as the Rust's future is. A name is its WIT
//! name in C++'s casing: a type's and a case's PascalCase, a namespace's, a
//! field's and an enum member's snake_case, a constant's SCREAMING_CASE. An
//! interface's namespace is its package's namespace, `abi`, its package's
//! name and its own, which a package's one interface of its name folds into
//! its package's (`folded`), as its Rust module does: the words wit-bindgen's
//! C prefixes a name with, under `abi` as the bindings' Rust modules are. What a header takes from the
//! library it reaches through one namespace, `_rt`, of using-declarations, as
//! the Rust's `_rt` module re-exports. A program's header includes the library
//! as the amalgamation, the two files a program takes, by its one header; the
//! headers of xpute's own packages lie inside the library and include its
//! headers by their paths, which the amalgamation takes in.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use wit_parser::{Function, InterfaceId, PackageId, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use xpute_gen::spec::{Includes, Member, cpp_enum, cpp_ident, cpp_member, mark, pascal, snake};

use super::{
    Gen, Kind, Obj, Role, TOOL, WitTarget, cases, discriminant, escaped, folded, is_async, is_lent,
    is_side_trait, kind, members, method_const, method_name, object_method, package_name, params,
    plus, rec_layout, rs_name, side_trait, type_name, variant_layout,
};

/// Where a name the bindings take from the library is declared: its
/// namespace, and the header that declares it, from the library's root.
type Declaration = (&'static str, &'static str);

/// The errno and a result (kit status/error.hpp).
const ERROR: Declaration = ("::xpute", "xpute/kit/status/error.hpp");
/// The link's packet writer and reader.
const CABI: Declaration = ("::xpute::link", "xpute/link/cabi.hpp");
/// The link's own items: a handle, a method and an object.
const LINK: Declaration = ("::xpute::link", "xpute/link/link.hpp");
/// The coroutine an async call answers.
const WAIT: Declaration = ("::xpute::link", "xpute/link/wait.hpp");
/// The guest's end of the link, which calls, mounts and answers.
const GUEST_LINK: Declaration = ("::xpute::guest", "xpute/guest/link.hpp");
/// The guest's send of an async call.
const GUEST_WAIT: Declaration = ("::xpute::guest", "xpute/guest/wait.hpp");

/// The library's one header that a program's bindings include, the
/// amalgamation's (cpp/amalgamation.awk), which holds every header above and
/// the bindings of xpute's packages.
const AMALGAMATION: &str = "xpute.hpp";

/// How a header reaches the library.
pub(super) enum Library {
    /// From inside it, as the bindings of xpute's own packages are, which the
    /// amalgamation takes in: each header by its path from the library's
    /// root, and each package's headers where `externs` puts them.
    Inside(Vec<(String, String)>),
    /// From over it, as a program's bindings are: by the amalgamation alone.
    Over,
}

/// The names the C++ bindings give in a scope a WIT name shares: the
/// interface's handle, a record's store and load, the class an object's
/// implementation derives from and what mounts it, an async method's answer,
/// and an async call's lent buffer's template parameter. A WIT name that
/// meets one takes `_`.
const GIVEN: [&str; 7] = [
    "HANDLE",
    "store",
    "load",
    "mount",
    "task_return",
    "Guest",
    "L",
];

/// Whether the C++ bindings give `n`: one of `GIVEN`, a lent buffer's template
/// parameter (`T0`, …), or a resource's side class's form.
fn given(n: &str) -> bool {
    GIVEN.contains(&n)
        || n.strip_prefix('T')
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        || is_side_trait(n)
}

/// The class over an object's implementation, which its trailing `_`
/// keeps from every WIT name.
fn object_class(o: &Obj) -> String {
    if o.resource {
        format!("{}Object_", pascal(&o.name))
    } else {
        "Object_".into()
    }
}

/// Whether a value of `k` is passed by value: a scalar's.
fn is_scalar(k: &Kind) -> bool {
    matches!(
        k,
        Kind::Bool | Kind::Number(_) | Kind::Handle | Kind::Enum(_)
    )
}

/// A WIT name as a C++ identifier in snake_case.
fn name(n: &str) -> String {
    escaped(cpp_ident(&snake(n)), given)
}

/// Whether the interface `i` is its package's namespace itself (`folded`).
pub(super) fn is_folded(r: &Resolve, i: InterfaceId) -> bool {
    let pkg = package_name(r, i);
    let (_, p) = pkg.split_once(':').unwrap_or_default();
    folded(r, i, &rs_name(p))
}

/// The namespace of the interface `i` (the module's doc), its package's own
/// where it is folded.
fn namespace(r: &Resolve, i: InterfaceId) -> String {
    let pkg = package_name(r, i);
    let (ns, p) = pkg.split_once(':').unwrap_or_default();
    let iname = r.interfaces[i].name.as_deref().unwrap_or_default();
    let mut words = vec![name(ns), "abi".into(), name(p)];
    if !is_folded(r, i) {
        words.push(name(iname));
    }
    words.join("::")
}

/// Where each package of `peers`, its paths from xpute's root `xpute`,
/// written in C++ has its headers, by its `namespace:package`: its directory
/// as an include names it, from its library's root, the nearest directory
/// holding `compile_flags.txt` (`-I.`).
pub(super) fn externs(
    r: &Resolve,
    ids: &[PackageId],
    xpute: &Path,
    peers: &[&WitTarget],
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (&id, p) in ids.iter().zip(peers) {
        let Some(dir) = p.cpp else { continue };
        let library = Path::new(dir)
            .ancestors()
            .find(|d| xpute.join(d).join("compile_flags.txt").is_file())
            .ok_or(format!("{dir}: under no C++ library"))?;
        let n = &r.packages[id].name;
        let under = Path::new(dir)
            .strip_prefix(library)
            .map_err(|e| format!("{dir}: {e}"))?;
        out.push((
            format!("{}:{}", n.namespace, n.name),
            under.to_string_lossy().into_owned(),
        ));
    }
    Ok(out)
}

/// One interface's header being written.
struct Header<'a> {
    g: &'a Gen<'a>,
    library_at: &'a Library,
    /// The standard's headers it includes, and the library's.
    std: RefCell<Includes>,
    library: RefCell<BTreeSet<String>>,
    /// What it takes from the library, by namespace, which `_rt` declares.
    uses: RefCell<BTreeMap<String, BTreeSet<String>>>,
}

impl Header<'_> {
    fn include(&self, hdr: &'static str) {
        self.std.borrow_mut().insert(hdr);
    }

    /// `name`, declared in the namespace `ns` by the header `hdr`, reached
    /// through `_rt`.
    fn rt_use(&self, (ns, hdr): Declaration, name: &str) -> String {
        let hdr = match self.library_at {
            Library::Inside(_) => hdr,
            Library::Over => AMALGAMATION,
        };
        self.library.borrow_mut().insert(hdr.into());
        self.uses
            .borrow_mut()
            .entry(ns.into())
            .or_default()
            .insert(name.into());
        format!("_rt::{name}")
    }

    /// An errno's member, as a method's result.
    fn errno(&self, e: &str) -> String {
        self.include("cstdint");
        format!(
            "static_cast<std::int32_t>({}::{e})",
            self.rt_use(ERROR, "Errno")
        )
    }

    /// The result of the errno `e`, an expression of `Errno`.
    fn refusal(&self, e: &str) -> String {
        self.include("cstdint");
        format!("static_cast<std::int32_t>({e})")
    }

    /// A discriminant's C++ type, of `n` cases.
    fn discriminant_type(&self, n: usize) -> Result<&'static str, String> {
        self.include("cstdint");
        Ok(discriminant(n)?.cpp())
    }

    /// A WIT type's C++ type, by its whole path, which no case's struct
    /// hides, its header included where it is another interface's.
    fn path(&self, t: TypeId) -> Result<String, String> {
        let r = self.g.r;
        let TypeOwner::Interface(i) = r.types[t].owner else {
            return Err(format!("{}: a type of no interface", self.g.at()));
        };
        if i != self.g.here {
            let m = rs_name(r.interfaces[i].name.as_deref().unwrap_or_default());
            let pkg = package_name(r, i);
            let hdr = match self.library_at {
                _ if pkg == self.g.pkg => format!("{m}.hpp"),
                Library::Inside(externs) => {
                    let (_, dir) = externs
                        .iter()
                        .find(|(p, _)| *p == pkg)
                        .ok_or(format!("{}: {pkg} is written in no C++", self.g.at()))?;
                    if is_folded(r, i) {
                        format!("{dir}.hpp")
                    } else {
                        format!("{dir}/{m}.hpp")
                    }
                }
                Library::Over => AMALGAMATION.into(),
            };
            self.library.borrow_mut().insert(hdr);
        }
        Ok(format!("::{}::{}", namespace(r, i), type_name(r, t, given)))
    }

    fn ty(&self, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool => "bool".into(),
            Kind::Number(n) => {
                self.include("cstdint");
                n.cpp().into()
            }
            Kind::Handle => {
                self.include("cstdint");
                "std::uint32_t".into()
            }
            Kind::Str => {
                self.include("string");
                "std::string".into()
            }
            Kind::Array(n) => {
                self.include("cstdint");
                self.include("vector");
                format!("std::vector<{}>", n.cpp())
            }
            Kind::List(e) => {
                self.include("vector");
                format!("std::vector<{}>", self.ty(e)?)
            }
            Kind::Option(e) => {
                self.include("optional");
                format!("std::optional<{}>", self.ty(e)?)
            }
            Kind::Result(ok, err) => {
                self.include("variant");
                let case = |k: &Option<Box<Kind>>| {
                    k.as_ref()
                        .map_or(Ok("std::monostate".into()), |k| self.ty(k))
                };
                format!("std::variant<{}, {}>", case(ok)?, case(err)?)
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => self.path(*t)?,
        })
    }

    /// The size and alignment of `k` as C++ writes them: a record's or a
    /// variant's own constants, any other's numbers.
    fn layout(&self, k: &Kind) -> Result<(String, String), String> {
        Ok(match k {
            Kind::Record(_) | Kind::Variant(_) => {
                let t = self.ty(k)?;
                (format!("{t}::SZ"), format!("{t}::ALIGN"))
            }
            _ => {
                let (sz, align) = self.g.layout(k)?;
                (sz.to_string(), align.to_string())
            }
        })
    }

    /// Stores `e`, a value of `k`, at `at` with the writer `w`.
    fn store(&self, w: &str, at: &str, e: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool | Kind::Number(_) | Kind::Handle => {
                format!("{w}.put<{}>({at}, {e});", self.ty(k)?)
            }
            Kind::Enum(t) => {
                let u = self.discriminant_type(self.g.case_cnt(*t))?;
                format!("{w}.put<{u}>({at}, static_cast<{u}>({e}));")
            }
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(n) => format!("{w}.arr<{}>({at}, {e});", n.cpp()),
            Kind::List(el) => {
                let (sz, align) = self.layout(el)?;
                // A list inside a list takes names of its own depth.
                let d = self.g.depth.get();
                self.g.depth.set(d + 1);
                let inner = self.store(
                    w,
                    &format!("*b{d} + i{d} * {sz}"),
                    &format!("{e}[i{d}]"),
                    el,
                );
                self.g.depth.set(d);
                format!(
                    "if (auto b{d} = {w}.list({at}, {e}.size(), {sz}, {align})) {{ for (std::size_t i{d} = 0; i{d} < {e}.size(); i{d}++) {{ {} }} }}",
                    inner?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => format!("{e}.store({w}, {at});"),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.g.layout(el)?)])?;
                self.include("cstdint");
                format!(
                    "if ({e}) {{ {w}.put<std::uint8_t>({at}, 1); {} }}",
                    self.store(w, &plus(at, off), &format!("(*{e})"), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.g.result_layout(ok, err)?;
                self.include("cstdint");
                let case = |k: &Option<Box<Kind>>, c: usize| -> Result<String, String> {
                    match k {
                        Some(k) => Ok(format!(
                            " {}",
                            self.store(w, &plus(at, off), &format!("std::get<{c}>({e})"), k)?
                        )),
                        None => Ok(String::new()),
                    }
                };
                format!(
                    "if ({e}.index() == 0) {{{} }} else {{ {w}.put<std::uint8_t>({at}, 1);{} }}",
                    case(ok, 0)?,
                    case(err, 1)?
                )
            }
        })
    }

    /// Loads a value of `k` at `at` with the reader `r`.
    fn load(&self, r: &str, at: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool | Kind::Number(_) | Kind::Handle => {
                format!("{r}.get<{}>({at})", self.ty(k)?)
            }
            Kind::Enum(t) => format!(
                "static_cast<{}>({r}.case_({at}, {}))",
                self.ty(k)?,
                self.g.case_cnt(*t)
            ),
            Kind::Str => format!("std::string({r}.str({at}))"),
            Kind::Array(n) => format!("{r}.arr<{}>({at})", n.cpp()),
            Kind::List(el) => {
                let (sz, align) = self.layout(el)?;
                // A list inside a list takes names of its own depth.
                let d = self.g.depth.get();
                self.g.depth.set(d + 1);
                let inner = self.load(r, &format!("b{d} + i{d} * {sz}"), el);
                self.g.depth.set(d);
                format!(
                    "[&] {{ auto [b{d}, n{d}] = {r}.list({at}, {sz}, {align}); {} v{d}; v{d}.reserve(n{d}); for (std::size_t i{d} = 0; i{d} < n{d}; i{d}++) v{d}.push_back({}); return v{d}; }}()",
                    self.ty(k)?,
                    inner?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => format!("{}::load({r}, {at})", self.ty(k)?),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.g.layout(el)?)])?;
                let ty = self.ty(k)?;
                format!(
                    "({r}.case_({at}, 2) == 0 ? {ty}() : {ty}({}))",
                    self.load(r, &plus(at, off), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.g.result_layout(ok, err)?;
                let ty = self.ty(k)?;
                self.include("utility");
                let case = |k: &Option<Box<Kind>>, c: usize| -> Result<String, String> {
                    let v = match k {
                        Some(k) => self.load(r, &plus(at, off), k)?,
                        None => "std::monostate()".into(),
                    };
                    Ok(format!("{ty}(std::in_place_index<{c}>, {v})"))
                };
                format!(
                    "({r}.case_({at}, 2) == 0 ? {} : {})",
                    case(ok, 0)?,
                    case(err, 1)?
                )
            }
        })
    }

    /// The Canonical ABI's members of the type `n`, beside its fields: its
    /// layout, its store, whose statements are `store`, and its load's head.
    fn cabi(&self, n: &str, sz: usize, align: usize, store: &str) -> String {
        self.include("cstddef");
        let (lower, lift) = (self.rt_use(CABI, "Lower"), self.rt_use(CABI, "Lift"));
        format!(
            "    static constexpr std::size_t SZ = {sz};\n    static constexpr std::size_t ALIGN = {align};\n\n    void store({lower} &w, std::size_t at) const noexcept {{\n{store}    }}\n\n    static {n} load({lift} &r, std::size_t at) noexcept {{\n"
        )
    }

    /// The interface's handle and its methods' numbers, its types, and the
    /// guest's side of its objects, by `role`; none for an interface of types
    /// alone.
    fn interface(&self, objs: &[Obj], role: Option<Role>) -> Result<String, String> {
        let g = self.g;
        let mut out = String::new();
        if !objs.is_empty() {
            let _ = writeln!(
                out,
                "// The interface's handle, named by `{}` (frame.json5 `NAMED`).\ninline constexpr {} HANDLE = {:#x};\n",
                g.at(),
                self.rt_use(LINK, "Handle"),
                g.handle()
            );
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "// `{}`\ninline constexpr {} {} = {k};",
                    f.name,
                    self.rt_use(LINK, "Method"),
                    method_const(o, f, given)
                );
            }
        }
        if !objs.is_empty() {
            out.push('\n');
        }
        let at = g.at();
        for &t in g.r.interfaces[g.here].types.values() {
            let td = &g.r.types[t];
            if !matches!(td.owner, TypeOwner::Interface(i) if i == g.here) {
                continue;
            }
            let n = type_name(g.r, t, given);
            match &td.kind {
                TypeDefKind::Enum(e) => {
                    let ms: Vec<Member> = members(e)
                        .into_iter()
                        .map(|m| Member {
                            ident: cpp_member(&m.name),
                            ..m
                        })
                        .collect();
                    // Held as the Rust's is, whatever width its discriminant
                    // crosses at.
                    let mut lines = Vec::new();
                    cpp_enum(
                        &n,
                        &ms,
                        "std::uint32_t",
                        &mut lines,
                        &mut self.std.borrow_mut(),
                    );
                    let _ = writeln!(out, "{}\n", lines.join("\n"));
                }
                TypeDefKind::Record(_) => {
                    let fields = g.fields(t)?;
                    let (offs, sz, align) = rec_layout(&g.field_layouts(t)?);
                    let _ = writeln!(out, "struct {n} {{");
                    for (f, k) in &fields {
                        let _ = writeln!(out, "    {} {};", self.ty(k)?, name(f));
                    }
                    let mut store = String::new();
                    for ((f, k), &off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            store,
                            "        {}",
                            self.store("w", &plus("at", off), &format!("this->{}", name(f)), k)?
                        );
                    }
                    let _ = writeln!(
                        out,
                        "\n{}        return {n}{{",
                        self.cabi(&n, sz, align, &store)
                    );
                    for ((f, k), &off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            out,
                            "            .{} = {},",
                            name(f),
                            self.load("r", &plus("at", off), k)?
                        );
                    }
                    out.push_str("        };\n    }\n};\n\n");
                }
                TypeDefKind::Variant(v) => {
                    self.include("variant");
                    let cases = cases(g.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&g.payload_layouts(t)?)?;
                    let u = self.discriminant_type(cases.len())?;
                    // A case named as its variant would name the variant's
                    // own struct.
                    let case = |c: &str| {
                        let p = pascal(c);
                        if p == n { p + "_" } else { p }
                    };
                    let _ = writeln!(out, "struct {n} {{");
                    for (c, k) in &cases {
                        match k {
                            Some(k) => {
                                let _ = writeln!(
                                    out,
                                    "    struct {} {{\n        {} val;\n    }};",
                                    case(c),
                                    self.ty(k)?
                                );
                            }
                            None => {
                                let _ = writeln!(out, "    struct {} {{}};", case(c));
                            }
                        }
                    }
                    let alternatives: Vec<String> = cases.iter().map(|(c, _)| case(c)).collect();
                    let mut store = String::from("        switch (this->variant.index()) {\n");
                    for (k, (_, kd)) in cases.iter().enumerate() {
                        let payload = match kd {
                            Some(kd) => format!(
                                " {}",
                                self.store(
                                    "w",
                                    &plus("at", off),
                                    &format!("std::get<{k}>(this->variant).val"),
                                    kd
                                )?
                            ),
                            None => String::new(),
                        };
                        let _ = writeln!(
                            store,
                            "        case {k}: w.put<{u}>(at, {k});{payload} break;"
                        );
                    }
                    store.push_str("        }\n");
                    let _ = writeln!(
                        out,
                        "\n    std::variant<{}> variant;\n\n{}        switch (r.case_(at, {})) {{",
                        alternatives.join(", "),
                        self.cabi(&n, sz, align, &store),
                        cases.len()
                    );
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        let v = match kd {
                            Some(kd) => self.load("r", &plus("at", off), kd)?,
                            None => String::new(),
                        };
                        let _ =
                            writeln!(out, "        case {k}: return {n}{{{}{{{v}}}}};", case(c));
                    }
                    // `case_` answers one of the cases, 0 where it refuses.
                    out.push_str("        }\n        return {};\n    }\n};\n\n");
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(out, "using {n} = {};\n", self.ty(&kind(g.r, ty, &at)?)?);
                }
                _ => {
                    kind(g.r, &Type::Id(t), &at)?;
                }
            }
        }
        if let Some(role) = role {
            for o in objs {
                match role {
                    Role::Implement => self.export(o, &mut out)?,
                    Role::Call => self.import(o, &mut out)?,
                }
            }
        }
        Ok(out)
    }

    /// A method's args: each its name and its kind.
    fn kinds(&self, f: &Function) -> Result<Vec<(String, Kind)>, String> {
        let at = format!("{}.{}", self.g.at(), f.name);
        params(f)
            .map(|p| Ok((name(&p.name), kind(self.g.r, &p.ty, &at)?)))
            .collect()
    }

    /// The args `ps` as a packet: none, or their tuple's size and what writes
    /// it, each of `lents` lending memory, written as its lent record
    /// (`{lent}_lent_`), a name no WIT name escapes to.
    fn args(&self, ps: &[(String, Kind)], lents: &[String]) -> Result<String, String> {
        if ps.is_empty() {
            self.include("optional");
            return Ok("std::nullopt".into());
        }
        let (offs, sz) = self
            .g
            .tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
        let mut writes = Vec::new();
        for ((n, k), at) in ps.iter().zip(&offs) {
            writes.push(if lents.contains(n) {
                format!("{n}_lent_.store(w_, {at});")
            } else {
                self.store("w_", &at.to_string(), n, k)?
            });
        }
        Ok(format!(
            "{}{{{sz}, [&]({} &w_) {{ {} }}}}",
            self.rt_use(LINK, "Packet"),
            self.rt_use(CABI, "Lower"),
            writes.join(" ")
        ))
    }

    /// The answer of a method whose value is of `k`, from its result
    /// `result_` and its return data `data_`: a `Result`.
    fn answer(&self, k: &Option<Kind>) -> Result<String, String> {
        let answer = self.rt_use(LINK, "answer");
        Ok(match k {
            None => format!("{answer}(result_)"),
            Some(k) => format!(
                "{answer}(result_, data_, [&]({} &r_) {{ return {}; }})",
                self.rt_use(CABI, "Lift"),
                self.load("r_", "0", k)?
            ),
        })
    }

    /// The C++ that writes the value `x_` holds, a method's `Result` of `k`
    /// that holds one, as its answer at once, in the frame its args came in
    /// (`ret_`): none for no value.
    fn put(&self, k: &Option<Kind>) -> Result<String, String> {
        Ok(match k {
            None => String::new(),
            Some(k) => format!(
                "const auto &v_ = *x_;\n            ret_.start({});\n            {}\n            ",
                self.layout(k)?.0,
                self.store("ret_", "0", "v_", k)?
            ),
        })
    }

    /// The C++ that answers `x_`, an async method's `Result` of `k`, later:
    /// its refusal, or its value written as the answer's frame is laid.
    fn task_return(&self, k: &Option<Kind>) -> Result<String, String> {
        let task_return = self.rt_use(GUEST_LINK, "task_return");
        let refusal = self.refusal("x_.error().errnum");
        Ok(match k {
            None => format!("{task_return}(tag_, x_ ? 0 : {refusal}, std::nullopt);"),
            Some(k) => format!(
                "if (!x_) return {task_return}(tag_, {refusal}, std::nullopt);\n                const auto &v_ = *x_;\n                {task_return}(tag_, 0, {}{{{}, [&]({} &w_) {{ {} }}}});",
                self.rt_use(LINK, "Packet"),
                self.layout(k)?.0,
                self.rt_use(CABI, "Lower"),
                self.store("w_", "0", "v_", k)?
            ),
        })
    }

    /// The C++ type a call takes a value of `k` as, as the Rust borrows it: a
    /// scalar by value, a string as a view, an optional string as an optional
    /// view and a list as a span of its elements, any other by const
    /// reference. A string a call reads is never copied to be passed.
    fn borrowed(&self, k: &Kind) -> Result<String, String> {
        Ok(match k {
            _ if is_scalar(k) => self.ty(k)?,
            Kind::Str => {
                self.include("string_view");
                "std::string_view".into()
            }
            Kind::Option(e) if matches!(**e, Kind::Str) => {
                self.include("optional");
                self.include("string_view");
                "std::optional<std::string_view>".into()
            }
            Kind::Array(n) => {
                self.include("cstdint");
                self.include("span");
                format!("std::span<const {}>", n.cpp())
            }
            Kind::List(e) => {
                self.include("span");
                format!("std::span<const {}>", self.ty(e)?)
            }
            _ => format!("const {} &", self.ty(k)?),
        })
    }

    /// The `Result` of a value of `k`, or of none.
    fn result(&self, k: &Option<Kind>) -> Result<String, String> {
        let v = k.as_ref().map_or(Ok("void".into()), |k| self.ty(k))?;
        Ok(format!("{}<{v}>", self.rt_use(ERROR, "Result")))
    }

    /// The implementing side of an object: its class, the object over it, and
    /// what mounts it under the interface's handle or opens it.
    fn export(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        let (t, obj) = (side_trait(o, "Guest"), object_class(o));
        self.include("memory");
        self.include("optional");
        self.include("span");
        self.include("utility");
        self.include("cstdint");
        let _ = writeln!(
            out,
            "class {t} {{\n  public:\n    virtual ~{t}() = default;"
        );
        for f in &o.methods {
            let ps: Vec<String> = self
                .kinds(f)?
                .iter()
                .map(|(n, k)| Ok(format!("{} {n}", self.ty(k)?)))
                .collect::<Result<_, String>>()?;
            let rk = self.g.ret(f)?;
            let m = name(method_name(f));
            if is_async(f) {
                let ret = rk.as_ref().map_or(Ok("void".into()), |k| self.ty(k))?;
                let ps: String = ps.iter().map(|p| format!("{p}, ")).collect();
                let _ = writeln!(
                    out,
                    "    virtual void {m}({ps}{}<{ret}> task_return) = 0;",
                    self.rt_use(LINK, "Return")
                );
            } else {
                let _ = writeln!(
                    out,
                    "    virtual {} {m}({}) = 0;",
                    self.result(&rk)?,
                    ps.join(", ")
                );
            }
        }
        let (method, tag) = (self.rt_use(LINK, "Method"), self.rt_use(LINK, "Tag"));
        let _ = writeln!(
            out,
            "}};\n\nclass {obj} final : public {} {{\n  public:\n    explicit {obj}(std::unique_ptr<{t}> implementation) noexcept : implementation_(std::move(implementation)) {{}}\n\n    std::int32_t call({method} m_, [[maybe_unused]] {tag} tag_, [[maybe_unused]] {} args_, [[maybe_unused]] {} &ret_) override {{\n        switch (m_) {{",
            self.rt_use(LINK, "Object"),
            self.rt_use(CABI, "Lift"),
            self.rt_use(CABI, "Lower")
        );
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let _ = writeln!(out, "        case {}: {{", method_const(o, f, given));
            if !ps.is_empty() {
                let ebadmsg = self.refusal(&format!("{}::ebadmsg", self.rt_use(ERROR, "Errno")));
                let (offs, _) = self
                    .g
                    .tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
                for ((n, k), at) in ps.iter().zip(&offs) {
                    let _ = writeln!(
                        out,
                        "            {} {n} = {};",
                        self.ty(k)?,
                        self.load("args_", &at.to_string(), k)?
                    );
                }
                let _ = writeln!(out, "            if (!args_.finish()) return {ebadmsg};");
            }
            let args: Vec<String> = ps
                .iter()
                .map(|(n, k)| {
                    if is_scalar(k) {
                        n.clone()
                    } else {
                        format!("std::move({n})")
                    }
                })
                .collect();
            let rk = self.g.ret(f)?;
            let m = name(method_name(f));
            let refusal = self.refusal("x_.error().errnum");
            if is_async(f) {
                let ret = rk.as_ref().map_or(Ok("void".into()), |k| self.ty(k))?;
                let args: String = args.iter().map(|a| format!("{a}, ")).collect();
                let _ = writeln!(
                    out,
                    "            implementation_->{m}({args}{}<{ret}>(tag_, {}, [tag_]({} x_) {{\n                {}\n            }}));\n            return {};\n        }}",
                    self.rt_use(LINK, "Return"),
                    self.rt_use(GUEST_LINK, "on_cancel"),
                    self.result(&rk)?,
                    self.task_return(&rk)?,
                    self.errno("einprogress")
                );
            } else {
                let _ = writeln!(
                    out,
                    "            {} x_ = implementation_->{m}({});\n            if (!x_) return {refusal};\n            {}return 0;\n        }}",
                    self.result(&rk)?,
                    args.join(", "),
                    self.put(&rk)?
                );
            }
        }
        let _ = writeln!(
            out,
            "        default:\n            return {};\n        }}\n    }}\n\n  private:\n    std::unique_ptr<{t}> implementation_;\n}};\n",
            self.errno("enosys")
        );
        if o.resource {
            let _ = writeln!(
                out,
                "// The object over `implementation`, for an opener or a function to answer.\ninline std::unique_ptr<{}> {}(std::unique_ptr<{t}> implementation) {{\n    return std::make_unique<{obj}>(std::move(implementation));\n}}\n",
                self.rt_use(LINK, "Object"),
                name(&o.name)
            );
        } else {
            let _ = writeln!(
                out,
                "// Mounts `implementation` under the interface's handle.\ninline void mount(std::unique_ptr<{t}> implementation) {{\n    {}(HANDLE, std::make_unique<{obj}>(std::move(implementation)));\n}}\n",
                self.rt_use(GUEST_LINK, "mount_named")
            );
        }
        Ok(())
    }

    /// The calling side of an object: its methods, by a handle, a
    /// singleton's its interface's named one.
    fn import(&self, o: &Obj, out: &mut String) -> Result<(), String> {
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let lents: Vec<String> = params(f)
                .filter(|p| is_lent(self.g.r, &p.ty))
                .map(|p| name(&p.name))
                .collect();
            // An async call's one lent buffer, which its answer hands back.
            let lent = if is_async(f) {
                lents.first().cloned()
            } else {
                None
            };
            // A sync call lends spans of any elements, each its own; an async
            // call lends a vector of plain values (link `Lend`).
            let generic = if lents.is_empty() {
                String::new()
            } else if is_async(f) {
                format!("template <{} L> ", self.rt_use(LINK, "Lend"))
            } else {
                let ts: Vec<String> = (0..lents.len()).map(|k| format!("class T{k}")).collect();
                format!("template <{}> ", ts.join(", "))
            };
            let mut sig = Vec::new();
            if o.resource {
                sig.push(format!(
                    "{} {}_",
                    self.rt_use(LINK, "Handle"),
                    name(&o.name)
                ));
            }
            for (n, k) in &ps {
                sig.push(match lents.iter().position(|l| l == n) {
                    Some(_) if is_async(f) => format!("L {n}"),
                    Some(j) => {
                        self.include("span");
                        format!("std::span<T{j}> {n}")
                    }
                    None => {
                        let t = self.borrowed(k)?;
                        if t.ends_with('&') {
                            format!("{t}{n}")
                        } else {
                            format!("{t} {n}")
                        }
                    }
                });
            }
            let lend: String = if lents.is_empty() {
                String::new()
            } else {
                let ty = self.rt_use(LINK, "Lent");
                let lent = self.rt_use(LINK, "lent");
                lents
                    .iter()
                    .map(|l| format!("    {ty} {l}_lent_ = {lent}({l});\n"))
                    .collect()
            };
            let h = if o.resource {
                format!("{}_", name(&o.name))
            } else {
                "HANDLE".into()
            };
            let c = method_const(o, f, given);
            let m = name(&object_method(o, f));
            let rk = self.g.ret(f)?;
            if is_async(f) {
                self.include("utility");
                let result = self.result(&rk)?;
                let (output, kept, give) = match &lent {
                    Some(l) => (
                        format!("std::pair<L, {result}>"),
                        format!("std::move({l})"),
                        format!("{{std::move(lent_), {}}}", self.answer(&rk)?),
                    ),
                    None => {
                        self.include("variant");
                        (result, "std::monostate()".into(), self.answer(&rk)?)
                    }
                };
                // The call is sent as the task is made, and the task answers it.
                let _ = writeln!(
                    out,
                    "{generic}inline {}<{output}> {m}({}) noexcept {{\n{lend}    auto sent_ = {}({h}, {c}, {}, {kept});\n    auto [result_, data_, lent_] = co_await std::move(sent_);\n    co_return {give};\n}}\n",
                    self.rt_use(WAIT, "Task"),
                    sig.join(", "),
                    self.rt_use(GUEST_WAIT, "wait"),
                    self.args(&ps, &lents)?
                );
            } else {
                let _ = writeln!(
                    out,
                    "{generic}inline {} {m}({}) noexcept {{\n{lend}    auto [result_, data_] = {}({h}, {c}, {});\n    return {};\n}}\n",
                    self.result(&rk)?,
                    sig.join(", "),
                    self.rt_use(GUEST_LINK, "call"),
                    self.args(&ps, &lents)?,
                    self.answer(&rk)?
                );
            }
        }
        Ok(())
    }
}

/// The header at `path` of `g`'s interface, whose objects are `objs`, from
/// the WIT at `src`; `role` is what the guest does with the objects, none for
/// an interface of types alone, and `library_at` how it reaches the library.
pub(super) fn hdr(
    g: &Gen,
    objs: &[Obj],
    role: Option<Role>,
    path: &str,
    src: &str,
    library_at: &Library,
) -> Result<String, String> {
    let hdr = Header {
        g,
        library_at,
        std: RefCell::default(),
        library: RefCell::default(),
        uses: RefCell::default(),
    };
    let body = hdr.interface(objs, role)?;
    let mut out = format!("// {path}\n//\n// {}\n\n#pragma once\n", mark(src, TOOL));
    if !hdr.std.borrow().is_empty() {
        out.push('\n');
    }
    for h in hdr.std.borrow().iter() {
        let _ = writeln!(out, "#include <{h}>");
    }
    if !hdr.library.borrow().is_empty() {
        out.push('\n');
    }
    for h in hdr.library.borrow().iter() {
        let _ = writeln!(out, "#include \"{h}\"");
    }
    // What the header takes from the library, which its namespace `_rt`
    // holds alone, declared before what names it.
    let mut rt = String::new();
    if !hdr.uses.borrow().is_empty() {
        rt.push_str("namespace _rt {\n");
        for (ns, names) in hdr.uses.borrow().iter() {
            for n in names {
                let _ = writeln!(rt, "using {ns}::{n};");
            }
        }
        rt.push_str("} // namespace _rt\n\n");
    }
    let ns = namespace(g.r, g.here);
    let _ = write!(
        out,
        "\nnamespace {ns} {{\n\n{rt}{body}}} // namespace {ns}\n"
    );
    Ok(out)
}
