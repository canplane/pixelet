// pxl-tool/fmt.rs

//! xpute-tool's formatter (xpute_tool::fmt), over this repository.

pub fn format_text(path: &str, text: String) -> Result<String, String> {
    crate::spec::repo();
    xpute_tool::fmt::format_text(path, text)
}
