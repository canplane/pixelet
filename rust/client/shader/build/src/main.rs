// pxl-shader-build/main.rs

//! Builds the shader crate to SPIR-V and reads each entry into WGSL with
//! naga, printing where it lands or the compiler's own error: each entry a
//! module of its own, as a pipeline takes its module's one entry.
//!
//! The reading is ours, not the codegen's, for the one meaning SPIR-V has no
//! word for: WGSL's `workgroupUniformLoad`. The shader crate says it as the
//! barriers about a load of the workgroup's memory (way.rs `load_shared`),
//! and `uniform_loads` reads exactly that triple back into the statement, so
//! WebGPU sees a value every lane shares where the shader means one.
//!
//! So is spirv-opt. The codegen runs it with every function inlined into
//! its entry (SPIRV-Tools' performance passes open with `InlineExhaustive`),
//! and every entry here is read so, no function of it called (`wgsl_of`):
//! Adreno's driver refuses modules whose functions hand an array or a struct
//! across a call (doc/experiment/20261103-shaders-on-two-phones), and what
//! naga writes is rewritten for the same driver (rewrite.rs).
//! A function a binding's pointer reaches is the codegen's clone of it for
//! that binding, the pointer made again inside from the binding itself
//! (rustc_codegen_spirv linker/specialize_roots.rs, vendored beside), or,
//! where the codegen cannot tell which binding, inlined to legalize it
//! (linker/inline.rs).

use naga::proc::TypeResolution;
use naga::valid::{Capabilities, FunctionInfo, ModuleInfo, ValidationFlags, Validator};
use naga::{
    AddressSpace, Arena, Barrier, Block, Expression, Function, GlobalVariable, Handle, Module,
    Range, Span, Statement, Type, UniqueArena,
};
use spirv_builder::{Capability, ModuleResult, ShaderPanicStrategy, SpirvBuilder};

mod rewrite;

fn main() {
    let shader = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    // WebGPU's own types alone, and a texture's size asked of it (WGSL
    // `textureDimensions`): the rays are as large as the surface they lie on.
    // Vulkan 1.3 is the SPIR-V rust-gpu emits for its naga target. A check
    // that fails (a bound, an overflow) is unreachable, not an early return:
    // pxl-engine's rule is that nothing aborts, so the branch is dead by
    // contract and spirv-opt drops it; where one is reached after all, the
    // invocation runs on with WGSL's defined behaviour for the access.
    let result = SpirvBuilder::new(shader, "spirv-unknown-vulkan1.3")
        .multimodule(true)
        .shader_panic_strategy(ShaderPanicStrategy::UNSOUND_DO_NOT_USE_UndefinedBehaviorViaUnreachable)
        .capability(Capability::ImageQuery)
        .extra_arg("--no-spirv-opt")
        .build();
    let modules = match result.map(|r| r.module) {
        Ok(ModuleResult::MultiModule(m)) => m,
        Ok(ModuleResult::SingleModule(_)) => unreachable!("built as multimodule"),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    };
    for (entry, spv) in &modules {
        let wgsl = wgsl_of(&std::fs::read(spv).expect("spv")).unwrap_or_else(|e| {
            eprintln!("failed: {entry}: {e}");
            std::process::exit(1);
        });
        std::fs::write(spv.with_extension("wgsl"), wgsl).expect("wgsl");
    }
    println!("built: {modules:?}");
}

/// One entry's WGSL: its SPIR-V optimized, every function inlined, as the
/// codegen optimizes.
fn wgsl_of(spv: &[u8]) -> Result<String, String> {
    let words: Vec<u32> = spv
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    wgsl(&optimized(words)?)
}

/// One entry's SPIR-V through spirv-opt as the codegen runs it, its
/// performance passes and then its dead constants and debug info dropped
/// (rustc_codegen_spirv link.rs `do_spirv_opt`), its ids numbered anew: the
/// codegen numbers them across the crate and naga names values after them,
/// so an edit to one entry would rename values in every other.
fn optimized(words: Vec<u32>) -> Result<Vec<u32>, String> {
    use spirv_tools::opt::{self, Optimizer, Passes};
    let mut optimizer = opt::create(Some(spirv_tools::TargetEnv::Vulkan_1_3));
    optimizer
        .register_performance_passes()
        .register_pass(Passes::EliminateDeadConstant)
        .register_pass(Passes::StripDebugInfo)
        .register_pass(Passes::CompactIds);
    let mut told = String::new();
    let options = opt::Options {
        validator_options: Some(spirv_tools::val::ValidatorOptions::default()),
        ..Default::default()
    };
    let out = optimizer.optimize(
        words,
        &mut |m: spirv_tools::error::Message| told.push_str(&format!("{}\n", m.message)),
        Some(options),
    );
    match out {
        Ok(out) => Ok(out.as_words().to_vec()),
        Err(e) => Err(format!("spirv-opt: {e}: {told}")),
    }
}

