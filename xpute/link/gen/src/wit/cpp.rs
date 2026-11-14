// xpute-link-gen/wit/cpp.rs

//! The link's bindings in C++, a header per interface, in the shape of
//! wit-bindgen's C bindings and in C++'s own forms: the interface's handle
//! and its methods' numbers, and its types, each storing itself into a packet
//! and loading itself from one (cpp/xpute/link/cabi.hpp) at the offsets the
//! Rust and the TypeScript take, from the one layout of each type
//! (`Gen::layout`). A record is a struct, an enum xpute-gen's (`cpp_enum`), a
//! variant a struct over a `std::variant` of one struct per case, an option a
//! `std::optional`, a result a `std::variant` of its ok and its error, a
//! string a `std::string` and a list a `std::vector`. A name is its WIT name
//! in C++'s casing: a type's and a case's PascalCase, a namespace's, a
//! field's and an enum member's snake_case, a constant's SCREAMING_CASE. An
//! interface's namespace is its package's namespace, `abi`, its package's
//! name and its own: the words wit-bindgen's C prefixes a name with, under
//! `abi` as the bindings' Rust modules are.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use wit_parser::{InterfaceId, PackageId, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use xpute_gen::spec::{cpp_enum, cpp_ident, cpp_member, mark, pascal, snake, Includes, Member};

use super::{
    cases, discriminant_type, escaped, kind, members, method_const, package_name, rec_layout,
    rs_name, type_name, variant_layout, Gen, Kind, Obj, WitTarget, NUMBERS, TOOL,
};

/// The link's packet writer and reader, from the library's root.
const CABI: &str = "xpute/link/cabi.hpp";

/// The names the C++ bindings give in a scope a WIT name shares: the
/// interface's handle, and a record's store and load. A WIT name that meets
/// one takes `_`.
const GIVEN: [&str; 3] = ["HANDLE", "store", "load"];

fn given(n: &str) -> bool {
    GIVEN.contains(&n)
}

/// A WIT name as a C++ identifier in snake_case.
fn name(n: &str) -> String {
    escaped(cpp_ident(&snake(n)), given)
}

/// The namespace of the interface `i` (the module's doc).
fn namespace(r: &Resolve, i: InterfaceId) -> String {
    let pkg = package_name(r, i);
    let (ns, p) = pkg.split_once(':').unwrap_or_default();
    let iname = r.interfaces[i].name.as_deref().unwrap_or_default();
    [name(ns), "abi".into(), name(p), name(iname)].join("::")
}

