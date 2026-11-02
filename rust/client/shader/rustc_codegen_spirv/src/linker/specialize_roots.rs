// PATCH(pixelet): new file; see the crate's README.md.

//! Rewrites away pointer parameters SPIR-V's Logical addressing does not
//! allow (a `StorageBuffer`/`Uniform`/... pointer, an aggregate holding such
//! pointers, or a `Function` pointer to such an aggregate), so the inliner
//! (see `inline.rs`) need not inline every function taking one to legalize it.
//!
//! As Tint's `DirectVariableAccess` does for WGSL, a callee is cloned per
//! distinct tuple of the root global variables (and constant access chain
//! indices) its pointer arguments come from: in the clone, each such
//! pointer is rebuilt by an access chain from its root global, the dynamic
//! indices and the non-pointer parts of an aggregate passed as values. A
//! `Function` pointer to an aggregate holding pointers is cloned only where
//! the callee never writes through it (so the clone's local copy is exact).
//! A call whose pointers cannot be traced to their roots is left as it is,
//! for the inliner to legalize as before. What no clone can rebuild (a
//! pointer returned, a closure's captures of locals) is inlined first
//! (`unspecializable`), and a field's address passed where a parameter must
//! be a whole variable is passed as a copy (`copy_pointer_arguments`).

use crate::custom_insts::CUSTOM_EXT_INST_SET;
use rspirv::dr::{Block, Function, Instruction, Module, Operand};
use rspirv::spirv::{Op, StorageClass, Word};
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// How a parameter carries pointers.
#[derive(Copy, Clone, PartialEq)]
enum Kind {
    /// A pointer of a storage class no parameter may have (`Ptr`).
    Ptr,
    /// An aggregate value with such pointers among its leaves.
    Value,
    /// A `Function` pointer to such an aggregate.
    Mem,
}

/// One leaf of a carried type: its path of member indices, its type, and
/// whether it is a pointer (which must be rebuilt from a root global).
struct Leaf {
    path: Vec<u32>,
    ty: Word,
    ptr: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Index {
    Const(Word),
    Dyn(Word),
}

/// A pointer as an access chain from a module-scope variable.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Root {
    var: Word,
    indices: Vec<Index>,
}

/// A clone's key: per parameter, per leaf, the root of a pointer leaf
/// (with dynamic indices by their type), `None` for a value leaf.
type Shape = Vec<Option<Root>>;

/// The definitions and uses within one function.
struct Body<'a> {
    defs: FxHashMap<Word, &'a Instruction>,
    uses: FxHashMap<Word, Vec<&'a Instruction>>,
    params: Vec<Word>,
}

impl<'a> Body<'a> {
    fn of(f: &'a Function) -> Self {
        let mut defs = FxHashMap::default();
        let mut uses: FxHashMap<Word, Vec<&'a Instruction>> = FxHashMap::default();
        for inst in f.all_inst_iter() {
            if let Some(id) = inst.result_id {
                defs.insert(id, inst);
            }
            for op in &inst.operands {
                if let Some(id) = op.id_ref_any() {
                    uses.entry(id).or_default().push(inst);
                }
            }
        }
        let params = f.parameters.iter().map(|p| p.result_id.unwrap()).collect();
        Body { defs, uses, params }
    }
}

/// Where a trace stands: a function's body, and, inside a callee traced
/// through for its return value, the caller's place and arguments.
#[derive(Copy, Clone)]
struct At<'s> {
    body: &'s Body<'s>,
    caller: Option<(&'s At<'s>, &'s [Word])>,
}

struct Cx<'m> {
    types: FxHashMap<Word, &'m Instruction>,
    globals: FxHashSet<Word>,
    constants: FxHashSet<Word>,
    templates: FxHashMap<Word, &'m Function>,
    custom_set: Option<Word>,
    read_only: std::cell::RefCell<FxHashMap<(Word, usize), bool>>,
    phis: std::cell::RefCell<Vec<Word>>,
}

const MAX_DEPTH: u32 = 8;

impl<'m> Cx<'m> {
    fn new(types: &'m [Instruction], imports: &'m [Instruction]) -> Self {
        let mut cx = Cx {
            types: FxHashMap::default(),
            globals: FxHashSet::default(),
            constants: FxHashSet::default(),
            templates: FxHashMap::default(),
            custom_set: imports
                .iter()
                .find(|i| i.operands[0].unwrap_literal_string() == &CUSTOM_EXT_INST_SET[..])
                .and_then(|i| i.result_id),
            read_only: Default::default(),
            phis: Default::default(),
        };
        let mut int32 = FxHashSet::default();
        for inst in types {
            let id = inst.result_id.unwrap();
            cx.types.insert(id, inst);
            match inst.class.opcode {
                Op::TypeInt if inst.operands[0].unwrap_literal_bit32() == 32 => {
                    int32.insert(id);
                }
                Op::Variable => {
                    cx.globals.insert(id);
                }
                Op::Constant if int32.contains(&inst.result_type.unwrap()) => {
                    cx.constants.insert(id);
                }
                _ => {}
            }
        }
        cx
    }

