// pxl-spec/color.rs

//! A color as the code passes it: sRGB, its channels packed into one `u32`.

/// `0xRRGGBB`.
pub type Rgb = u32;

/// `0xRRGGBBAA`, its alpha straight, not premultiplied.
pub type Rgba = u32;

/// The color `hex` as the specs write one, `#rrggbb`. Anything else panics,
/// which in a const is a compile error.
pub const fn rgb(hex: &str) -> Rgb {
    let b = hex.as_bytes();
    assert!(b.len() == 7 && b[0] == b'#', "a color is #rrggbb");
    let mut n = 0;
    let mut i = 1;
    while i < 7 {
        let Some(d) = (b[i] as char).to_digit(16) else {
            panic!("a color is #rrggbb");
        };
        n = n << 4 | d;
        i += 1;
    }
    n
}
