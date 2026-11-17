// xpute-link-gen/wit/rs.rs

//! The link's bindings in Rust (wit.rs): a module per interface, its types,
//! each storing itself into a packet and loading itself from one (xpute-link
//! cabi.rs), the side that implements it and the side that calls it. The
//! guest's reach its link through its crate's `link`, the one the guest lays;
//! a host's take the side they mount on and call by (xpute-link `Side`), as
//! the TypeScript takes its link, given by the runtime that made it.

use std::fmt::Write as _;

use wit_parser::{Function, Type, TypeDefKind, TypeOwner};

use xpute_gen::spec::{pascal, rs_enum};

use super::{
    Gen, Kind, Obj, Role, cases, discriminant, is_async, is_lent, kind, members, method_const,
    method_name, object_method, params, plus, rec_layout, rs_given, rs_name, side_trait, type_name,
    variant_layout,
};

/// The Rust modules the bindings take items from, beside the link's crate.
const RS_ERRNO: &str = "xpute_kit::status::errno";

/// The object over an object's implementation, a name of the
/// generator's alone, which its leading `_` keeps from every WIT name.
fn object_struct(o: &Obj) -> String {
    if o.resource {
        format!("_{}Object", pascal(&o.name))
    } else {
        "_Object".into()
    }
}

/// What a Rust function of `n` arguments, `self` among them, carries past
/// clippy's limit, indented by `indent`: the lint written off, as the tree
/// writes it where each argument is its own (wit-bindgen likewise).
fn many_args(n: usize, indent: &str) -> String {
    if n > 7 {
        format!(
            "{indent}#[expect(clippy::too_many_arguments, reason = \"the WIT function's parameters, each its own\")]\n"
        )
    } else {
        String::new()
    }
}

/// The value the Rust expression `e`, a reference, names: the referent of a
/// borrow it writes, else its dereference.
fn deref(e: &str) -> String {
    e.strip_prefix('&')
        .map_or_else(|| format!("*{e}"), str::to_owned)
}

/// The Rust expression `e`, a reference, as a method's receiver: what a
/// borrow it writes borrows, which the call borrows again.
fn receiver(e: &str) -> String {
    e.strip_prefix('&')
        .map_or_else(|| format!("({e})"), str::to_owned)
}