    fn pointer(&self, ty: Word) -> Option<(StorageClass, Word)> {
        let t = self.types.get(&ty)?;
        (t.class.opcode == Op::TypePointer).then(|| {
            (
                t.operands[0].unwrap_storage_class(),
                t.operands[1].unwrap_id_ref(),
            )
        })
    }

    fn members(&self, ty: Word) -> Option<Vec<Word>> {
        let t = self.types.get(&ty)?;
        match t.class.opcode {
            Op::TypeStruct => Some(t.operands.iter().map(|o| o.unwrap_id_ref()).collect()),
            Op::TypeArray => {
                let len = self.types.get(&t.operands[1].unwrap_id_ref())?;
                let n = match len.operands.first()? {
                    Operand::LiteralBit32(n) if len.class.opcode == Op::Constant => *n,
                    _ => return None,
                };
                Some(vec![t.operands[0].unwrap_id_ref(); n as usize])
            }
            _ => None,
        }
    }

    fn carries(&self, ty: Word) -> bool {
        self.pointer(ty).is_some()
            || self
                .members(ty)
                .is_some_and(|m| m.iter().any(|&t| self.carries(t)))
    }

    fn kind(&self, ty: Word) -> Option<Kind> {
        match self.pointer(ty) {
            Some((sc, pointee)) => {
                if !legal_param_class(sc) {
                    Some(Kind::Ptr)
                } else if sc == StorageClass::Function && self.carries(pointee) {
                    Some(Kind::Mem)
                } else {
                    None
                }
            }
            None => self.carries(ty).then_some(Kind::Value),
        }
    }

    /// The leaves of a carried type, depth first; `None` where a pointer
    /// leaf is of a class no global variable has.
    fn leaves(&self, ty: Word, path: &mut Vec<u32>, out: &mut Vec<Leaf>) -> Option<()> {
        if let Some((sc, _)) = self.pointer(ty) {
            if sc == StorageClass::Function {
                return None;
            }
            out.push(Leaf {
                path: path.clone(),
                ty,
                ptr: true,
            });
        } else if let Some(members) = self.members(ty).filter(|_| self.carries(ty)) {
            for (i, m) in members.into_iter().enumerate() {
                path.push(i as u32);
                self.leaves(m, path, out)?;
                path.pop();
            }
        } else {
            out.push(Leaf {
                path: path.clone(),
                ty,
                ptr: false,
            });
        }
        Some(())
    }

    /// The leaves a parameter of `kind` and type `ty` is split into.
    fn param_leaves(&self, kind: Kind, ty: Word) -> Option<Vec<Leaf>> {
        let mut out = vec![];
        match kind {
            Kind::Ptr => out.push(Leaf {
                path: vec![],
                ty,
                ptr: true,
            }),
            Kind::Value => self.leaves(ty, &mut vec![], &mut out)?,
            Kind::Mem => self.leaves(self.pointer(ty)?.1, &mut vec![], &mut out)?,
        }
        Some(out)
    }

    fn is_debuginfo(&self, inst: &Instruction) -> bool {
        matches!(inst.class.opcode, Op::Line | Op::NoLine)
            || inst.class.opcode == Op::ExtInst
                && Some(inst.operands[0].unwrap_id_ref()) == self.custom_set
    }

    /// Whether `f` never writes through its `i`th parameter (a pointer, or
    /// an aggregate holding one), nor lets it escape.
    fn read_only(&self, f: Word, i: usize) -> bool {
        if let Some(&r) = self.read_only.borrow().get(&(f, i)) {
            return r;
        }
        let r = self.templates.get(&f).is_some_and(|func| {
            let body = Body::of(func);
            self.ptr_read_only(&body, body.params[i], &mut FxHashSet::default())
        });
        self.read_only.borrow_mut().insert((f, i), r);
        r
    }

    /// Whether nothing writes through `ptr`, or through a pointer derived
    /// from it (an access chain, a phi, an aggregate holding it, a callee's
    /// parameter), and none escapes to memory or a return.
    fn ptr_read_only(&self, body: &Body<'_>, ptr: Word, seen: &mut FxHashSet<Word>) -> bool {
        if !seen.insert(ptr) {
            return true;
        }
        body.uses.get(&ptr).into_iter().flatten().all(|u| {
            let mut onward = || self.ptr_read_only(body, u.result_id.unwrap(), seen);
            match u.class.opcode {
                Op::Load => true,
                Op::CopyMemory => u.operands[0].unwrap_id_ref() != ptr,
                Op::AccessChain
                | Op::InBoundsAccessChain
                | Op::Phi
                | Op::CopyObject
                | Op::Select
                | Op::CompositeInsert
                | Op::CompositeConstruct => onward(),
                Op::CompositeExtract => !self.carries(u.result_type.unwrap()) || onward(),
                Op::FunctionCall => {
                    let callee = u.operands[0].unwrap_id_ref();
                    callee != ptr
                        && u.operands[1..]
                            .iter()
                            .enumerate()
                            .filter(|(_, o)| o.id_ref_any() == Some(ptr))
                            .all(|(k, _)| self.read_only(callee, k))
                }
                _ => self.is_debuginfo(u),
            }
        })
    }

