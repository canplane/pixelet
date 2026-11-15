// xpute-gpu-native-host/image.rs

//! This is `COPY_EXTERNAL_IMAGE` on a native device. The web asks its browser
//! to decode the image (createImageBitmap); here a thread of the host's own
//! decodes it, and a later poll lands it in its texture. The formats are PNG,
//! JPEG and WebP, the ones every browser is sure to read, so a guest can count
//! on them on either host. Any other format is refused here.

use std::io::Cursor;

use super::object::Object;
use super::pending::signal;
use super::run::OwnedTexture;
use super::worker::Worker;
use super::GpuHost;
use xpute_gpu_link::spec::result;

/// A `Decoding` is an image sent to the decoder. It lands in its texture at the
/// first poll after its decode comes back.
pub(super) struct Decoding {
    /// This is its number among the jobs sent. The decoder answers in the order
    /// it was sent them.
    job: u64,
    to: OwnedTexture,
    word: u64,
    ticket: u32,
}

/// A job holds its number, the bytes, flip y and premultiply.
type Job = (u64, Vec<u8>, bool, bool);

pub(super) struct Decoder {
    worker: Worker<Job, (u64, Option<Image>)>,
    next: u64,
}

impl Decoder {
    pub(super) fn new(wake: fn()) -> Decoder {
        Decoder {
            worker: Worker::new(wake, |(job, bytes, flip_y, premultiplied): Job| {
                (job, decode(&bytes, flip_y, premultiplied))
            }),
            next: 0,
        }
    }
}

impl GpuHost {
    /// Copies `bytes` now, since they are the guest's again once the run is
    /// answered (gpu/wit/gpu.wit `run`).
    pub(super) fn copy_external_image(
        &mut self,
        to: OwnedTexture,
        bytes: &[u8],
        flip_y: bool,
        premultiplied: bool,
        word: u64,
        ticket: u32,
    ) {
        let job = self.decoder.next;
        self.decoder.next += 1;
        let _ = self
            .decoder
            .worker
            .ask
            .send((job, bytes.to_vec(), flip_y, premultiplied));
        self.decoding.push_back(Decoding {
            job,
            to,
            word,
            ticket,
        });
    }

    /// Writes each decoded image into its texture, ahead in the queue of
    /// anything the guest submits from now on, and answers `OK`. An image the
    /// host cannot decode is answered `REFUSED`, as on the web. An image
    /// whose texture is gone, or which the device's loss forgot, has nowhere
    /// to land and is not answered.
    pub(super) fn land_images(&mut self) {
        while let Ok((job, image)) = self.decoder.worker.answers.try_recv() {
            let Some(d) = self.decoding.pop_front_if(|d| d.job == job) else {
                continue;
            };
            let exists = self
                .objects
                .iter()
                .flatten()
                .any(|o| matches!(o, Object::Texture(t) if *t == d.to.texture));
            if !exists {
                continue;
            }
            let Some(image) = image else {
                signal(d.word, d.ticket, Some(result::REFUSED));
                continue;
            };
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
            self.queue
                .write_texture(d.to.info(), &image.rgba, layout, size);
            signal(d.word, d.ticket, Some(result::OK));
        }
    }
}

/// The pixels are RGBA8 rows, top first.
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

/// This converts pixels of `channels` bytes each to RGBA8. Gray is spread to
/// the three colors, and a missing alpha is opaque.
fn rgba(width: u32, height: u32, channels: usize, from: &[u8]) -> Image {
    let rgba = match channels {
        4 => from.to_vec(),
        3 => from
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        2 => from
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        _ => from.iter().flat_map(|&g| [g, g, g, 255]).collect(),
    };
    Image {
        width,
        height,
        rgba,
    }
}

fn png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = info.color_type.samples();
    Some(rgba(
        info.width,
        info.height,
        channels,
        &buf[..info.buffer_size()],
    ))
}

fn jpeg(bytes: &[u8]) -> Option<Image> {
    let options = zune_jpeg::zune_core::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA);
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
    Some(rgba(
        width,
        height,
        if decoder.has_alpha() { 4 } else { 3 },
        &buf,
    ))
}
