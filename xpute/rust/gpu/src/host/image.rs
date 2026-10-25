// xpute-gpu/host/image.rs

//! COPY_EXTERNAL_IMAGE on a native device: what the web asks of its browser
//! (createImageBitmap), decoded here on a thread of its own and landed at a
//! later poll. PNG, JPEG and WebP, what a browser is sure to read; anything
//! else is left out, as there.

use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver, Sender};

use xpute_host::mem::Memory;

use super::object::Object;
use super::run::OwnedTexture;
use super::GpuHost;
use crate::spec::DONE;

/// An image sent the decoder, landed in its texture at the poll after its
/// decode comes back.
pub(super) struct Decoding {
    /// Its place in what was sent, which the decoder answers in order.
    job: u64,
    to: OwnedTexture,
    word: u32,
    ticket: u32,
}

/// A job's number, the bytes, flip y and premultiply.
type Job = (u64, Vec<u8>, bool, bool);

pub(super) struct Decoder {
    ask: Sender<Job>,
    answers: Receiver<(u64, Option<Image>)>,
    next: u64,
}

impl Decoder {
    /// Each answer `wake`s the host's loop: nothing else asks for the poll
    /// that lands it.
    pub(super) fn new(wake: fn()) -> Decoder {
        let (ask, jobs) = channel::<Job>();
        let (answer, answers) = channel();
        std::thread::spawn(move || {
            for (job, bytes, flip_y, premultiplied) in jobs {
                if answer.send((job, decode(&bytes, flip_y, premultiplied))).is_err() {
                    return;
                }
                wake();
            }
        });
        Decoder { ask, answers, next: 0 }
    }
}

impl GpuHost {
    /// `bytes` copied now, since they are the guest's only until the next turn.
    pub(super) fn copy_external_image(&mut self, to: OwnedTexture, bytes: &[u8], flip_y: bool, premultiplied: bool, word: u32, ticket: u32) {
        let job = self.decoder.next;
        self.decoder.next += 1;
        let _ = self.decoder.ask.send((job, bytes.to_vec(), flip_y, premultiplied));
        self.decoding.push_back(Decoding { job, to, word, ticket });
    }

    /// Each decoded image written to its texture, ahead in the queue of
    /// anything the guest submits from here. One the host cannot decode is
    /// left out with its word unwritten, as on the web; one whose texture went
    /// has nowhere to land; one the device's loss forgot answers to nothing.
    pub(super) fn land_images(&mut self, memory: &mut Memory<'_>) {
        while let Ok((job, image)) = self.decoder.answers.try_recv() {
            let Some(d) = self.decoding.pop_front_if(|d| d.job == job) else { continue };
            let held = self.objects.iter().flatten().any(|o| matches!(o, Object::Texture(t) if *t == d.to.texture));
            let Some(image) = image.filter(|_| held) else { continue };
            let layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * image.width),
                rows_per_image: Some(image.height),
            };
            let size = wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            };
            self.queue.write_texture(d.to.info(), &image.rgba, layout, size);
            if memory.word(d.word) == d.ticket {
                memory.set_word(d.word, d.ticket | DONE);
            }
        }
    }
}

/// RGBA8 rows, top first.
struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decode(bytes: &[u8], flip_y: bool, premultiplied: bool) -> Option<Image> {
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