    fn index(&self, body: &Body<'_>, id: Word, dyn_ok: bool) -> Option<Index> {
        if self.constants.contains(&id) {
            Some(Index::Const(id))
        } else {
            // A parameter dominates every call; another value only where the
            // trace went from the call's argument through its definitions.
            (body.params.contains(&id) || dyn_ok && body.defs.contains_key(&id))
                .then_some(Index::Dyn(id))
        }
    }

    /// The root of the pointer at `path` within the value `id`.
    fn trace(&self, at: At<'_>, id: Word, path: &[u32], dyn_ok: bool, depth: u32) -> Option<Root> {
        if depth > MAX_DEPTH {
            return None;
        }
        if self.globals.contains(&id) {
            return path.is_empty().then(|| Root {
                var: id,
                indices: vec![],
            });
        }
        if let Some(k) = at.body.params.iter().position(|&p| p == id) {
            let (caller, args) = at.caller?;
            return self.trace(*caller, args[k], path, false, depth);
        }
        let def = at.body.defs.get(&id)?;
        let ids = |from: usize| def.operands[from..].iter().map(|o| o.unwrap_id_ref());
        let lits = |from: usize| {
            def.operands[from..]
                .iter()
                .map(|o| o.unwrap_literal_bit32())
        };
        match def.class.opcode {
            Op::AccessChain | Op::InBoundsAccessChain if path.is_empty() => {
                let mut root =
                    self.trace(at, def.operands[0].unwrap_id_ref(), &[], dyn_ok, depth)?;
                for i in ids(1) {
                    if at.caller.is_some() && !self.constants.contains(&i) {
                        return None;
                    }
                    root.indices.push(self.index(at.body, i, dyn_ok)?);
                }
                Some(root)
            }
            Op::CopyObject => self.trace(at, def.operands[0].unwrap_id_ref(), path, dyn_ok, depth),
            Op::Phi => {
                if self.phis.borrow().contains(&id) {
                    return None;
                }
                self.phis.borrow_mut().push(id);
                let mut found = None;
                let mut ok = true;
                for v in def.operands.iter().step_by(2).map(|o| o.unwrap_id_ref()) {
                    if self.phis.borrow().contains(&v) {
                        continue;
                    }
                    match self.trace(at, v, path, false, depth) {
                        Some(r) if found.get_or_insert_with(|| r.clone()) == &r => {}
                        _ => ok = false,
                    }
                    if !ok {
                        break;
                    }
                }
                self.phis.borrow_mut().pop();
                found.filter(|_| ok)
            }
            Op::CompositeConstruct => {
                let (&first, rest) = path.split_first()?;
                let member = def.operands.get(first as usize)?.unwrap_id_ref();
                self.trace(at, member, rest, dyn_ok, depth)
            }
            Op::CompositeInsert => {
                let at_insert: Vec<u32> = lits(2).collect();
                if path.starts_with(&at_insert) {
                    let rest = &path[at_insert.len()..];
                    self.trace(at, def.operands[0].unwrap_id_ref(), rest, dyn_ok, depth)
                } else if at_insert.starts_with(path) {
                    None
                } else {
                    self.trace(at, def.operands[1].unwrap_id_ref(), path, dyn_ok, depth)
                }
            }
            Op::CompositeExtract => {
                let full: Vec<u32> = lits(1).chain(path.iter().copied()).collect();
                self.trace(at, def.operands[0].unwrap_id_ref(), &full, dyn_ok, depth)
            }
            Op::Load => self.trace_mem(at, def.operands[0].unwrap_id_ref(), path, depth + 1),
            Op::FunctionCall => {
                let callee = self.templates.get(&def.operands[0].unwrap_id_ref())?;
                let body = Body::of(callee);
                let args: Vec<Word> = ids(1).collect();
                let inner = At {
                    body: &body,
                    caller: Some((&at, &args)),
                };
                let mut found = None;
                for inst in callee
                    .all_inst_iter()
                    .filter(|i| i.class.opcode == Op::ReturnValue)
                {
                    let r = self.trace(
                        inner,
                        inst.operands[0].unwrap_id_ref(),
                        path,
                        false,
                        depth + 1,
                    )?;
                    if found.get_or_insert_with(|| r.clone()) != &r {
                        return None;
                    }
                }
                found
            }
            _ => None,
        }
    }

    /// The root of the pointer at `path` within the memory `ptr` points to:
    /// a local variable (or a constant access chain into one) all of whose
    /// writes that reach the pointer store the same root.
    fn trace_mem(&self, at: At<'_>, ptr: Word, path: &[u32], depth: u32) -> Option<Root> {
        let (var, prefix) = self.place(at.body, ptr, &mut vec![])?;
        let full: Vec<u32> = prefix.into_iter().chain(path.iter().copied()).collect();
        let mut found = None;
        self.writes(
            at,
            var,
            &[],
            &full,
            &mut found,
            &mut FxHashSet::default(),
            depth,
        )?;
        found
    }