impl Gen<'_> {
    fn kinds(&self, f: &Function) -> Result<Vec<(String, Kind)>, String> {
        let at = format!("{}.{}", self.at(), f.name);
        params(f)
            .map(|p| Ok((rs_name(&p.name), kind(self.r, &p.ty, &at)?)))
            .collect()
    }

    /// `name` of the Rust module `module`, reached through the module's
    /// runtime module `_rt`, which re-exports it, as wit-bindgen's bindings
    /// reach theirs.
    fn rs_use(&self, module: &str, name: &str) -> String {
        self.rs_uses
            .borrow_mut()
            .entry(module.into())
            .or_default()
            .insert(name.into());
        format!("_rt::{name}")
    }

    /// The size and alignment of `k` as Rust writes them: a record's or a
    /// variant's own constants, any other's numbers.
    fn rs_layout(&self, k: &Kind) -> Result<(String, String), String> {
        Ok(match k {
            Kind::Record(_) | Kind::Variant(_) => {
                let t = self.rs_ty(k, true);
                (format!("{t}::SZ"), format!("{t}::ALIGN"))
            }
            _ => {
                let (sz, align) = self.layout(k)?;
                (sz.to_string(), align.to_string())
            }
        })
    }

    /// Stores `e`, a reference to a value of `k`, at `at` with the writer
    /// `w`.
    fn rs_store(&self, w: &str, at: &str, e: &str, k: &Kind) -> Result<String, String> {
        Ok(match k {
            Kind::Bool => format!("{w}.bool({at}, {});", deref(e)),
            Kind::Number(n) => format!("{w}.{}({at}, {});", n.rs, deref(e)),
            Kind::Handle => format!("{w}.u32({at}, {});", deref(e)),
            Kind::Enum(t) => {
                let d = discriminant(self.case_cnt(*t))?.rs;
                format!("{w}.{d}({at}, {} as {d});", deref(e))
            }
            Kind::Str => format!("{w}.str({at}, {e});"),
            Kind::Array(_) => format!("{w}.arr({at}, {e});"),
            Kind::List(el) => {
                let (sz, align) = self.rs_layout(el)?;
                let r = receiver(e);
                format!(
                    "if let Some(b) = {w}.list({at}, {r}.len(), {sz}, {align}) {{ for (i, x) in {r}.iter().enumerate() {{ {} }} }}",
                    self.rs_store(w, &format!("b + i * {sz}"), "x", el)?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => format!("{}.store({w}, {at});", receiver(e)),
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)])?;
                format!(
                    "if let Some(v) = {e} {{ {w}.u8({at}, 1); {} }}",
                    self.rs_store(w, &plus(at, off), "v", el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>| match k {
                    Some(k) => self.rs_store(w, &plus(at, off), "v", k),
                    None => Ok(String::new()),
                };
                // A case with no payload binds nothing.
                let bind = |k: &Option<Box<Kind>>| if k.is_some() { "v" } else { "_" };
                format!(
                    "match {e} {{ Ok({}) => {{ {} }} Err({}) => {{ {w}.u8({at}, 1); {} }} }}",
                    bind(ok),
                    case(ok)?,
                    bind(err),
                    case(err)?
                )
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
                let (sz, align) = self.rs_layout(el)?;
                format!(
                    "{r}.list({at}, {sz}, {align}).and_then(|(b, n)| (0..n).map(|i| {}).collect::<Result<Vec<_>, {errno}>>())",
                    self.rs_load(r, &format!("b + i * {sz}"), el)?
                )
            }
            Kind::Record(_) | Kind::Variant(_) => {
                format!("{}::load({r}, {at})", self.rs_ty(k, true))
            }
            Kind::Option(el) => {
                let (off, _, _) = variant_layout(&[None, Some(self.layout(el)?)])?;
                format!(
                    "{r}.case({at}, 2).and_then(|d| if d == 0 {{ Ok(None) }} else {{ {}.map(Some) }})",
                    self.rs_load(r, &plus(at, off), el)?
                )
            }
            Kind::Result(ok, err) => {
                let (off, _, _) = self.result_layout(ok, err)?;
                let case = |k: &Option<Box<Kind>>, tag: &str| match k {
                    Some(k) => Ok(format!(
                        "{}.map({tag})",
                        self.rs_load(r, &plus(at, off), k)?
                    )),
                    None => Ok::<_, String>(format!("Ok({tag}(()))")),
                };
                format!(
                    "{r}.case({at}, 2).and_then(|d| if d == 0 {{ {} }} else {{ {} }})",
                    case(ok, "Ok")?,
                    case(err, "Err")?
                )
            }
        })
    }

    /// The Rust of the args `ps`: none, or their tuple's size and what writes
    /// it, each of `lents` lending memory, written as its lent record
    /// (`{lent}_lent_`), a name no WIT name escapes to.
    fn rs_args(&self, ps: &[(String, Kind)], lents: &[String]) -> Result<String, String> {
        if ps.is_empty() {
            return Ok("None".into());
        }
        let (offs, sz) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
        let mut writes = Vec::new();
        for ((n, k), at) in ps.iter().zip(&offs) {
            writes.push(if lents.contains(n) {
                format!("{n}_lent_.store(w_, {at});")
            } else {
                // A string's, a list's, a record's or a variant's param is
                // a borrow already.
                let e = match k {
                    Kind::Str
                    | Kind::Array(_)
                    | Kind::List(_)
                    | Kind::Record(_)
                    | Kind::Variant(_) => n.clone(),
                    _ => format!("&{n}"),
                };
                self.rs_store("w_", &at.to_string(), &e, k)?
            });
        }
        let w = format!(
            "|w_: &mut {}| {{ {} }}",
            self.rs_use(self.link_rs, "Lower"),
            writes.join(" ")
        );
        Ok(format!("Some(({sz}, &{w}))"))
    }

    /// The Rust that writes `v`, a method's value, as its answer at once, in
    /// the frame its args came in (`ret_`), and answers 0.
    fn rs_put(&self, rk: &Option<Kind>) -> Result<String, String> {
        Ok(match rk {
            None => "0".into(),
            Some(k) => format!(
                "{{ ret_.start({}); {} 0 }}",
                self.rs_layout(k)?.0,
                self.rs_store("ret_", "0", "&v", k)?
            ),
        })
    }

    /// The args of a later answer of `v`, a method's value, written as its
    /// frame is laid.
    fn rs_later_pkt(&self, rk: &Option<Kind>) -> Result<String, String> {
        Ok(match rk {
            None => "None".into(),
            Some(k) => format!(
                "Some(({}, &|w: &mut {}| {{ {} }}))",
                self.rs_layout(k)?.0,
                self.rs_use(self.link_rs, "Lower"),
                self.rs_store("w", "0", "&v", k)?
            ),
        })
    }

    /// The Rust of the answer's decoding, from the result `r` and the return
    /// data `data`.
    fn rs_answer(&self, k: &Option<Kind>, data: &str) -> Result<String, String> {
        let answer = self.rs_use(self.link_rs, "answer");
        Ok(match k {
            None => format!("{answer}(r)"),
            Some(k) => format!(
                "{answer}(r).and_then(|_| {{ let r_ = {}::new({data}.ok_or({}::EBADMSG)?); {} }})",
                self.rs_use(self.link_rs, "Lift"),
                self.rs_use(RS_ERRNO, "Errno"),
                self.rs_load("r_", "0", k)?
            ),
        })
    }

    /// The implementing side of an object, on the side of the crate `side`, or
    /// on a side given where `side` is none: its trait, and the object over it.
    fn rs_export(
        &self,
        o: &Obj,
        side: Option<&str>,
        side_name: &str,
        out: &mut String,
    ) -> Result<(), String> {
        let (t, obj) = (side_trait(o, side_name), object_struct(o));
        let errno = self.rs_use(RS_ERRNO, "Errno");
        let rc = self.rs_use("std::rc", "Rc");
        let _ = writeln!(out, "pub trait {t} {{");
        for f in &o.methods {
            let ps: Vec<String> = self
                .kinds(f)?
                .iter()
                .map(|(n, k)| format!("{n}: {}", self.rs_ty(k, true)))
                .collect();
            let ret = self.ret(f)?.map_or("()".into(), |k| self.rs_ty(&k, true));
            let name = rs_name(method_name(f));
            if is_async(f) {
                let _ = writeln!(
                    out,
                    "{}    fn {name}(&self, {}task_return: {}<{ret}>);",
                    many_args(ps.len() + 2, "    "),
                    ps.iter().map(|p| format!("{p}, ")).collect::<String>(),
                    self.rs_use(self.link_rs, "Return")
                );
            } else {
                let _ = writeln!(
                    out,
                    "{}    fn {name}(&self, {}) -> Result<{ret}, {errno}>;",
                    many_args(ps.len() + 1, "    "),
                    ps.join(", ")
                );
            }
        }
        // A request's tag is read where a method answers later, and the args
        // where one takes any.
        let tag = if o.methods.iter().any(|f| is_async(f)) {
            "tag_"
        } else {
            "_"
        };
        let args = if o.methods.iter().any(|f| params(f).next().is_some()) {
            "args_"
        } else {
            "_"
        };
        // The writer of the answer is taken where a method answers a value at
        // once.
        let mut answers = false;
        for f in &o.methods {
            answers |= !is_async(f) && self.ret(f)?.is_some();
        }
        let (ret, run) = if answers {
            ("ret_", "let mut run")
        } else {
            ("_", "let run")
        };
        // A host's object that answers later holds the side it is mounted on.
        let holds = side.is_none() && o.methods.iter().any(|f| is_async(f));
        let (fields, imp, made) = if holds {
            (
                format!(
                    "&'static {}, {rc}<dyn {t}>",
                    self.rs_use(self.link_rs, "Side")
                ),
                "self.1",
                "side_, implementation",
            )
        } else {
            (format!("{rc}<dyn {t}>"), "self.0", "implementation")
        };
        let _ = writeln!(
            out,
            "}}\n\nstruct {obj}({fields});\n\nimpl {} for {obj} {{\n    fn call(&self, m_: {}, {tag}: {}, {args}: {}, {ret}: &mut {}) -> i32 {{\n        {run} = || -> Result<i32, {errno}> {{\n            Ok(match m_ {{",
            self.rs_use(self.link_rs, "Object"),
            self.rs_use(self.link_rs, "Method"),
            self.rs_use(self.link_rs, "Tag"),
            self.rs_use(self.link_rs, "Lift"),
            self.rs_use(self.link_rs, "Lower")
        );
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let _ = writeln!(
                out,
                "                {} => {{",
                method_const(o, f, rs_given)
            );
            let (offs, _) = self.tuple_layout(&ps.iter().map(|(_, k)| k).collect::<Vec<_>>())?;
            for ((n, kd), at) in ps.iter().zip(&offs) {
                let _ = writeln!(
                    out,
                    "                    let {n} = {}?;",
                    self.rs_load("args_", &at.to_string(), kd)?
                );
            }
            let names: String = ps.iter().map(|(n, _)| format!("{n}, ")).collect();
            let name = rs_name(method_name(f));
            let rk = self.ret(f)?;
            // A value is bound where the method answers one.
            let v = if rk.is_some() { "v" } else { "()" };
            if is_async(f) {
                // How the request stops and how it is answered, with its
                // value or its error, on the guest's link or on the side the
                // host's object holds.
                let later = self.rs_later_pkt(&rk)?;
                let (bind, on, ok, err) = match side {
                    Some(c) => {
                        let link = self.rs_use(c, "link");
                        (
                            String::new(),
                            format!("{link}::on_cancel(tag_, stop)"),
                            format!("{link}::task_return(tag_, 0, {later})"),
                            format!("{link}::task_return(tag_, e as i32, None)"),
                        )
                    }
                    None => {
                        let packet = self.rs_use(self.link_rs, "Packet");
                        (
                            "let side_ = self.0;\n                    ".to_string(),
                            "side_.on_cancel(tag_, stop)".to_string(),
                            format!("side_.task_return(tag_, 0, {packet}::Args({later}))"),
                            format!("side_.task_return(tag_, e as i32, {packet}::Args(None))"),
                        )
                    }
                };
                let done = format!(
                    "{}::new(Box::new(move |stop| {on}), Box::new(move |got| match got {{ Ok({v}) => {{ {ok}; }} Err(e) => {{ {err}; }} }}))",
                    self.rs_use(self.link_rs, "Return"),
                );
                let _ = writeln!(
                    out,
                    "                    {bind}{imp}.{name}({names}{done});\n                    {errno}::EINPROGRESS as i32\n                }}"
                );
            } else {
                let _ = writeln!(
                    out,
                    "                    match {imp}.{name}({}) {{\n                        Ok({v}) => {},\n                        Err(e) => e as i32,\n                    }}\n                }}",
                    names.trim_end_matches(", "),
                    self.rs_put(&rk)?
                );
            }
        }
        let _ = writeln!(
            out,
            "                _ => {errno}::ENOSYS as i32,\n            }})\n        }};\n        run().unwrap_or_else(|e| e as i32)\n    }}\n}}\n"
        );
        let object = self.rs_use(self.link_rs, "Object");
        match (o.resource, side) {
            (true, Some(_)) => {
                let _ = writeln!(
                    out,
                    "/// The object over `implementation`, for an opener or a function to answer.\npub fn {}(implementation: {rc}<dyn {t}>) -> {rc}<dyn {object}> {{\n    {rc}::new({obj}(implementation))\n}}\n",
                    rs_name(&o.name)
                );
            }
            (true, None) => {
                let side_ = if holds { "side_" } else { "_" };
                let _ = writeln!(
                    out,
                    "/// The object over `implementation` on `side_`, for an opener or a function to answer.\npub fn {}({side_}: &'static {}, implementation: {rc}<dyn {t}>) -> {rc}<dyn {object}> {{\n    {rc}::new({obj}({made}))\n}}\n",
                    rs_name(&o.name),
                    self.rs_use(self.link_rs, "Side")
                );
            }
            (false, Some(c)) => {
                let _ = writeln!(
                    out,
                    "/// Mounts `implementation` under the interface's handle.\npub fn mount(implementation: {rc}<dyn {t}>) {{\n    {}::mount_named(HANDLE, {rc}::new({obj}(implementation)));\n}}\n",
                    self.rs_use(c, "link")
                );
            }
            (false, None) => {
                let _ = writeln!(
                    out,
                    "/// Mounts `implementation` under the interface's handle on `side_`.\npub fn mount(side_: &'static {}, implementation: {rc}<dyn {t}>) {{\n    side_.mount_named(HANDLE, {rc}::new({obj}({made})));\n}}\n",
                    self.rs_use(self.link_rs, "Side")
                );
            }
        }
        Ok(())
    }

    /// The calling side of an object, on the side of the crate `side`, or by
    /// a side given where `side` is none: its methods, by a handle, a
    /// singleton's its interface's named one.
    fn rs_import(&self, o: &Obj, side: Option<&str>, out: &mut String) -> Result<(), String> {
        let errno = self.rs_use(RS_ERRNO, "Errno");
        for f in &o.methods {
            let ps = self.kinds(f)?;
            let lents: Vec<String> = params(f)
                .filter(|p| is_lent(self.r, &p.ty))
                .map(|p| rs_name(&p.name))
                .collect();
            // An async call's one lent buffer, which its answer hands back.
            let lent = if is_async(f) {
                lents.first().cloned()
            } else {
                None
            };
            // A sync call lends slices of any plain elements, each its own; an
            // async call lends any buffer that says where it lies (link `Lend`).
            let lent_ty = |k: usize| {
                if is_async(f) {
                    "L".to_string()
                } else {
                    format!("&mut [T{k}]")
                }
            };
            let generic = if lents.is_empty() {
                String::new()
            } else if is_async(f) {
                format!("<L: {}>", self.rs_use(self.link_rs, "Lend"))
            } else {
                let ts: Vec<String> = (0..lents.len())
                    .map(|k| format!("T{k}: Copy + Unpin + 'static"))
                    .collect();
                format!("<{}>", ts.join(", "))
            };
            // A host's call takes the side it calls by, a later answer's for
            // the life of the side.
            let mut sig: Vec<String> = match side {
                Some(_) => vec![],
                None if is_async(f) => {
                    vec![format!(
                        "side_: &'static {}",
                        self.rs_use(self.link_rs, "Side")
                    )]
                }
                None => vec![format!("side_: &{}", self.rs_use(self.link_rs, "Side"))],
            };
            if o.resource {
                sig.push(format!(
                    "{}_: {}",
                    rs_name(&o.name),
                    self.rs_use(self.link_rs, "Handle")
                ));
            }
            sig.extend(
                ps.iter()
                    .map(|(n, k)| match lents.iter().position(|l| l == n) {
                        Some(j) => format!("{n}: {}", lent_ty(j)),
                        None => format!("{n}: {}", self.rs_ty(k, false)),
                    }),
            );
            let h = if o.resource {
                format!("{}_", rs_name(&o.name))
            } else {
                "HANDLE".into()
            };
            let rk = self.ret(f)?;
            let ret = rk.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
            let lend: String = lents
                .iter()
                .map(|l| {
                    if is_async(f) {
                        format!(
                            "let mut {l} = {l}; let {l}_lent_ = {}::lent(&mut {l}); ",
                            self.rs_use(self.link_rs, "Lend")
                        )
                    } else {
                        format!(
                            "let {l}_lent_ = {}::lent(&{l}[..]); ",
                            self.rs_use(self.link_rs, "mem")
                        )
                    }
                })
                .collect();
            let c = method_const(o, f, rs_given);
            let name = rs_name(&object_method(o, f));
            if is_async(f) {
                let (output, kept, give) = match &lent {
                    Some(l) => (
                        format!("(L, Result<{ret}, {errno}>)"),
                        l.clone(),
                        format!("(lent, {})", self.rs_answer(&rk, "data.as_deref()")?),
                    ),
                    None => (
                        format!("Result<{ret}, {errno}>"),
                        "()".into(),
                        self.rs_answer(&rk, "data.as_deref()")?,
                    ),
                };
                // The call is sent here, and the future answers it.
                let args = self.rs_args(&ps, &lents)?;
                let sent = match side {
                    Some(s) => {
                        format!("{}::wait({h}, {c}, {args}, {kept})", self.rs_use(s, "wait"))
                    }
                    None => format!(
                        "{}(move |h_, m_, a_, w_| side_.send(h_, m_, {}::Args(a_), w_), Box::new(move |t_| side_.cancel(t_)), {h}, {c}, {args}, {kept})",
                        self.rs_use(&format!("{}::wait", self.link_rs), "wait_by"),
                        self.rs_use(self.link_rs, "Packet")
                    ),
                };
                let _ = writeln!(
                    out,
                    "{}pub fn {name}{generic}({}) -> impl Future<Output = {output}> + use<{}> {{\n    {lend}\n    let sent = {sent};\n    async move {{\n        let (r, {}, {}) = sent.await;\n        {give}\n    }}\n}}\n",
                    many_args(sig.len(), ""),
                    sig.join(", "),
                    if lents.is_empty() { "" } else { "L" },
                    if rk.is_some() { "data" } else { "_" },
                    if lent.is_some() { "lent" } else { "_" }
                );
            } else {
                let args = self.rs_args(&ps, &lents)?;
                let call = match side {
                    Some(s) => format!("{}::call({h}, {c}, {args})", self.rs_use(s, "link")),
                    None => format!(
                        "side_.call({h}, {c}, {}::Args({args}))",
                        self.rs_use(self.link_rs, "Packet")
                    ),
                };
                let _ = writeln!(
                    out,
                    "{}pub fn {name}{generic}({}) -> Result<{ret}, {errno}> {{\n    {lend}let (r, {}) = {call};\n    {}\n}}\n",
                    many_args(sig.len(), ""),
                    sig.join(", "),
                    if rk.is_some() { "data" } else { "_" },
                    self.rs_answer(&rk, "data")?
                );
            }
        }
        Ok(())
    }

    /// The interface's Rust module, by `role` on the side of its crate, none
    /// for an interface of types alone.
    pub(super) fn rs_interface(
        &self,
        objs: &[Obj],
        role: Option<(Role, Option<&str>, &str)>,
    ) -> Result<String, String> {
        let mut items = String::new();
        if !objs.is_empty() {
            let _ = writeln!(
                items,
                "/// The interface's handle, named by `{}` (frame.json5 `NAMED`).\npub const HANDLE: {} = {:#x};\n",
                self.at(),
                self.rs_use(self.link_rs, "Handle"),
                self.handle()
            );
        }
        for o in objs {
            for (k, f) in o.methods.iter().enumerate() {
                let _ = writeln!(
                    items,
                    "/// `{}`\npub const {}: {} = {k};",
                    f.name,
                    method_const(o, f, rs_given),
                    self.rs_use(self.link_rs, "Method")
                );
            }
        }
        items.push('\n');
        self.rs_types(&mut items)?;
        if let Some((role, side, side_name)) = role {
            for o in objs {
                match role {
                    Role::Implement => self.rs_export(o, side, side_name, &mut items)?,
                    Role::Call => self.rs_import(o, side, &mut items)?,
                }
            }
        }
        // What the module takes from other modules, which its runtime module
        // holds alone (wit-bindgen's `_rt`) for the module above it.
        let mut body = items;
        if !self.rs_uses.borrow().is_empty() {
            body.push_str("mod _rt {\n");
            for (m, names) in self.rs_uses.borrow().iter() {
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                let _ = writeln!(body, "    pub(super) use {m}::{{{}}};", names.join(", "));
            }
            body.push_str("}\n");
        }
        Ok(body)
    }

    fn rs_ty(&self, k: &Kind, owned: bool) -> String {
        match k {
            Kind::Bool => "bool".into(),
            Kind::Number(n) => n.rs.into(),
            Kind::Str => if owned { "String" } else { "&str" }.into(),
            Kind::Array(n) => {
                if owned {
                    format!("Vec<{}>", n.rs)
                } else {
                    format!("&[{}]", n.rs)
                }
            }
            Kind::List(e) => {
                let e = self.rs_ty(e, true);
                if owned {
                    format!("Vec<{e}>")
                } else {
                    format!("&[{e}]")
                }
            }
            Kind::Record(t) | Kind::Variant(t) | Kind::Enum(t) => {
                let (m, n) = self.type_path(*t, rs_given);
                let p = match m {
                    Some(m) if m.rs.starts_with("super::") => format!("{}::{n}", m.rs),
                    Some(m) => self.rs_use(&m.rs, &n),
                    None => n,
                };
                if owned || matches!(k, Kind::Enum(_)) {
                    p
                } else {
                    format!("&{p}")
                }
            }
            Kind::Option(e) => format!("Option<{}>", self.rs_ty(e, owned)),
            Kind::Result(ok, err) => {
                let case =
                    |k: &Option<Box<Kind>>| k.as_ref().map_or("()".into(), |k| self.rs_ty(k, true));
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
                    let _ = writeln!(
                        out,
                        "}}\n\nimpl {n} {{\n    pub const SZ: usize = {sz};\n    pub const ALIGN: usize = {align};\n\n    pub fn store(&self, w: &mut {}, at: usize) {{",
                        lower()
                    );
                    for ((f, k), &off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            out,
                            "        {}",
                            self.rs_store(
                                "w",
                                &plus("at", off),
                                &format!("&self.{}", rs_name(f)),
                                k
                            )?
                        );
                    }
                    let _ = writeln!(
                        out,
                        "    }}\n\n    pub fn load(r: {}<'_>, at: usize) -> Result<{n}, {}> {{\n        Ok({n} {{",
                        lift(),
                        errno()
                    );
                    for ((f, k), &off) in fields.iter().zip(&offs) {
                        let _ = writeln!(
                            out,
                            "            {}: {}?,",
                            rs_name(f),
                            self.rs_load("r", &plus("at", off), k)?
                        );
                    }
                    let _ = writeln!(out, "        }})\n    }}\n}}\n");
                }
                TypeDefKind::Variant(v) => {
                    let cases = cases(self.r, v, &at)?;
                    let (off, sz, align) = variant_layout(&self.payload_layouts(t)?)?;
                    let d = discriminant(cases.len())?.rs;
                    let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]\npub enum {n} {{");
                    for (c, k) in &cases {
                        match k {
                            Some(k) => {
                                let _ =
                                    writeln!(out, "    {}({}),", pascal(c), self.rs_ty(k, true));
                            }
                            None => {
                                let _ = writeln!(out, "    {},", pascal(c));
                            }
                        }
                    }
                    let _ = writeln!(
                        out,
                        "}}\n\nimpl {n} {{\n    pub const SZ: usize = {sz};\n    pub const ALIGN: usize = {align};\n\n    pub fn store(&self, w: &mut {}, at: usize) {{\n        match self {{",
                        lower()
                    );
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(
                                    out,
                                    "            {n}::{}(v) => {{ w.{d}(at, {k}); {} }}",
                                    pascal(c),
                                    self.rs_store("w", &plus("at", off), "v", kd)?
                                );
                            }
                            None => {
                                let _ = writeln!(
                                    out,
                                    "            {n}::{} => w.{d}(at, {k}),",
                                    pascal(c)
                                );
                            }
                        }
                    }
                    let _ = writeln!(
                        out,
                        "        }}\n    }}\n\n    pub fn load(r: {}<'_>, at: usize) -> Result<{n}, {}> {{\n        match r.case(at, {})? {{",
                        lift(),
                        errno(),
                        cases.len()
                    );
                    for (k, (c, kd)) in cases.iter().enumerate() {
                        match kd {
                            Some(kd) => {
                                let _ = writeln!(
                                    out,
                                    "            {k} => {}.map({n}::{}),",
                                    self.rs_load("r", &plus("at", off), kd)?,
                                    pascal(c)
                                );
                            }
                            None => {
                                let _ = writeln!(out, "            {k} => Ok({n}::{}),", pascal(c));
                            }
                        }
                    }
                    let _ = writeln!(
                        out,
                        "            _ => Err({}::EBADMSG),\n        }}\n    }}\n}}\n",
                        errno()
                    );
                }
                // A type `use`d from another interface is reached by its owner's path.
                TypeDefKind::Type(ty) if !matches!(ty, Type::Id(_)) => {
                    let _ = writeln!(
                        out,
                        "pub type {n} = {};\n",
                        self.rs_ty(&kind(self.r, ty, &at)?, true)
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
