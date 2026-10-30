// pxl-tool/spec/cmd.rs

//! pixelet's command table, on xpute's (its `base`), written by xpute-tool's
//! (xpute_tool::spec::cmd).

use xpute_tool::spec::cmd::{Target, TsEnums};

use super::{repo, Output};

const TARGETS: [Target; 1] = [Target {
    json: "spec/client/kernel/domain/control/cmd.json",
    ts: "ts/client/src/abi/cmd.spec.ts",
    ts_logical: "@pxl/client/abi/cmd.spec.ts",
    rs_dir: "rust/client/kernel/src/domain/control",
    rs_logical: "pxl-kernel/domain/control",
    number: "xpute_guest::abi::number",
    cpp: None,
    ts_enums: TsEnums::ConstEnum,
}];

pub fn outputs() -> Result<Vec<Output>, String> {
    repo();
    xpute_tool::spec::cmd::outputs(&TARGETS)
}