    /// The local variable `ptr` points into, and the constant path to where.
    fn place(&self, body: &Body<'_>, ptr: Word, phis: &mut Vec<Word>) -> Option<(Word, Vec<u32>)> {
        let def = body.defs.get(&ptr)?;
        match def.class.opcode {
            Op::Variable => Some((ptr, vec![])),
            Op::AccessChain | Op::InBoundsAccessChain => {
                let (var, mut path) = self.place(body, def.operands[0].unwrap_id_ref(), phis)?;
                for o in &def.operands[1..] {
                    path.push(self.literal(o.unwrap_id_ref())?);
                }
                Some((var, path))
            }
            Op::Phi => {
                phis.push(ptr);
                let mut found = None;
                for v in def.operands.iter().step_by(2).map(|o| o.unwrap_id_ref()) {
                    if phis.contains(&v) {
                        continue;
                    }
                    let p = self.place(body, v, phis)?;
                    if found.get_or_insert_with(|| p.clone()) != &p {
                        return None;
                    }
                }
                phis.pop();
                found
            }
            _ => None,
        }
    }

    /// Every write to `ptr` (at `at_path` within the variable) that may
    /// reach `full`; `None` where one may write it unseen.
    fn writes(
        &self,
        at: At<'_>,
        ptr: Word,
        at_path: &[u32],
        full: &[u32],
        found: &mut Option<Root>,
        seen: &mut FxHashSet<Word>,
        depth: u32,
    ) -> Option<()> {
        if !seen.insert(ptr) {
            return Some(());
        }
        let overlaps = |p: &[u32]| p.starts_with(full) || full.starts_with(p);
        for u in at.body.uses.get(&ptr).into_iter().flatten() {
            match u.class.opcode {
                Op::Load => {}
                Op::Phi => {
                    self.writes(at, u.result_id.unwrap(), at_path, full, found, seen, depth)?
                }
                Op::CopyMemory if u.operands[0].unwrap_id_ref() != ptr => {}
                Op::CopyMemory => {
                    if overlaps(at_path) {
                        let rest = full.strip_prefix(at_path)?;
                        let r =
                            self.trace_mem(at, u.operands[1].unwrap_id_ref(), rest, depth + 1)?;
                        if found.get_or_insert_with(|| r.clone()) != &r {
                            return None;
                        }
                    }
                }
                Op::Store if u.operands[1].unwrap_id_ref() != ptr => {
                    if overlaps(at_path) {
                        let rest = full.strip_prefix(at_path)?;
                        let r =
                            self.trace(at, u.operands[1].unwrap_id_ref(), rest, false, depth + 1)?;
                        if found.get_or_insert_with(|| r.clone()) != &r {
                            return None;
                        }
                    }
                }
                Op::AccessChain | Op::InBoundsAccessChain
                    if u.operands[0].unwrap_id_ref() == ptr =>
                {
                    let mut p = at_path.to_vec();
                    let mut constant = true;
                    for o in &u.operands[1..] {
                        match self.literal(o.unwrap_id_ref()) {
                            Some(l) => p.push(l),
                            None => constant = false,
                        }
                    }
                    let result = u.result_id.unwrap();
                    if !constant {
                        if overlaps(&p)
                            && !self.ptr_read_only(at.body, result, &mut FxHashSet::default())
                        {
                            return None;
                        }
                    } else if overlaps(&p) {
                        self.writes(at, result, &p, full, found, seen, depth)?;
                    }
                }
                Op::FunctionCall => {
                    let callee = u.operands[0].unwrap_id_ref();
                    let ro = u.operands[1..]
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| o.id_ref_any() == Some(ptr))
                        .all(|(k, _)| self.read_only(callee, k));
                    if !ro && overlaps(at_path) {
                        return None;
                    }
                }
                // Held in an aggregate: whatever takes it must only read.
                Op::CompositeInsert | Op::CompositeConstruct | Op::CopyObject | Op::Select => {
                    if overlaps(at_path)
                        && !self.ptr_read_only(
                            at.body,
                            u.result_id.unwrap(),
                            &mut FxHashSet::default(),
                        )
                    {
                        return None;
                    }
                }
                _ if self.is_debuginfo(u) => {}
                _ => return None,
            }
        }
        Some(())
    }

    fn literal(&self, id: Word) -> Option<u32> {
        let c = self
            .types
            .get(&id)
            .filter(|_| self.constants.contains(&id))?;
        match c.operands.first()? {
            Operand::LiteralBit32(v) => Some(*v),
            _ => None,
        }
    }
}

fn legal_param_class(sc: StorageClass) -> bool {
    matches!(
        sc,
        StorageClass::UniformConstant
            | StorageClass::Function
            | StorageClass::Private
            | StorageClass::Workgroup
            | StorageClass::AtomicCounter
    )
}

/// What a call becomes: the clone's key, the instructions computing the
/// values it is passed, and its arguments.
struct Rewrite {
    call: Word,
    key: (Word, Vec<Shape>),
    before: Vec<Instruction>,
    args: Vec<Word>,
}

