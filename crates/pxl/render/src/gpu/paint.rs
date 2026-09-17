// pxl_render/gpu/paint.rs

//! A 2D paint stream: what to draw on the host's scratch canvas, as words
//! (api.json's `paint`), run in one call. The host has the fonts and the text
//! shaping a module has not; what is painted — shapes, colours, sizes, text —
//! is decided here and in whoever writes the stream.
//!
//! A string goes as its address and length, so it has to outlive the run:
//! the stream borrows every string it names.

use core::marker::PhantomData;

use super::api::GpuApi;
use super::api_spec::paint as op;

const PAINT_WORDS: usize = 512;

pub struct Paint<'a> {
    ops: [u32; PAINT_WORDS],
    len: usize,
    strings: PhantomData<&'a str>,
}

impl Default for Paint<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Paint<'a> {
    pub fn new() -> Self {
        Paint {
            ops: [0; PAINT_WORDS],
            len: 0,
            strings: PhantomData,
        }
    }

    fn push(&mut self, words: &[u32]) {
        assert!(self.len + words.len() <= PAINT_WORDS, "a paint stream past {PAINT_WORDS} words");
        self.ops[self.len..self.len + words.len()].copy_from_slice(words);
        self.len += words.len();
    }

    fn text(s: &'a str) -> [u32; 2] {
        [s.as_ptr() as usize as u32, s.len() as u32]
    }

    /// The scratch at least `width` by `height`, cleared that far.
    pub fn size(&mut self, width: u32, height: u32) {
        self.push(&[op::SIZE, width, height]);
    }

    pub fn save(&mut self) {
        self.push(&[op::SAVE]);
    }

    pub fn restore(&mut self) {
        self.push(&[op::RESTORE]);
    }

    pub fn translate(&mut self, x: f32, y: f32) {
        self.push(&[op::TRANSLATE, x.to_bits(), y.to_bits()]);
    }

    pub fn scale(&mut self, x: f32, y: f32) {
        self.push(&[op::SCALE, x.to_bits(), y.to_bits()]);
    }

    pub fn smoothing(&mut self, on: bool) {
        self.push(&[op::SMOOTHING, on as u32]);
    }

    pub fn fill_style(&mut self, css: &'a str) {
        let [at, len] = Self::text(css);
        self.push(&[op::FILL_STYLE, at, len]);
    }

    pub fn stroke_style(&mut self, css: &'a str) {
        let [at, len] = Self::text(css);
        self.push(&[op::STROKE_STYLE, at, len]);
    }

    pub fn line_width(&mut self, width: f32) {
        self.push(&[op::LINE_WIDTH, width.to_bits()]);
    }

    pub fn font(&mut self, css: &'a str) {
        let [at, len] = Self::text(css);
        self.push(&[op::FONT, at, len]);
    }

    /// 0 left, 1 center.
    pub fn text_align(&mut self, align: u32) {
        self.push(&[op::TEXT_ALIGN, align]);
    }

    /// 0 middle.
    pub fn text_baseline(&mut self, baseline: u32) {
        self.push(&[op::TEXT_BASELINE, baseline]);
    }

    pub fn begin_path(&mut self) {
        self.push(&[op::BEGIN_PATH]);
    }

    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32) {
        self.push(&[op::ROUND_RECT, x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits(), radius.to_bits()]);
    }

    pub fn arc(&mut self, x: f32, y: f32, radius: f32, from: f32, to: f32) {
        self.push(&[op::ARC, x.to_bits(), y.to_bits(), radius.to_bits(), from.to_bits(), to.to_bits()]);
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.push(&[op::MOVE_TO, x.to_bits(), y.to_bits()]);
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.push(&[op::LINE_TO, x.to_bits(), y.to_bits()]);
    }

    pub fn close_path(&mut self) {
        self.push(&[op::CLOSE_PATH]);
    }

    pub fn fill(&mut self) {
        self.push(&[op::FILL]);
    }

    pub fn stroke(&mut self) {
        self.push(&[op::STROKE]);
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.push(&[op::FILL_RECT, x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits()]);
    }

    pub fn fill_text(&mut self, text: &'a str, x: f32, y: f32) {
        let [at, len] = Self::text(text);
        self.push(&[op::FILL_TEXT, at, len, x.to_bits(), y.to_bits()]);
    }

    /// `image`'s rectangle (sx, sy, sw, sh) drawn into (dx, dy, dw, dh).
    #[allow(clippy::too_many_arguments)]
    pub fn image(&mut self, image: u32, sx: f32, sy: f32, sw: f32, sh: f32, dx: f32, dy: f32, dw: f32, dh: f32) {
        self.push(&[
            op::IMAGE,
            image,
            sx.to_bits(),
            sy.to_bits(),
            sw.to_bits(),
            sh.to_bits(),
            dx.to_bits(),
            dy.to_bits(),
            dw.to_bits(),
            dh.to_bits(),
        ]);
    }

    /// Runs the stream, and starts it over.
    pub fn run(&mut self, g: &GpuApi) {
        (g.paint)(&self.ops[..self.len]);
        self.len = 0;
    }
}