/// One entry's SPIR-V as WGSL, validated as WebGPU takes it.
fn wgsl(spv: &[u32]) -> Result<String, String> {
    let spv = pointers_where_used(spv);
    let mut module = naga::front::spv::parse_u8_slice(
        &spv.iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<u8>>(),
        &naga::front::spv::Options::default(),
    )
    .map_err(|e| e.to_string())?;
    let validate = |m: &Module| {
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(m)
            .map_err(|e| format!("{e:?}"))
    };
    let info = validate(&module)?;
    uniform_loads(&mut module, &info);
    let info = validate(&module)?;
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map(|text| {
            rewrite::small_arrays_as_vectors(&rewrite::split_phis(&i32_min_called(&text)))
        })
        .map_err(|e| e.to_string())
}

/// What naga writes for `i32::MIN`, and the function it is called through.
const I32_MIN: &str = "i32(-2147483648)";
const I32_MIN_FN: &str = "i32_min";

/// `text` with each `i32::MIN` a call of a function that returns it.
/// WebKit writes the constant into Metal as the bare `-2147483648`, which
/// C++ reads as a `long`, so a `select` of it and an `int` names no one
/// overload and the module fails to compile on Safari; a call is no
/// constant, and its value is the function's `int`.
fn i32_min_called(text: &str) -> String {
    if !text.contains(I32_MIN) {
        return text.to_owned();
    }
    assert!(
        !text.contains(&format!("fn {I32_MIN_FN}(")),
        "naga named a function {I32_MIN_FN}"
    );
    format!(
        "{}\nfn {I32_MIN_FN}() -> i32 {{\n    return -2147483647i - 1i;\n}}\n",
        text.replace(I32_MIN, &format!("{I32_MIN_FN}()"))
    )
}

/// The module's words with each access chain a pointer is loaded, stored,
/// copied, chained or passed through made again in the block that uses it,
/// where spirv-opt left it in another: naga holds a value used outside the
/// scope that made it in a local variable, and no local holds a pointer.
fn pointers_where_used(spv: &[u32]) -> Vec<u32> {
    // SPIR-V's opcodes (the SPIR-V specification, 3.52).
    const TYPE_POINTER: u32 = 32;
    const FUNCTION: u32 = 54;
    const FUNCTION_END: u32 = 56;
    const FUNCTION_CALL: u32 = 57;
    const LOAD: u32 = 61;
    const STORE: u32 = 62;
    const COPY_MEMORY: u32 = 63;
    const ACCESS_CHAIN: u32 = 65;
    const IN_BOUNDS_ACCESS_CHAIN: u32 = 66;
    const LABEL: u32 = 248;
    let mut bound = spv[3];
    let mut pointers = std::collections::HashSet::new();
    let mut out = spv[..5].to_vec();
    // Per function: each chain's words and block; then each instruction
    // with the chains it names from another block made before it.
    let mut i = 5;
    let mut chains = std::collections::HashMap::<u32, (usize, Vec<u32>)>::new();
    let mut block = 0;
    let mut made = std::collections::HashMap::<(u32, usize), u32>::new();
    while i < spv.len() {
        let n = (spv[i] >> 16) as usize;
        let inst = &spv[i..i + n];
        let op = inst[0] & 0xffff;
        match op {
            TYPE_POINTER => {
                pointers.insert(inst[1]);
            }
            FUNCTION | FUNCTION_END => {
                chains.clear();
                made.clear();
            }
            LABEL => block += 1,
            _ => {}
        }
        let uses: &[usize] = match op {
            LOAD | ACCESS_CHAIN | IN_BOUNDS_ACCESS_CHAIN => &[3],
            STORE => &[1],
            COPY_MEMORY => &[1, 2],
            _ => &[],
        };
        let mut inst = inst.to_vec();
        let args = if op == FUNCTION_CALL { 4..n } else { 0..0 };
        for k in uses.iter().copied().chain(args) {
            inst[k] = in_block(inst[k], block, &chains, &mut made, &mut bound, &mut out);
        }
        if matches!(op, ACCESS_CHAIN | IN_BOUNDS_ACCESS_CHAIN) && pointers.contains(&inst[1]) {
            chains.insert(inst[2], (block, inst.clone()));
        }
        out.extend(inst);
        i += n.max(1);
    }
    out[3] = bound;
    out
}

/// `id`, or where it is a chain made in another block, a chain made again
/// at the end of `out` in `block` (once a block).
fn in_block(
    id: u32,
    block: usize,
    chains: &std::collections::HashMap<u32, (usize, Vec<u32>)>,
    made: &mut std::collections::HashMap<(u32, usize), u32>,
    bound: &mut u32,
    out: &mut Vec<u32>,
) -> u32 {
    let Some((at, words)) = chains.get(&id) else { return id };
    if *at == block {
        return id;
    }
    if let Some(&again) = made.get(&(id, block)) {
        return again;
    }
    let mut words = words.clone();
    words[3] = in_block(words[3], block, chains, made, bound, out);
    words[2] = *bound;
    *bound += 1;
    made.insert((id, block), words[2]);
    out.extend(&words);
    words[2]
}