/// Types, constants and ids made along the way.
struct Made<'h> {
    bound: &'h mut u32,
    types: Vec<Instruction>,
    pointers: FxHashMap<(StorageClass, Word), Word>,
    u32_consts: FxHashMap<u32, Word>,
    fn_types: FxHashMap<Vec<Word>, Word>,
    u32_ty: Word,
}

impl Made<'_> {
    fn id(&mut self) -> Word {
        let id = *self.bound;
        *self.bound += 1;
        id
    }
    fn pointer(&mut self, sc: StorageClass, pointee: Word) -> Word {
        if let Some(&id) = self.pointers.get(&(sc, pointee)) {
            return id;
        }
        let id = self.id();
        let ops = vec![Operand::StorageClass(sc), Operand::IdRef(pointee)];
        self.types
            .push(Instruction::new(Op::TypePointer, None, Some(id), ops));
        self.pointers.insert((sc, pointee), id);
        id
    }
    fn u32(&mut self, v: u32) -> Word {
        if let Some(&id) = self.u32_consts.get(&v) {
            return id;
        }
        let id = self.id();
        let ops = vec![Operand::LiteralBit32(v)];
        self.types.push(Instruction::new(
            Op::Constant,
            Some(self.u32_ty),
            Some(id),
            ops,
        ));
        self.u32_consts.insert(v, id);
        id
    }
    fn fn_type(&mut self, ret_and_params: Vec<Word>) -> Word {
        if let Some(&id) = self.fn_types.get(&ret_and_params) {
            return id;
        }
        let id = self.id();
        let ops = ret_and_params.iter().map(|&t| Operand::IdRef(t)).collect();
        self.types
            .push(Instruction::new(Op::TypeFunction, None, Some(id), ops));
        self.fn_types.insert(ret_and_params, id);
        id
    }
}

/// The functions no rewrite keeps from being inlined: those returning a
/// pointer, or taking one no root global can rebuild (a closure's captures).
pub fn unspecializable(module: &Module) -> FxHashSet<Word> {
    let cx = Cx::new(&module.types_global_values, &module.ext_inst_imports);
    module
        .functions
        .iter()
        .filter(|f| {
            cx.carries(f.def.as_ref().unwrap().result_type.unwrap())
                || f.parameters.iter().any(|p| {
                    let ty = p.result_type.unwrap();
                    cx.kind(ty)
                        .is_some_and(|k| cx.param_leaves(k, ty).is_none())
                })
        })
        .map(|f| f.def_id().unwrap())
        .collect()
}

pub fn specialize_roots(module: &mut Module) {
    let Some(u32_ty) = module
        .types_global_values
        .iter()
        .find(|i| {
            i.class.opcode == Op::TypeInt
                && i.operands[0].unwrap_literal_bit32() == 32
                && i.operands[1].unwrap_literal_bit32() == 0
        })
        .and_then(|i| i.result_id)
    else {
        return;
    };
    let functions = std::mem::take(&mut module.functions);
    let originals = functions.clone();
    let header = module.header.as_mut().unwrap();
    let mut made = Made {
        bound: &mut header.bound,
        types: vec![],
        pointers: FxHashMap::default(),
        u32_consts: FxHashMap::default(),
        fn_types: FxHashMap::default(),
        u32_ty,
    };
    let mut cx = Cx::new(&module.types_global_values, &module.ext_inst_imports);
    for inst in &module.types_global_values {
        let id = inst.result_id.unwrap();
        match inst.class.opcode {
            Op::Constant if inst.result_type == Some(u32_ty) => {
                made.u32_consts
                    .insert(inst.operands[0].unwrap_literal_bit32(), id);
            }
            Op::TypePointer => {
                made.pointers.insert(
                    (
                        inst.operands[0].unwrap_storage_class(),
                        inst.operands[1].unwrap_id_ref(),
                    ),
                    id,
                );
            }
            Op::TypeFunction => {
                made.fn_types.insert(
                    inst.operands.iter().map(|o| o.unwrap_id_ref()).collect(),
                    id,
                );
            }
            _ => {}
        }
    }
    for f in &originals {
        cx.templates.insert(f.def_id().unwrap(), f);
    }

    // The parameters of each function that carry pointers, where every one
    // can be rebuilt from roots.
    let carried: FxHashMap<Word, Vec<Option<(Kind, Vec<Leaf>)>>> = originals
        .iter()
        .filter_map(|f| {
            let mut any = false;
            let mut ps = vec![];
            for p in &f.parameters {
                let ty = p.result_type.unwrap();
                ps.push(match cx.kind(ty) {
                    Some(kind) => {
                        any = true;
                        Some((kind, cx.param_leaves(kind, ty)?))
                    }
                    None => None,
                });
            }
            any.then(|| (f.def_id().unwrap(), ps))
        })
        .collect();

    // Starting from every function no parameter of which carries a pointer,
    // each call to one that does is rewritten to a clone where it can be.
    let mut out: Vec<Function> = vec![];
    let mut queue: VecDeque<Function> = functions
        .into_iter()
        .filter_map(|f| {
            if carried.contains_key(&f.def_id().unwrap()) {
                out.push(f);
                None
            } else {
                Some(f)
            }
        })
        .collect();
    let mut clones: FxHashMap<(Word, Vec<Shape>), Word> = FxHashMap::default();
    let mut names = vec![];
    let mut decorations = vec![];
    while let Some(mut f) = queue.pop_front() {
        let rewrites = {
            let body = Body::of(&f);
            let at = At {
                body: &body,
                caller: None,
            };
            let mut rewrites = vec![];
            for call in f
                .all_inst_iter()
                .filter(|i| i.class.opcode == Op::FunctionCall)
            {
                let callee = call.operands[0].unwrap_id_ref();
                let Some(params) = carried.get(&callee) else {
                    continue;
                };
                rewrites.extend(rewrite(&cx, &mut made, at, call, callee, params));
            }
            rewrites
        };
        for r in rewrites {
            let clone = match clones.get(&r.key) {
                Some(&id) => id,
                None => {
                    let template = cx.templates[&r.key.0];
                    let (g, renamed) =
                        instantiate(&cx, &mut made, template, &carried[&r.key.0], &r.key.1);
                    let id = g.def_id().unwrap();
                    for inst in &module.debug_names {
                        if inst.class.opcode == Op::Name
                            && inst.operands[0].unwrap_id_ref() == r.key.0
                        {
                            let mut n = inst.clone();
                            n.operands[0] = Operand::IdRef(id);
                            names.push(n);
                        }
                    }
                    for inst in &module.annotations {
                        if let Some(Operand::IdRef(t)) = inst.operands.first()
                            && let Some(&new) = renamed.get(t)
                        {
                            let mut d = inst.clone();
                            d.operands[0] = Operand::IdRef(new);
                            decorations.push(d);
                        }
                    }
                    clones.insert(r.key.clone(), id);
                    queue.push_back(g);
                    id
                }
            };
            for block in &mut f.blocks {
                if let Some(at) = block
                    .instructions
                    .iter()
                    .position(|i| i.result_id == Some(r.call))
                {
                    let call = &mut block.instructions[at];
                    call.operands = std::iter::once(clone)
                        .chain(r.args.iter().copied())
                        .map(Operand::IdRef)
                        .collect();
                    block.instructions.splice(at..at, r.before);
                    break;
                }
            }
        }
        out.push(f);
    }
    drop(cx);
    module.types_global_values.extend(made.types);
    module.debug_names.extend(names);
    module.annotations.extend(decorations);
    module.functions = out;
}

