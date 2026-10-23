// xpute-gpu/image.rs

//! An encoded image's bytes as RGBA8 rows, top first, for COPY_EXTERNAL_IMAGE
//! on a native device: what the web asks of its browser (createImageBitmap),
//! decoded here. PNG, JPEG and WebP, which are what a browser is sure to
//! read; anything else is none, as an image the browser cannot read is left
//! out there.

use std::io::Cursor;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(bytes: &[u8], flip_y: bool, premultiplied: bool) -> Option<Image> {
    let mut image = if bytes.starts_with(b"\x89PNG") {
        png(bytes)
    } else if bytes.starts_with(b"\xff\xd8") {
        jpeg(bytes)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        webp(bytes)
    } else {
        None
    }?;
    if flip_y {
        let row = 4 * image.width as usize;
        let rows = image.height as usize;
        for y in 0..rows / 2 {
            let (top, bottom) = image.rgba.split_at_mut((rows - 1 - y) * row);
            top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
        }
    }
    if premultiplied {
        for p in image.rgba.as_chunks_mut::<4>().0 {
            let a = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
    Some(image)
}

/// Pixels of `channels` bytes each, as RGBA8: grey spread to the three
/// colours, a missing alpha opaque.
fn rgba(width: u32, height: u32, channels: usize, from: &[u8]) -> Image {
    let rgba = match channels {
        4 => from.to_vec(),
        3 => from.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        2 => from.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        _ => from.iter().flat_map(|&g| [g, g, g, 255]).collect(),
    };
    Image { width, height, rgba }
}

fn png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = info.color_type.samples();
    Some(rgba(info.width, info.height, channels, &buf[..info.buffer_size()]))
}

fn jpeg(bytes: &[u8]) -> Option<Image> {
    let options = zune_jpeg::zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    let rgba = decoder.decode().ok()?;
    let info = decoder.info()?;
    Some(Image {
        width: u32::from(info.width),
        height: u32::from(info.height),
        rgba,
    })
}

fn webp(bytes: &[u8]) -> Option<Image> {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
    let (width, height) = decoder.dimensions();
    let mut buf = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buf).ok()?;
    Some(rgba(width, height, if decoder.has_alpha() { 4 } else { 3 }, &buf))
}
