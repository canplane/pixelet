// xpute-tool/spec/data.rs

//! xpute's own data specs, as its targets declare them (xpute-gen-targets).

use xpute_gen::spec::Output;
use xpute_gen::spec::data::outputs_at;

use crate::root;

pub fn outputs() -> Result<Vec<Output>, String> {
    outputs_at(&root(), &xpute_gen_targets::DATA_SPECS)
}
