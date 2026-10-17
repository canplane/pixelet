// gpu-query-builder/main.rs

//! Builds the shader crate to SPIR-V and reads each entry into WGSL with
//! naga, printing where it lands or the compiler's own error: each entry a
//! module of its own, as a pipeline takes its module's one entry.
//!
//! The reading is ours, not the codegen's, for the one meaning SPIR-V has no
//! word for: WGSL's `workgroupUniformLoad`. The shader crate says it as the
//! barriers about a load of the workgroup's memory (`workgroup_uniform_load`),
//! and `uniform_loads` reads exactly that triple back into the statement, so
//! WebGPU sees a value every lane shares where the shader means one.

use naga::proc::TypeResolution;
use naga::valid::{Capabilities, FunctionInfo, ModuleInfo, ValidationFlags, Validator};
use naga::{AddressSpace, Arena, Barrier, Block, Expression, Function, GlobalVariable, Handle, Module, Range, Span, Statement, Type, UniqueArena};
use spirv_builder::{Capability, ModuleResult, SpirvBuilder};

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/../shader");
    // WebGPU's own types alone, and a texture's size asked of it (WGSL
    // `textureDimensions`): the rays are as large as the surface they lie on.
    // Vulkan 1.3 is the SPIR-V rust-gpu emits for its naga target.
    let result = SpirvBuilder::new(shader, "spirv-unknown-vulkan1.3").multimodule(true).capability(Capability::ImageQuery).build();
    let modules = match result.map(|r| r.module) {
        Ok(ModuleResult::MultiModule(m)) => m,
        Ok(ModuleResult::SingleModule(_)) => unreachable!("built as multimodule"),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    };
    for (entry, spv) in &modules {
        let wgsl = wgsl(&std::fs::read(spv).expect("spv")).unwrap_or_else(|e| {
            eprintln!("failed: {entry}: {e}");
            std::process::exit(1);
        });
        std::fs::write(spv.with_extension("wgsl"), wgsl).expect("wgsl");
    }
    println!("built: {modules:?}");
}

/// One entry's SPIR-V as WGSL, validated as WebGPU takes it.
fn wgsl(spv: &[u8]) -> Result<String, String> {
    let mut module = naga::front::spv::parse_u8_slice(spv, &naga::front::spv::Options::default()).map_err(|e| e.to_string())?;
    let validate = |m: &Module| Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(m).map_err(|e| format!("{e:?}"));
    let info = validate(&module)?;
    uniform_loads(&mut module, &info);
    let info = validate(&module)?;
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty()).map_err(|e| e.to_string())
}

/// Every `ControlBarrier(WORK_GROUP)`, load of a workgroup pointer,
/// `ControlBarrier(WORK_GROUP)` in a block, with nothing between but that
/// pointer's own access chain, becomes `WorkGroupUniformLoad`: the same
/// barriers about the same load, said as one. A triple that does not match
/// fully is left as it is.
fn uniform_loads(module: &mut Module, info: &ModuleInfo) {
    let globals = &module.global_variables;
    for (h, f) in module.functions.iter_mut() {
        let loads = rewrite(f, globals);
        retype(&mut module.types, f, &info[h], loads);
    }
    for (i, e) in module.entry_points.iter_mut().enumerate() {
        let loads = rewrite(&mut e.function, globals);
        retype(&mut module.types, &mut e.function, info.get_entry_point(i), loads);
    }
}

/// Each taken load's value is the statement's result now, of the load's type.
fn retype(types: &mut UniqueArena<Type>, f: &mut Function, fi: &FunctionInfo, loads: Vec<Handle<Expression>>) {
    for load in loads {
        let ty = match &fi[load].ty {
            TypeResolution::Handle(t) => *t,
            TypeResolution::Value(inner) => types.insert(Type { name: None, inner: inner.clone() }, Span::UNDEFINED),
        };
        f.expressions[load] = Expression::WorkGroupUniformLoadResult { ty };
    }
}

/// `uniform_loads`' rewrite of one function's statements; the loads it took.
fn rewrite(f: &mut Function, globals: &Arena<GlobalVariable>) -> Vec<Handle<Expression>> {
    let mut loads = Vec::new();
    let mut body = std::mem::take(&mut f.body);
    block(&mut body, f, globals, &mut loads);
    f.body = body;
    loads
}

fn block(b: &mut Block, f: &Function, globals: &Arena<GlobalVariable>, loads: &mut Vec<Handle<Expression>>) {
    let s: Vec<(Statement, Span)> = std::mem::take(b)
        .span_into_iter()
        .map(|(mut st, span)| {
            match &mut st {
                Statement::Block(inner) => block(inner, f, globals, loads),
                Statement::If { accept, reject, .. } => {
                    block(accept, f, globals, loads);
                    block(reject, f, globals, loads);
                }
                Statement::Loop { body, continuing, .. } => {
                    block(body, f, globals, loads);
                    block(continuing, f, globals, loads);
                }
                Statement::Switch { cases, .. } => cases.iter_mut().for_each(|c| block(&mut c.body, f, globals, loads)),
                _ => {}
            }
            (st, span)
        })
        .collect();
    let barrier = |k: usize| matches!(s[k].0, Statement::ControlBarrier(b) if b == Barrier::WORK_GROUP);
    let mut i = 0;
    while i < s.len() {
        let close = (i + 1..s.len()).find(|&k| !matches!(s[k].0, Statement::Emit(_)));
        if let Some(k) = close.filter(|&k| barrier(i) && barrier(k)) {
            if let Some((chain, load, pointer)) = load_between(&s[i + 1..k], f, globals) {
                for h in chain {
                    b.push(Statement::Emit(Range::new_from_bounds(h, h)), s[i].1);
                }
                b.push(Statement::WorkGroupUniformLoad { pointer, result: load }, s[i].1);
                loads.push(load);
                i = k + 1;
                continue;
            }
        }
        b.push(s[i].0.clone(), s[i].1);
        i += 1;
    }
}

/// What the emits between two barriers hold, when that is one load of a
/// workgroup pointer and the links of its access chain alone: those links
/// in order, the load, and its pointer.
fn load_between(emits: &[(Statement, Span)], f: &Function, globals: &Arena<GlobalVariable>) -> Option<(Vec<Handle<Expression>>, Handle<Expression>, Handle<Expression>)> {
    let mut held = Vec::new();
    for (st, _) in emits {
        let Statement::Emit(r) = st else { return None };
        held.extend(r.clone());
    }
    let mut loads = held.iter().filter(|h| matches!(f.expressions[**h], Expression::Load { .. }));
    let (Some(&load), None) = (loads.next(), loads.next()) else { return None };
    let Expression::Load { pointer } = f.expressions[load] else { return None };
    let mut chain = Vec::new();
    let mut at = pointer;
    loop {
        match f.expressions[at] {
            Expression::GlobalVariable(g) if globals[g].space == AddressSpace::WorkGroup => break,
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => {
                if held.contains(&at) {
                    chain.push(at);
                }
                at = base;
            }
            _ => return None,
        }
    }
    chain.reverse();
    (chain.len() + 1 == held.len()).then_some((chain, load, pointer))
}
