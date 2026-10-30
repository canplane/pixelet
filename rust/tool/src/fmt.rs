// pxl-tool/fmt.rs

//! xpute-tool's formatter (xpute_tool::fmt), over this repository.

use xpute_tool::fmt as xpute_fmt;

use crate::spec::repo;

pub fn format_text(path: &str, text: String) -> Result<String, String> {
    repo();
    xpute_fmt::format_text(path, text)
}
