// pxl-render/build/wgsl.rs

//! How the build writes a value the Rust holds for the shaders: as a constant, as a table a
//! function reads by a switch, or as a predicate over a value.

use std::fmt::Display;

/// A value a shader names, with the WGSL type it takes.
#[derive(Clone)]
pub(crate) enum Val {
    U32(u32),
    I32(i32),
    F32(f32),
    /// An f32 written as its bits, so that the shader reads the float the Rust computed
    /// whatever its compiler makes of a decimal.
    Bits(f32),
    Vec2f([f32; 2]),
    Vec3f([f32; 3]),
    Vec4f([f32; 4]),
    Vec2u([u32; 2]),
    Vec4u([u32; 4]),
    Vec3i([i32; 3]),
    Vec4i([i32; 4]),
    Array(Vec<Val>),
}

impl Val {
    fn ty(&self) -> String {
        match self {
            Val::U32(_) => "u32".into(),
            Val::I32(_) => "i32".into(),
            Val::F32(_) | Val::Bits(_) => "f32".into(),
            Val::Vec2f(_) => "vec2f".into(),
            Val::Vec3f(_) => "vec3f".into(),
            Val::Vec4f(_) => "vec4f".into(),
            Val::Vec2u(_) => "vec2u".into(),
            Val::Vec4u(_) => "vec4u".into(),
            Val::Vec3i(_) => "vec3i".into(),
            Val::Vec4i(_) => "vec4i".into(),
            Val::Array(v) => format!("array<{}, {}>", v[0].ty(), v.len()),
        }
    }

    /// The value as a literal: a scalar as written, a vector or an array as its type built
    /// from its elements.
    fn lit(&self) -> String {
        let elems: Vec<Val> = match self {
            Val::U32(v) => return format!("{v}u"),
            Val::I32(v) => return format!("{v}i"),
            Val::F32(v) => return format!("{v:?}"),
            Val::Bits(v) => return format!("bitcast<f32>({}u)", v.to_bits()),
            Val::Vec2f(v) => v.map(Val::F32).to_vec(),
            Val::Vec3f(v) => v.map(Val::F32).to_vec(),
            Val::Vec4f(v) => v.map(Val::F32).to_vec(),
            Val::Vec2u(v) => v.map(Val::U32).to_vec(),
            Val::Vec4u(v) => v.map(Val::U32).to_vec(),
            Val::Vec3i(v) => v.map(Val::I32).to_vec(),
            Val::Vec4i(v) => v.map(Val::I32).to_vec(),
            Val::Array(v) => v.clone(),
        };
        let lits: Vec<String> = elems.iter().map(Val::lit).collect();
        format!("{}({})", self.ty(), lits.join(", "))
    }
}

/// Each value as a constant of its name.
pub(crate) fn consts<N: Display>(vals: impl IntoIterator<Item = (N, Val)>) -> String {
    vals.into_iter()
        .map(|(name, v)| format!("const {name}: {} = {};\n", v.ty(), v.lit()))
        .collect()
}

/// A function `name` of `k` that returns each case's value at its `k`, and `default` at every
/// other.
pub(crate) fn table(
    name: &str,
    cases: impl IntoIterator<Item = (u32, Val)>,
    default: Val,
) -> String {
    let cases: String = cases
        .into_iter()
        .map(|(k, v)| format!("case {k}u: {{ return {}; }} ", v.lit()))
        .collect();
    format!(
        "fn {name}(k: u32) -> {} {{ switch k {{ {cases}default: {{ return {}; }} }} }}\n",
        default.ty(),
        default.lit()
    )
}

/// The WGSL predicate `name` over a value, true where `rule` holds: written as the runs of
/// values under `cnt` that it holds, a run that reaches either end of them open there, so a
/// value past them answers as the last one does. It answers `rule` only for a value under
/// `cnt`.
pub(crate) fn predicate(name: &str, cnt: u32, rule: impl Fn(u32) -> bool) -> String {
    let last = cnt - 1;
    let mut runs: Vec<[u32; 2]> = Vec::new();
    for v in (0..=last).filter(|&v| rule(v)) {
        match runs.last_mut() {
            Some(r) if r[1] + 1 == v => r[1] = v,
            _ => runs.push([v, v]),
        }
    }
    let run = |&[lo, hi]: &[u32; 2]| match (lo == 0, hi == last) {
        (true, true) => "true".to_owned(),
        (true, false) => format!("v <= {hi}u"),
        (false, true) => format!("v >= {lo}u"),
        (false, false) if runs.len() == 1 => format!("v >= {lo}u && v <= {hi}u"),
        (false, false) => format!("(v >= {lo}u && v <= {hi}u)"),
    };
    let body = if runs.is_empty() {
        "false".to_owned()
    } else {
        runs.iter().map(run).collect::<Vec<_>>().join(" || ")
    };
    format!("fn {name}(v: u32) -> bool {{ return {body}; }}\n")
}
