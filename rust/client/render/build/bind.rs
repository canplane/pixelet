// pxl-render/build/bind.rs

//! What each binding of a shader is read by, read off the module naga validated: for stages.rs,
//! the stages that read each binding and the layout kinds that take it (`STAGES`), and what
//! each compute entry point does with each binding (`ACCESS`).

use crate::part::VERTEX_ENTRY;
use naga::valid::{GlobalUse, ModuleInfo};

/// For each compute entry point of `module`, the bindings it uses, and whether it reads each,
/// writes it, and writes it through atomics alone (frame/shader/access.rs `READ`, `WRITE`,
/// `ATOMIC`): its line of `ACCESS`.
pub fn access(module: &naga::Module, info: &ModuleInfo) -> String {
    let mut accessed = Vec::new();
    for (i, entry) in module.entry_points.iter().enumerate() {
        if entry.stage != naga::ShaderStage::Compute {
            continue;
        }
        let mut by: Vec<[u32; 3]> = Vec::new();
        for (handle, var) in module.global_variables.iter() {
            let Some(b) = var.binding else { continue };
            let used = info.get_entry_point(i)[handle];
            if used.is_empty() {
                continue;
            }
            let read = used.intersects(GlobalUse::READ | GlobalUse::QUERY);
            let write = used.contains(GlobalUse::WRITE);
            // A binding is written through atomics alone when it holds atomics and no plain
            // store the entry point reaches writes it. A store into a plain member of a
            // struct that also holds atomics is a plain store.
            let atomic = holds_atomic(module, var.ty)
                && !stores_plainly(module, &entry.function, handle, &mut Vec::new());
            let bits = u32::from(read) | u32::from(write) << 1 | u32::from(atomic) << 2;
            match by.iter_mut().find(|a| a[0] == b.group && a[1] == b.binding) {
                Some(a) => a[2] |= bits,
                None => by.push([b.group, b.binding, bits]),
            }
        }
        by.sort();
        let by: Vec<String> = by
            .iter()
            .map(|[g, b, a]| format!("[{g}, {b}, {a}]"))
            .collect();
        accessed.push(format!("(\"{}\", &[{}])", entry.name, by.join(", ")));
    }
    format!("    &[{}],\n", accessed.join(", "))
}

/// For each binding of `module`, its group and number, the stages whose entry points read it,
/// the layout kinds that take it as it is declared, and the vertex entries that read it: the
/// shader's line of `STAGES`.
pub fn stages(module: &naga::Module, info: &ModuleInfo, name: &str) -> String {
    let mut read_by: Vec<(u32, u32, Vec<&str>, String, u32)> = Vec::new();
    for (handle, var) in module.global_variables.iter() {
        let Some(b) = var.binding else { continue };
        let takes = layout_kinds(module, var).unwrap_or_else(|| {
            panic!(
                "{name}: binding {}:{} has no layout kind",
                b.group, b.binding
            )
        });
        let (mut by, mut vertex) = (Vec::new(), 0u32);
        for (i, entry) in module.entry_points.iter().enumerate() {
            let stage = match entry.stage {
                naga::ShaderStage::Vertex => "VERTEX",
                naga::ShaderStage::Fragment => "FRAGMENT",
                _ => "COMPUTE",
            };
            if info.get_entry_point(i)[handle].is_empty() {
                continue;
            }
            if !by.contains(&stage) {
                by.push(stage);
            }
            if entry.stage == naga::ShaderStage::Vertex {
                let k = VERTEX_ENTRY
                    .iter()
                    .position(|&n| n == entry.name)
                    .unwrap_or_else(|| {
                        panic!("{name}: vertex entry {} not in VERTEX_ENTRY", entry.name)
                    });
                vertex |= 1 << k;
            }
        }
        // Two parts may declare the same binding when only one of them reads it. The entry
        // kept is the one that is read.
        match read_by
            .iter_mut()
            .find(|r| (r.0, r.1) == (b.group, b.binding))
        {
            Some(_) if by.is_empty() => {}
            Some(r) if r.2.is_empty() => *r = (b.group, b.binding, by, takes, vertex),
            Some(_) => panic!("{name}: binding {}:{} read twice", b.group, b.binding),
            None => read_by.push((b.group, b.binding, by, takes, vertex)),
        }
    }
    read_by.sort();
    let entries: Vec<String> = read_by
        .iter()
        .map(|(g, b, by, takes, vertex)| {
            format!(
                "[{g}, {b}, {}, {takes}, {vertex}]",
                if by.is_empty() {
                    "NONE".to_string()
                } else {
                    by.join(" | ")
                }
            )
        })
        .collect();
    format!("    &[{}],\n", entries.join(", "))
}

