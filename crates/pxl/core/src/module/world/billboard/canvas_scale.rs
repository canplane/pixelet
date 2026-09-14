// pxl_core/module/world/billboard/canvas_scale.rs

//! Canvas-drawing constants shared by every world-anchored billboard consumer
//! (the chat bubble, the POI badges) so they stay matched by construction
//! instead of each picking its own.

/// Matches apps/web/src/app/style/index.css's own body font-family. Each
/// consumer still picks its own size/weight.
pub const FONT_FAMILY: &str = r#"-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Pretendard Variable", system-ui, sans-serif"#;

/// Mirrors the app's own HTML-side weight scale (Tailwind's font-normal /
/// font-semibold) rather than canvas's "bold" keyword, which resolves to 700 —
/// Pretendard Variable's weight axis (45–920) renders either as a real
/// distinct face, not a synthetic bold.
pub const FONT_WEIGHT_REGULAR: u32 = 400;
pub const FONT_WEIGHT_BOLD: u32 = 600;
