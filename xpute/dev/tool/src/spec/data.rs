// xpute-tool/spec/data.rs

//! xpute's own data specs, then the GPU plugin's, each as its targets declare
//! them (xpute-gen-targets, xpute-gpu-gen-targets).

use xpute_gen::spec::data::outputs_at;
use xpute_gen::spec::Output;

use crate::root;

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = outputs_at(&root(), &xpute_gen_targets::DATA_SPECS)?;
    out.extend(outputs_at(&root(), &xpute_gpu_gen_targets::DATA_SPECS)?);
    Ok(out)
}