/// The rewrite of `call` to a clone of `callee`, where every pointer it
/// passes is traced to its root.
fn rewrite(
    cx: &Cx<'_>,
    made: &mut Made<'_>,
    at: At<'_>,
    call: &Instruction,
    callee: Word,
    params: &[Option<(Kind, Vec<Leaf>)>],
) -> Option<Rewrite> {
    let mut shapes = vec![];
    let mut before = vec![];
    let mut args = vec![];
    for (k, (p, op)) in params.iter().zip(&call.operands[1..]).enumerate() {
        let arg = op.unwrap_id_ref();
        let Some((kind, leaves)) = p else {
            shapes.push(vec![]);
            args.push(arg);
            continue;
        };
        let kind = *kind;
        if kind == Kind::Mem && !cx.read_only(callee, k) {
            return None;
        }
        let mut shape = vec![];
        let mut dyns = vec![];
        let mut values = vec![];
        for leaf in leaves {
            if leaf.ptr {
                let root = match kind {
                    Kind::Ptr | Kind::Value => cx.trace(at, arg, &leaf.path, true, 0)?,
                    Kind::Mem => cx.trace_mem(at, arg, &leaf.path, 0)?,
                };
                let mut keyed = root.clone();
                for i in &mut keyed.indices {
                    if let Index::Dyn(id) = *i {
                        dyns.push(id);
                        *i = Index::Dyn(at.body.defs[&id].result_type?);
                    }
                }
                shape.push(Some(keyed));
            } else {
                let id = made.id();
                match kind {
                    Kind::Ptr => unreachable!(),
                    Kind::Value => {
                        let ops = std::iter::once(Operand::IdRef(arg))
                            .chain(leaf.path.iter().map(|&l| Operand::LiteralBit32(l)))
                            .collect();
                        before.push(Instruction::new(
                            Op::CompositeExtract,
                            Some(leaf.ty),
                            Some(id),
                            ops,
                        ));
                    }
                    Kind::Mem => {
                        let ptr = if leaf.path.is_empty() {
                            arg
                        } else {
                            let ptr_ty = made.pointer(StorageClass::Function, leaf.ty);
                            let chain = made.id();
                            let ops = std::iter::once(arg)
                                .chain(leaf.path.iter().map(|&l| made.u32(l)))
                                .map(Operand::IdRef)
                                .collect();
                            before.push(Instruction::new(
                                Op::InBoundsAccessChain,
                                Some(ptr_ty),
                                Some(chain),
                                ops,
                            ));
                            chain
                        };
                        before.push(Instruction::new(
                            Op::Load,
                            Some(leaf.ty),
                            Some(id),
                            vec![Operand::IdRef(ptr)],
                        ));
                    }
                }
                values.push(id);
                shape.push(None);
            }
        }
        args.extend(values);
        args.extend(dyns);
        shapes.push(shape);
    }
    Some(Rewrite {
        call: call.result_id.unwrap(),
        key: (callee, shapes),
        before,
        args,
    })
}