/// The source of `binds`, written into stages.rs beside `STAGES`. `binds` holds when a compute
/// group's (binding, stages, kind) words lay out exactly what the shaders of the pipelines over
/// it read. Every binding such a shader reads must be in group 0, have an entry in `group` that
/// the compute stage sees, and have a kind its declaration takes (its number, whether a buffer
/// is read-only, a texture's kind). Every entry of `group` must be read by one of the shaders.
/// Each owner of a compute group asserts it.
pub const BINDS: &str = "
/// Whether `group`, a compute group's (binding, stages, kind) words, lays out exactly what
/// the shaders of compute pipelines `pipelines` (`compute`) read, each binding as its
/// declaration takes it (build/bind.rs `BINDS`).
pub const fn binds(pipelines: &[u32], group: &[u32]) -> bool {
    let mut s = 0;
    while s < pipelines.len() {
        let entries = STAGES[compute(pipelines[s]).0 as usize];
        let mut k = 0;
        while k < entries.len() {
            let [g, b, by, takes, _] = entries[k];
            if by != NONE {
                let mut found = false;
                let mut e = 0;
                while e < group.len() {
                    found |= group[e] == b
                        && group[e + 1] & COMPUTE != 0
                        && takes & 1 << group[e + 2] != 0;
                    e += 3;
                }
                if g != 0 || !found {
                    return false;
                }
            }
            k += 1;
        }
        s += 1;
    }
    let mut e = 0;
    while e < group.len() {
        let mut read = false;
        let mut s = 0;
        while s < pipelines.len() {
            read |= stages_of(compute(pipelines[s]).0, 0, group[e]) != NONE;
            s += 1;
        }
        if !read {
            return false;
        }
        e += 3;
    }
    true
}
";

/// Whether `function`, or a function it calls, makes a plain store into `global`: a `Store` of
/// a non-atomic value into the global, or into a member or element of it. `seen` holds the
/// functions already walked.
fn stores_plainly(
    module: &naga::Module,
    function: &naga::Function,
    global: naga::Handle<naga::GlobalVariable>,
    seen: &mut Vec<naga::Handle<naga::Function>>,
) -> bool {
    use naga::{Expression, Statement};
    /// The global a pointer is into and the type it points at, where the pointer is to a
    /// global or to a member or an element of one.
    fn pointee(
        module: &naga::Module,
        f: &naga::Function,
        e: naga::Handle<Expression>,
    ) -> Option<(naga::Handle<naga::GlobalVariable>, naga::Handle<naga::Type>)> {
        use naga::TypeInner;
        let element = |ty: naga::Handle<naga::Type>, idx: Option<u32>| match &module.types[ty].inner
        {
            TypeInner::Array { base, .. } => Some(*base),
            TypeInner::Struct { members, .. } => idx.map(|i| members[i as usize].ty),
            _ => None,
        };
        match f.expressions[e] {
            Expression::GlobalVariable(g) => Some((g, module.global_variables[g].ty)),
            Expression::AccessIndex { base, index } => {
                let (g, ty) = pointee(module, f, base)?;
                Some((g, element(ty, Some(index))?))
            }
            Expression::Access { base, .. } => {
                let (g, ty) = pointee(module, f, base)?;
                Some((g, element(ty, None)?))
            }
            _ => None,
        }
    }
    fn block(
        module: &naga::Module,
        f: &naga::Function,
        b: &naga::Block,
        global: naga::Handle<naga::GlobalVariable>,
        seen: &mut Vec<naga::Handle<naga::Function>>,
    ) -> bool {
        b.iter().any(|st| match st {
            // naga holds an `atomicStore` as a `Store` to an atomic, so a plain store is one to
            // anything else.
            Statement::Store { pointer, .. } => {
                pointee(module, f, *pointer).is_some_and(|(g, ty)| {
                    g == global && !matches!(module.types[ty].inner, naga::TypeInner::Atomic(_))
                })
            }
            Statement::Block(inner) => block(module, f, inner, global, seen),
            Statement::If { accept, reject, .. } => {
                block(module, f, accept, global, seen) || block(module, f, reject, global, seen)
            }
            Statement::Switch { cases, .. } => cases
                .iter()
                .any(|c| block(module, f, &c.body, global, seen)),
            Statement::Loop {
                body, continuing, ..
            } => block(module, f, body, global, seen) || block(module, f, continuing, global, seen),
            Statement::Call { function, .. } => {
                if seen.contains(function) {
                    return false;
                }
                seen.push(*function);
                let called = &module.functions[*function];
                block(module, called, &called.body, global, seen)
            }
            _ => false,
        })
    }
    block(module, function, &function.body, global, seen)
}

