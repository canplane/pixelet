// pxl-asset-bake/brand.rs

//! The two image operations the brand renderer needs, which are the two the
//! sheet does not: cutting a transparent margin away, and fitting an image
//! inside a box without cropping it.

use image::{Rgba, RgbaImage};

/// The bounding box of everything that differs from the corner pixel by more
/// than `threshold`, per channel.
///
/// The symbol is a cube on a transparent field, and its margin is not the
/// same on every side. Keeping the margin makes a size mean the image rather
/// than the cube, and a 16-pixel favicon then drew a 12-pixel cube.
pub fn trim(img: &RgbaImage, threshold: u8) -> RgbaImage {
    let bg = *img.get_pixel(0, 0);
    let differs = |p: &Rgba<u8>| p.0.iter().zip(bg.0.iter()).any(|(a, b)| a.abs_diff(*b) > threshold);

    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if differs(p) {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == u32::MAX {
        return img.clone();
    }
    image::imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// The image scaled to fit inside `w`×`h` without cropping, at whatever size
/// that comes to — the caller places it.
///
/// Lanczos3, because the sources are hand-painted well above the sizes they
/// are drawn at and a box filter would alias the detail they were chosen for.
pub fn fit(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let scale = (w as f64 / img.width() as f64).min(h as f64 / img.height() as f64);
    let (iw, ih) = (((img.width() as f64 * scale).round() as u32).max(1), ((img.height() as f64 * scale).round() as u32).max(1));
    image::imageops::resize(img, iw, ih, image::imageops::FilterType::Lanczos3)
}

/// `img` centred on a `background` field of exactly `w`×`h`.
///
/// One rounding, not two: an icon is the symbol fitted to its inset and then
/// centred on the full square, and doing that as two centrings — into the
/// inset, then into the square — rounds twice and lands a pixel off.
pub fn centre_on(img: &RgbaImage, w: u32, h: u32, background: Rgba<u8>) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(w, h, background);
    // Over, not replace: the field shows through wherever the symbol is
    // transparent, which is the whole point of a flattened icon.
    image::imageops::overlay(&mut out, img, ((w.saturating_sub(img.width())) / 2) as i64, ((h.saturating_sub(img.height())) / 2) as i64);
    out
}
