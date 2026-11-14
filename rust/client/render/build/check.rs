// pxl-render/build/check.rs

//! What naga's validation does not hold a module to: what the phones' drivers and the browsers
//! refuse, and the structs a shader lays over words the Rust lays out.

use crate::word;
use pxl_engine::{light, ray};

/// The structs a shader declares over words pxl-engine or word.rs lays out, each with the word
/// each of its first members begins at and its size in words: the raster's view (raster.wgsl
/// `Raster`, ray/raster.rs) and the lists' argument structs (visible.wgsl `MissCount`,
/// light_receiver_tile.wgsl `ReceiverDispatch`). Some shader declares each.
pub(crate) const LAYOUTS: [(&str, &[u32], usize); 3] = [
    ("Raster", &[], word::RASTER_FLOATS),
    (
        "MissCount",
        &[ray::MISS_ARGS_CNT, ray::MISS_ARGS_CNTS],
        ray::MISS_ARGS_WORDS,
    ),
    (
        "ReceiverDispatch",
        &[
            light::RECEIVER_TILES_LIST,
            light::RECEIVER_TILES_CNT,
            light::RECEIVER_TILES_SLIDE,
        ],
        light::RECEIVER_TILES_WORDS as usize,
    ),
];

/// The structs of `LAYOUTS` that `module` declares, each asserted to lie as the Rust lays it.
pub(crate) fn layouts(module: &naga::Module, name: &str) -> Vec<&'static str> {
    let mut met = Vec::new();
    for (ty, offs, words) in LAYOUTS {
        let Some((_, t)) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(ty))
        else {
            continue;
        };
        let naga::TypeInner::Struct { ref members, span } = t.inner else {
            panic!("{name}: `{ty}` is a struct")
        };
        let got: Vec<u32> = members.iter().map(|m| m.offset / 4).collect();
        assert_eq!(
            &got[..offs.len()],
            offs,
            "{name}: `{ty}`'s words against the Rust's"
        );
        assert_eq!(
            span as usize,
            4 * words,
            "{name}: `{ty}`'s size against the Rust's"
        );
        met.push(ty);
    }
    met
}

/// Refuses, in every assembled module, compute and render, what the Galaxy S23's driver or
/// Safari fails to compile. The S23's Vulkan driver refuses a module whose struct holds a struct,
/// directly or in an array, and one where a function's local is a struct or is or holds an array.
/// The one struct local allowed is a vertex or fragment entry point's local of the type it
/// returns. Safari writes `i32::MIN`, written or folded, as a C++ `long` that no `select` with an
/// `int` takes. Safari 27 writes the result of `atomicCompareExchangeWeak` as a struct with a
/// `thread`-qualified field, which Metal refuses.
pub(crate) fn refuse_faults(module: &naga::Module, name: &str) {
    use naga::TypeInner::{Array, BindingArray, Struct};
    let is_struct = |mut ty: naga::Handle<naga::Type>| loop {
        match module.types[ty].inner {
            Struct { .. } => return true,
            Array { base, .. } | BindingArray { base, .. } => ty = base,
            _ => return false,
        }
    };
    fn holds_array(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
        match &module.types[ty].inner {
            Array { .. } | BindingArray { .. } => true,
            Struct { members, .. } => members.iter().any(|m| holds_array(module, m.ty)),
            _ => false,
        }
    }
    let mut faults = Vec::new();
    for (_, ty) in module.types.iter() {
        let Struct { members, .. } = &ty.inner else {
            continue;
        };
        for m in members {
            if is_struct(m.ty) {
                faults.push(format!(
                    "struct {} holds a struct in {}",
                    ty.name.as_deref().unwrap_or("?"),
                    m.name.as_deref().unwrap_or("?")
                ));
            }
        }
    }
    // Every function, each entry point with the type it returns where it is a vertex or a
    // fragment entry point.
    let fns = module.functions.iter().map(|(_, f)| (f, None));
    let entries = module.entry_points.iter().map(|e| {
        let output = matches!(
            e.stage,
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment
        )
        .then(|| e.function.result.as_ref().map(|r| r.ty))
        .flatten();
        (&e.function, output)
    });
    let i32_min =
        |e: &naga::Expression| matches!(e, naga::Expression::Literal(naga::Literal::I32(i32::MIN)));
    let compares = |e: &naga::Expression| {
        matches!(
            e,
            naga::Expression::AtomicResult {
                comparison: true,
                ..
            }
        )
    };
    let mut min_cnt = module
        .global_expressions
        .iter()
        .filter(|(_, e)| i32_min(e))
        .count();
    let mut compared = 0;
    for (f, output) in fns.chain(entries) {
        for (_, var) in f.local_variables.iter() {
            let ty = &module.types[var.ty];
            let local = || {
                format!(
                    "{}'s var {}: {}",
                    f.name.as_deref().unwrap_or("?"),
                    var.name.as_deref().unwrap_or("?"),
                    ty.name.as_deref().unwrap_or("array")
                )
            };
            if matches!(ty.inner, Struct { .. }) && output != Some(var.ty) {
                faults.push(format!("{} is a struct", local()));
            }
            if holds_array(module, var.ty) {
                faults.push(format!("{} holds an array", local()));
            }
        }
        min_cnt += f.expressions.iter().filter(|(_, e)| i32_min(e)).count();
        compared += f.expressions.iter().filter(|(_, e)| compares(e)).count();
    }
    if min_cnt > 0 {
        faults.push(format!("i32::MIN held {min_cnt} times"));
    }
    if compared > 0 {
        faults.push(format!("atomicCompareExchangeWeak held {compared} times"));
    }
    assert!(faults.is_empty(), "{name}: {}", faults.join("; "));
}