/// Whether a value of `ty` is or holds an atomic.
fn holds_atomic(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
    match &module.types[ty].inner {
        naga::TypeInner::Atomic(_) => true,
        naga::TypeInner::Array { base, .. } => holds_atomic(module, *base),
        naga::TypeInner::Struct { members, .. } => {
            members.iter().any(|m| holds_atomic(module, m.ty))
        }
        _ => false,
    }
}

/// The layout kinds (api.json's record words) that can bind `var` as it is declared, written as
/// an expression of bits over record.rs's names. None when no record word lays out its kind.
fn layout_kinds(module: &naga::Module, var: &naga::GlobalVariable) -> Option<String> {
    use naga::{
        AddressSpace, ImageClass, ImageDimension as D, ScalarKind, StorageAccess, StorageFormat,
        TypeInner,
    };
    let kinds: &[&str] = match (var.space, &module.types[var.ty].inner) {
        (AddressSpace::Uniform, _) => &["UNIFORM", "UNIFORM_DYNAMIC"],
        (AddressSpace::Storage { access }, _) if access.contains(StorageAccess::STORE) => {
            &["STORAGE_RW"]
        }
        (AddressSpace::Storage { .. }, _) => &["STORAGE"],
        (_, TypeInner::Sampler { comparison: true }) => &["SAMPLER_COMPARISON"],
        (_, TypeInner::Sampler { comparison: false }) => &["SAMPLER"],
        (
            _,
            &TypeInner::Image {
                dim,
                arrayed,
                ref class,
            },
        ) => match (dim, arrayed, class) {
            (
                D::D2,
                false,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE", "TEXTURE_UNFILTERED"],
            (
                D::D2,
                true,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE_ARRAY"],
            (
                D::D2,
                true,
                ImageClass::Sampled {
                    kind: ScalarKind::Uint,
                    multi: false,
                },
            ) => &["TEXTURE_UINT_ARRAY"],
            (
                D::D3,
                false,
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
            ) => &["TEXTURE_3D"],
            (D::D2, false, ImageClass::Depth { multi: false }) => &["DEPTH"],
            (
                D::D2,
                true,
                ImageClass::Storage {
                    format: StorageFormat::R32Uint,
                    access,
                },
            ) if access.contains(StorageAccess::LOAD | StorageAccess::STORE) => {
                &["STORAGE_TEXTURE_RW"]
            }
            (
                D::D3,
                false,
                ImageClass::Storage {
                    format: StorageFormat::Rgba16Float,
                    access,
                },
            ) if *access == StorageAccess::STORE => &["STORAGE_TEXTURE_3D"],
            (
                D::D2,
                false,
                ImageClass::Storage {
                    format: StorageFormat::Rgba16Float,
                    access,
                },
            ) if *access == StorageAccess::STORE => &["STORAGE_TEXTURE_2D"],
            _ => return None,
        },
        _ => return None,
    };
    Some(
        kinds
            .iter()
            .map(|k| format!("1 << {k}"))
            .collect::<Vec<_>>()
            .join(" | "),
    )
}