/// Where each package of `peers` (each with its root) written in C++ has its
/// headers, by its `namespace:package`: its directory as an include names it,
/// from its library's root, the nearest directory holding `compile_flags.txt`
/// (`-I.`).
pub(super) fn externs(
    r: &Resolve,
    ids: &[PackageId],
    peers: &[&(&Path, &WitTarget)],
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (&id, (pr, p)) in ids.iter().zip(peers) {
        let Some(dir) = p.cpp else { continue };
        let library = Path::new(dir)
            .ancestors()
            .find(|d| pr.join(d).join("compile_flags.txt").is_file())
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
    externs: &'a [(String, String)],
    /// The standard's headers it includes, and the library's.
    std: RefCell<Includes>,
    library: RefCell<BTreeSet<String>>,
}

impl Header<'_> {
    fn include(&self, hdr: &'static str) {
        self.std.borrow_mut().insert(hdr);
    }

    /// A discriminant's C++ type, of `n` cases.
    fn discriminant_type(&self, n: usize) -> &'static str {
        self.include("cstdint");
        let rs = discriminant_type(n);
        NUMBERS.iter().find(|x| x.rs == rs).map_or("", |x| x.cpp)
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
            let hdr = if pkg == self.g.pkg {
                format!("{m}.hpp")
            } else {
                let (_, dir) = self
                    .externs
                    .iter()
                    .find(|(p, _)| *p == pkg)
                    .ok_or(format!("{}: {pkg} is written in no C++", self.g.at()))?;
                format!("{dir}/{m}.hpp")
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
                n.cpp.into()
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
                format!("std::vector<{}>", n.cpp)
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

    /// Stores `e`, a value of `k`, at `at` with the writer `w`.
    fn store(&self, w: &str, at: &str, e: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool | Kind::Number(_) | Kind::Handle => {
                format!("{w}.put<{}>({at}, {e});", self.ty(k)?)
            }
            Kind::Enum(t) => {
                let u = self.discriminant_type(self.g.case_cnt(*t));
                format!("{w}.put<{u}>({at}, static_cast<{u}>({e}));")
            }
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(n) => format!("{w}.arr<{}>({at}, {e});", n.cpp),
            Kind::List(el) => {
                let (sz, align) = self.g.layout(el)?;
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
                let (off, _, _) = variant_layout(&[None, Some(self.g.layout(el)?)]);
                self.include("cstdint");
                format!(
                    "if ({e}) {{ {w}.put<std::uint8_t>({at}, 1); {} }}",
                    self.store(w, &format!("{at} + {off}"), &format!("(*{e})"), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.g.result_layout(ok, err)?;
                self.include("cstdint");
                let case = |k: &Option<Box<Kind>>, c: usize| -> Result<String, String> {
                    match k {
                        Some(k) => Ok(format!(
                            " {}",
                            self.store(
                                w,
                                &format!("{at} + {off}"),
                                &format!("std::get<{c}>({e})"),
                                k
                            )?
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
            Kind::Array(n) => format!("{r}.arr<{}>({at})", n.cpp),
            Kind::List(el) => {
                let (sz, align) = self.g.layout(el)?;
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
                let (off, _, _) = variant_layout(&[None, Some(self.g.layout(el)?)]);
                let ty = self.ty(k)?;
                format!(
                    "({r}.case_({at}, 2) == 0 ? {ty}() : {ty}({}))",
                    self.load(r, &format!("{at} + {off}"), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.g.result_layout(ok, err)?;
                let ty = self.ty(k)?;
                self.include("utility");
                let case = |k: &Option<Box<Kind>>, c: usize| -> Result<String, String> {
                    let v = match k {
                        Some(k) => self.load(r, &format!("{at} + {off}"), k)?,
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
        self.library.borrow_mut().insert(CABI.into());
        format!("    static constexpr std::size_t SZ = {sz};\n    static constexpr std::size_t ALIGN = {align};\n\n    void store(::xpute::link::Lower &w, std::size_t at) const noexcept {{\n{store}    }}\n\n    static {n} load(::xpute::link::Lift &r, std::size_t at) noexcept {{\n")
    }

    /// The interface's handle and its methods' numbers, and its types.
    fn interface(&self, objs: &[Obj]) -> Result<String, String> {
        let g = self.g;
        let mut out = String::new();
        if objs.iter().any(|o| !o.resource) {
            self.include("cstdint");
            let _ = writeln!(out, "// The interface's handle, named by `{}` (frame.json `NAMED`).\ninline constexpr std::uint32_t HANDLE = {:#x};\n", g.at(), g.handle());
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                self.include("cstdint");
                let _ = writeln!(
                    out,
                    "// `{}`\ninline constexpr std::uint32_t {} = {k};",
                    f.name,
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
                    for ((f, k), off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            store,
                            "        {}",
                            self.store(
                                "w",
                                &format!("at + {off}"),
                                &format!("this->{}", name(f)),
                                k
                            )?
                        );
                    }
                    let _ = writeln!(
                        out,
                        "\n{}        return {n}{{",
                        self.cabi(&n, sz, align, &store)
                    );
                    for ((f, k), off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            out,
                            "            .{} = {},",
                            name(f),
                            self.load("r", &format!("at + {off}"), k)?
                        );
                    }
                    out.push_str("        };\n    }\n};\n\n");
                }
                TypeDefKind::Variant(v) => {
                    self.include("variant");
                    let cases = cases(g.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&g.payload_layouts(t)?);
                    let u = self.discriminant_type(cases.len());
                    // A case named as its variant would name the variant's
                    // own struct.
                    let case = |c: &str| {
                        let p = pascal(c);
                        if p == n {
                            p + "_"
                        } else {
                            p
                        }
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
                                    &format!("at + {off}"),
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
                            Some(kd) => self.load("r", &format!("at + {off}"), kd)?,
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
        Ok(out)
    }
}

/// The header at `path` of `g`'s interface, whose objects are `objs`, from
/// the WIT at `src`; `externs` are where other packages' headers are
/// (`externs`).
pub(super) fn hdr(
    g: &Gen,
    objs: &[Obj],
    path: &str,
    src: &str,
    externs: &[(String, String)],
) -> Result<String, String> {
    let hdr = Header {
        g,
        externs,
        std: RefCell::default(),
        library: RefCell::default(),
    };
    let body = hdr.interface(objs)?;
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
    let ns = namespace(g.r, g.here);
    let _ = write!(out, "\nnamespace {ns} {{\n\n{body}}} // namespace {ns}\n");
    Ok(out)
}