/// A clone of `template` with its carried parameters rebuilt from `shapes`,
/// and the template's ids (but its carried parameters) renamed to the clone's.
fn instantiate(
    cx: &Cx<'_>,
    made: &mut Made<'_>,
    template: &Function,
    params: &[Option<(Kind, Vec<Leaf>)>],
    shapes: &[Shape],
) -> (Function, FxHashMap<Word, Word>) {
    let mut renamed: FxHashMap<Word, Word> = FxHashMap::default();
    let mut fresh: FxHashMap<Word, Word> = FxHashMap::default();
    let mut new_params = vec![];
    let mut entry_vars = vec![];
    let mut entry = vec![];
    for ((p, info), shape) in template.parameters.iter().zip(params).zip(shapes) {
        let old = p.result_id.unwrap();
        let ty = p.result_type.unwrap();
        let Some((kind, leaves)) = info else {
            let id = made.id();
            renamed.insert(old, id);
            fresh.insert(old, id);
            new_params.push(Instruction::new(
                Op::FunctionParameter,
                Some(ty),
                Some(id),
                vec![],
            ));
            continue;
        };
        let mut values = FxHashMap::default();
        for leaf in leaves.iter().filter(|l| !l.ptr) {
            let id = made.id();
            new_params.push(Instruction::new(
                Op::FunctionParameter,
                Some(leaf.ty),
                Some(id),
                vec![],
            ));
            values.insert(leaf.path.clone(), id);
        }
        for (leaf, root) in leaves.iter().zip(shape) {
            let Some(root) = root else { continue };
            let mut ops = vec![Operand::IdRef(root.var)];
            for i in &root.indices {
                ops.push(Operand::IdRef(match *i {
                    Index::Const(c) => c,
                    Index::Dyn(t) => {
                        let id = made.id();
                        new_params.push(Instruction::new(
                            Op::FunctionParameter,
                            Some(t),
                            Some(id),
                            vec![],
                        ));
                        id
                    }
                }));
            }
            let id = if root.indices.is_empty() {
                root.var
            } else {
                let id = made.id();
                entry.push(Instruction::new(
                    Op::AccessChain,
                    Some(leaf.ty),
                    Some(id),
                    ops,
                ));
                id
            };
            values.insert(leaf.path.clone(), id);
        }
        let whole = match kind {
            Kind::Ptr => values[&vec![]],
            Kind::Value => construct(cx, made, ty, &mut vec![], &values, &mut entry),
            Kind::Mem => {
                let pointee = cx.pointer(ty).unwrap().1;
                let value = construct(cx, made, pointee, &mut vec![], &values, &mut entry);
                let var = made.id();
                let ops = vec![Operand::StorageClass(StorageClass::Function)];
                entry_vars.push(Instruction::new(Op::Variable, Some(ty), Some(var), ops));
                entry.push(Instruction::new(
                    Op::Store,
                    None,
                    None,
                    vec![Operand::IdRef(var), Operand::IdRef(value)],
                ));
                var
            }
        };
        renamed.insert(old, whole);
    }
    let mut blocks: Vec<Block> = template.blocks.clone();
    for inst in blocks
        .iter()
        .flat_map(|b| b.label.iter().chain(&b.instructions))
    {
        if let Some(id) = inst.result_id {
            let new = made.id();
            renamed.insert(id, new);
            fresh.insert(id, new);
        }
    }
    super::apply_rewrite_rules(&renamed, &mut blocks);
    // After the entry block's variables (and the debuginfo among them).
    let first = &mut blocks[0].instructions;
    let vars = first
        .iter()
        .take_while(|i| i.class.opcode == Op::Variable || cx.is_debuginfo(i))
        .count();
    first.splice(vars..vars, entry);
    first.splice(0..0, entry_vars);

    let mut def = template.def.clone().unwrap();
    let id = made.id();
    fresh.insert(template.def_id().unwrap(), id);
    def.result_id = Some(id);
    let ret = def.result_type.unwrap();
    let fn_ty = made.fn_type(
        std::iter::once(ret)
            .chain(new_params.iter().map(|p| p.result_type.unwrap()))
            .collect(),
    );
    let control = def.operands[0].unwrap_function_control();
    def.operands = vec![Operand::FunctionControl(control), Operand::IdRef(fn_ty)];
    (
        Function {
            def: Some(def),
            end: template.end.clone(),
            parameters: new_params,
            blocks,
        },
        fresh,
    )
}

/// The value of type `ty` at `path` built of `values`' leaves.
fn construct(
    cx: &Cx<'_>,
    made: &mut Made<'_>,
    ty: Word,
    path: &mut Vec<u32>,
    values: &FxHashMap<Vec<u32>, Word>,
    out: &mut Vec<Instruction>,
) -> Word {
    if let Some(&v) = values.get(path) {
        return v;
    }
    let members = cx.members(ty).unwrap();
    let mut parts = vec![];
    for (i, m) in members.into_iter().enumerate() {
        path.push(i as u32);
        parts.push(Operand::IdRef(construct(cx, made, m, path, values, out)));
        path.pop();
    }
    let id = made.id();
    out.push(Instruction::new(
        Op::CompositeConstruct,
        Some(ty),
        Some(id),
        parts,
    ));
    id
}

