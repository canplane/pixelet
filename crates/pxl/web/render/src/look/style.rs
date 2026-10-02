// pxl-render/look/style.rs

pub mod marker_colors {
    /// Reserved for "you could grab this", never for a preview of a change.
    pub const ACTION: u32 = 0x4fd1c5;
    /// For removal where the thing cannot be drawn as itself. Dark, since users
    /// read ACTION's cyan as "move".
    pub const ERASE: u32 = 0x1d2026;
}