/// Every `ControlBarrier(WORK_GROUP)`, load of a workgroup pointer,
/// `ControlBarrier(WORK_GROUP)` in a block, nothing else between reading
/// memory (`load_between`), becomes `WorkGroupUniformLoad`: the same
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
        retype(
            &mut module.types,
            &mut e.function,
            info.get_entry_point(i),
            loads,
        );
    }
}

/// Each taken load's value is the statement's result now, of the load's type.
fn retype(
    types: &mut UniqueArena<Type>,
    f: &mut Function,
    fi: &FunctionInfo,
    loads: Vec<Handle<Expression>>,
) {
    for load in loads {
        let ty = match &fi[load].ty {
            TypeResolution::Handle(t) => *t,
            TypeResolution::Value(inner) => types.insert(
                Type {
                    name: None,
                    inner: inner.clone(),
                },
                Span::UNDEFINED,
            ),
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

fn block(
    b: &mut Block,
    f: &Function,
    globals: &Arena<GlobalVariable>,
    loads: &mut Vec<Handle<Expression>>,
) {
    let s: Vec<(Statement, Span)> = std::mem::take(b)
        .span_into_iter()
        .map(|(mut st, span)| {
            match &mut st {
                Statement::Block(inner) => block(inner, f, globals, loads),
                Statement::If { accept, reject, .. } => {
                    block(accept, f, globals, loads);
                    block(reject, f, globals, loads);
                }
                Statement::Loop {
                    body, continuing, ..
                } => {
                    block(body, f, globals, loads);
                    block(continuing, f, globals, loads);
                }
                Statement::Switch { cases, .. } => cases
                    .iter_mut()
                    .for_each(|c| block(&mut c.body, f, globals, loads)),
                _ => {}
            }
            (st, span)
        })
        .collect();
    let barrier =
        |k: usize| matches!(s[k].0, Statement::ControlBarrier(b) if b == Barrier::WORK_GROUP);
    let mut i = 0;
    while i < s.len() {
        let close = (i + 1..s.len()).find(|&k| !matches!(s[k].0, Statement::Emit(_)));
        if let Some(k) = close.filter(|&k| barrier(i) && barrier(k)) {
            if let Some(t) = load_between(&s[i + 1..k], f, globals) {
                let emit = |b: &mut Block, hs: &[Handle<Expression>]| {
                    hs.iter().for_each(|&h| {
                        b.push(Statement::Emit(Range::new_from_bounds(h, h)), s[i].1)
                    })
                };
                emit(b, &t.before);
                b.push(
                    Statement::WorkGroupUniformLoad {
                        pointer: t.pointer,
                        result: t.load,
                    },
                    s[i].1,
                );
                emit(b, &t.after);
                loads.push(t.load);
                i = k + 1;
                continue;
            }
        }
        b.push(s[i].0.clone(), s[i].1);
        i += 1;
    }
}

/// What the emits between two barriers hold, when that is one load of a
/// workgroup pointer, all else pure: what was made before the load, which
/// its pointer is among, then the load, then what was made after it, which
/// reads its value. An expression is made after those it reads, so each
/// side keeps what it needs; a read of memory besides the load, which the
/// load's own barriers would move, matches nothing.
fn load_between(
    emits: &[(Statement, Span)],
    f: &Function,
    globals: &Arena<GlobalVariable>,
) -> Option<Triple> {
    let mut held = Vec::new();
    for (st, _) in emits {
        let Statement::Emit(r) = st else { return None };
        held.extend(r.clone());
    }
    let mut loads = held
        .iter()
        .filter(|h| matches!(f.expressions[**h], Expression::Load { .. }));
    let (Some(&load), None) = (loads.next(), loads.next()) else {
        return None;
    };
    let Expression::Load { pointer } = f.expressions[load] else {
        return None;
    };
    let mut at = pointer;
    loop {
        match f.expressions[at] {
            Expression::GlobalVariable(g) if globals[g].space == AddressSpace::WorkGroup => break,
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => at = base,
            _ => return None,
        }
    }
    let pure = |h: &Handle<Expression>| {
        *h == load
            || matches!(
                f.expressions[*h],
                Expression::Access { .. }
                    | Expression::AccessIndex { .. }
                    | Expression::Splat { .. }
                    | Expression::Swizzle { .. }
                    | Expression::Compose { .. }
                    | Expression::Unary { .. }
                    | Expression::Binary { .. }
                    | Expression::Select { .. }
                    | Expression::Relational { .. }
                    | Expression::Math { .. }
                    | Expression::As { .. }
            )
    };
    held.sort();
    held.iter().all(pure).then(|| Triple {
        before: held.iter().copied().filter(|h| *h < load).collect(),
        load,
        pointer,
        after: held.iter().copied().filter(|h| *h > load).collect(),
    })
}

/// A triple `load_between` takes.
struct Triple {
    before: Vec<Handle<Expression>>,
    load: Handle<Expression>,
    pointer: Handle<Expression>,
    after: Vec<Handle<Expression>>,
}