/// Passes a copy where a call's `Function` pointer argument is no variable
/// or parameter (a field's address, which Logical addressing does not allow
/// as an argument): a local of the pointee's type, filled before the call and,
/// where the callee may write through it, copied back after.
pub fn copy_pointer_arguments(module: &mut Module) {
    let mut cx = Cx::new(&module.types_global_values, &module.ext_inst_imports);
    let originals = module.functions.clone();
    for f in &originals {
        cx.templates.insert(f.def_id().unwrap(), f);
    }
    let params: FxHashMap<Word, Vec<Word>> = originals
        .iter()
        .map(|f| {
            (
                f.def_id().unwrap(),
                f.parameters
                    .iter()
                    .map(|p| p.result_type.unwrap())
                    .collect(),
            )
        })
        .collect();
    let header = module.header.as_mut().unwrap();
    let mut next = || {
        let id = header.bound;
        header.bound += 1;
        id
    };
    for f in &mut module.functions {
        let locals: FxHashSet<Word> = f
            .parameters
            .iter()
            .chain(
                f.blocks[0]
                    .instructions
                    .iter()
                    .filter(|i| i.class.opcode == Op::Variable),
            )
            .map(|i| i.result_id.unwrap())
            .collect();
        let chains: FxHashMap<Word, (Word, Vec<Operand>)> = f
            .all_inst_iter()
            .filter(|i| matches!(i.class.opcode, Op::AccessChain | Op::InBoundsAccessChain))
            .map(|i| {
                (
                    i.result_id.unwrap(),
                    (i.operands[0].unwrap_id_ref(), i.operands[1..].to_vec()),
                )
            })
            .collect();
        // The address `arg` names, as one access chain from a variable or
        // parameter, made again where it is used: a pointer is no value WGSL
        // carries across blocks.
        let whole = |mut arg: Word| {
            let mut indices = vec![];
            while let Some((base, more)) = chains.get(&arg) {
                indices.splice(0..0, more.iter().cloned());
                arg = *base;
            }
            locals.contains(&arg).then_some((arg, indices))
        };
        let mut vars = vec![];
        for block in &mut f.blocks {
            let mut i = 0;
            while i < block.instructions.len() {
                let inst = &block.instructions[i];
                if inst.class.opcode != Op::FunctionCall {
                    i += 1;
                    continue;
                }
                let callee = inst.operands[0].unwrap_id_ref();
                let Some(tys) = params.get(&callee) else {
                    i += 1;
                    continue;
                };
                let (mut before, mut after) = (vec![], vec![]);
                let mut call = inst.clone();
                for (k, &ty) in tys.iter().enumerate() {
                    let arg = call.operands[k + 1].unwrap_id_ref();
                    let Some((StorageClass::Function, pointee)) = cx.pointer(ty) else {
                        continue;
                    };
                    if locals.contains(&arg) || cx.carries(pointee) {
                        continue;
                    }
                    let Some((base, indices)) = whole(arg) else {
                        continue;
                    };
                    let chain = |out: &mut Vec<Instruction>, id: Word| {
                        let ops = std::iter::once(Operand::IdRef(base))
                            .chain(indices.iter().cloned())
                            .collect();
                        out.push(Instruction::new(Op::AccessChain, Some(ty), Some(id), ops));
                        id
                    };
                    let tmp = next();
                    vars.push(Instruction::new(
                        Op::Variable,
                        Some(ty),
                        Some(tmp),
                        vec![Operand::StorageClass(StorageClass::Function)],
                    ));
                    let at = chain(&mut before, next());
                    let v = next();
                    before.push(Instruction::new(
                        Op::Load,
                        Some(pointee),
                        Some(v),
                        vec![Operand::IdRef(at)],
                    ));
                    before.push(Instruction::new(
                        Op::Store,
                        None,
                        None,
                        vec![Operand::IdRef(tmp), Operand::IdRef(v)],
                    ));
                    if !cx.read_only(callee, k) {
                        let w = next();
                        after.push(Instruction::new(
                            Op::Load,
                            Some(pointee),
                            Some(w),
                            vec![Operand::IdRef(tmp)],
                        ));
                        let at = chain(&mut after, next());
                        after.push(Instruction::new(
                            Op::Store,
                            None,
                            None,
                            vec![Operand::IdRef(at), Operand::IdRef(w)],
                        ));
                    }
                    call.operands[k + 1] = Operand::IdRef(tmp);
                }
                let (nb, na) = (before.len(), after.len());
                block.instructions[i] = call;
                block.instructions.splice(i + 1..i + 1, after);
                block.instructions.splice(i..i, before);
                i += nb + 1 + na;
            }
        }
        f.blocks[0].instructions.splice(0..0, vars);
    }
}
